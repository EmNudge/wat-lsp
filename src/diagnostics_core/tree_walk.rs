//! Shared diagnostic tree walk for both native and WASM builds.
//!
//! This module provides the unified tree walk that orchestrates all semantic
//! diagnostic checks in a single pass over the AST. Both native and WASM
//! builds delegate to this shared implementation.

use std::collections::HashSet;

use crate::core::types::Diagnostic;
use crate::symbols::{SymbolTable, ValueType};
use crate::utils::{determine_instruction_context_at_node, node_to_range, InstructionContext};

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Pre-computed configuration for which diagnostic checks to perform.
pub struct DiagnosticConfig {
    pub check_atomics: bool,
    pub check_memory64: bool,
}

impl DiagnosticConfig {
    /// Build config from symbol table state.
    pub fn from_symbols(symbols: &SymbolTable) -> Self {
        Self {
            check_atomics: !symbols.memories.is_empty()
                && !symbols.memories.iter().any(|m| m.shared),
            check_memory64: symbols.memories.iter().any(|m| m.is_memory64),
        }
    }
}

/// Walk the AST and collect all semantic diagnostics in a single pass.
///
/// This is the shared implementation used by both native and WASM builds.
/// Callers can convert the returned `Vec<Diagnostic>` to platform-specific
/// types as needed (e.g., `tower_lsp::lsp_types::Diagnostic` for native).
pub fn walk_tree_for_diagnostics(
    node: Node,
    source: &str,
    symbols: &SymbolTable,
    config: &DiagnosticConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = node.kind();

    // Normalize kind for cross-platform matching
    #[cfg(feature = "native")]
    let kind_str = kind;
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind_str = &*kind;

    // Check for packed types (i8/i16) used in value type positions (params, results, locals, globals)
    if kind_str == "ERROR" {
        let text = &source[node.byte_range()];
        let trimmed = text.trim();
        if (trimmed == "i8" || trimmed == "i16") && is_value_type_context(&node) {
            diagnostics.push(Diagnostic::error(
                node_to_range(&node),
                format!(
                    "{} is a packed storage type and can only be used in struct/array field definitions",
                    trimmed
                ),
            ));
        }
    }

    // Check block label mismatches
    if matches!(kind_str, "block_block" | "block_loop" | "block_if") {
        diagnostics.extend(
            crate::diagnostics_core::module_checks::check_block_label_mismatch(&node, source),
        );
    }

    // Check for function body instruction lists — stack tracking and return type validation
    if kind_str == "module_field_func" {
        let func_line = node.start_position().row as u32;
        let expected_results: &[ValueType] = symbols
            .find_function_containing_line(func_line)
            .map(|f| f.results.as_slice())
            .unwrap_or(&[]);

        let mut cursor = node.walk();
        let mut found_instr_list = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "instr_list" {
                diagnostics.extend(crate::diagnostics_core::track_stack_in_instr_list(
                    &child,
                    source,
                    symbols,
                    Some(expected_results),
                ));
                found_instr_list = true;
                break;
            }
        }

        if !found_instr_list && !expected_results.is_empty() && !has_inline_import(&node) {
            diagnostics.push(Diagnostic::error(
                node_to_range(&node),
                format!(
                    "Type mismatch: function body leaves 0 values on stack but signature requires {}",
                    expected_results.len()
                ),
            ));
        }

        // Check for unused locals and parameters
        if !has_inline_import(&node) {
            check_unused_locals(&node, source, symbols, diagnostics);
        }
    }

    // Special handling for catch clauses (try_table)
    if kind_str == "catch_clause" {
        diagnostics.extend(
            crate::diagnostics_core::references::check_catch_clause_references(
                &node, source, symbols,
            ),
        );
        return;
    }

    // Special handling for legacy try catch clause
    if kind_str == "try_catch_clause" {
        diagnostics.extend(
            crate::diagnostics_core::references::check_first_index_reference(
                &node,
                source,
                symbols,
                &InstructionContext::Tag,
            ),
        );
        // Continue recursing to check nested instructions
    }

    // Special handling for start directive
    if kind_str == "module_field_start" {
        diagnostics.extend(
            crate::diagnostics_core::references::check_first_index_reference(
                &node,
                source,
                symbols,
                &InstructionContext::Call,
            ),
        );
        return;
    }

    // Check module-level references (exports, elem/data segment refs)
    {
        let module_ref_diags = crate::diagnostics_core::references::check_module_level_references(
            &node, source, symbols,
        );
        if !module_ref_diags.is_empty() {
            diagnostics.extend(module_ref_diags);
            if kind_str != "module_field_elem" {
                return;
            }
        }
    }

    // Special handling for try_delegate_clause (legacy try): index is a label reference
    if kind_str == "try_delegate_clause" {
        diagnostics.extend(
            crate::diagnostics_core::references::check_first_index_reference(
                &node,
                source,
                symbols,
                &InstructionContext::Branch,
            ),
        );
        return;
    }

    // Check folded expression operand count (expr1_plain nodes)
    if kind_str == "expr1_plain" {
        diagnostics.extend(
            crate::diagnostics_core::folded_checks::check_folded_operand_count(
                &node, source, symbols,
            ),
        );

        let text = &source[node.byte_range()];
        let first_token = text.split_whitespace().next().unwrap_or("");

        if config.check_atomics {
            diagnostics.extend(
                crate::diagnostics_core::memory_checks::check_atomic_operation(&node, first_token),
            );
        }
        if config.check_memory64 {
            diagnostics.extend(
                crate::diagnostics_core::memory_checks::check_memory64_for_instruction(
                    &node,
                    source,
                    symbols,
                    first_token,
                ),
            );
        }
    }

    // Check SIMD lane indices, alignment constraints, and GC struct field access
    if kind_str == "instr_plain" {
        diagnostics.extend(crate::diagnostics_core::simd_checks::check_simd_lane_index(
            &node, source,
        ));
        diagnostics.extend(crate::diagnostics_core::alignment_checks::check_alignment(
            &node, source,
        ));
        diagnostics.extend(
            crate::diagnostics_core::gc_checks::check_struct_field_access(&node, source, symbols),
        );
    } else if kind_str == "expr1_plain" {
        // In folded form, instr_plain is a child that won't be visited separately
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "instr_plain" {
                diagnostics.extend(crate::diagnostics_core::simd_checks::check_simd_lane_index(
                    &child, source,
                ));
                diagnostics.extend(crate::diagnostics_core::alignment_checks::check_alignment(
                    &child, source,
                ));
                diagnostics.extend(
                    crate::diagnostics_core::gc_checks::check_struct_field_access(
                        &child, source, symbols,
                    ),
                );
                break;
            }
        }
    }

    // Check for undefined references based on instruction context
    let context = determine_instruction_context_at_node(&node, source);
    if context != InstructionContext::General {
        diagnostics.extend(crate::diagnostics_core::references::check_references(
            &node, source, symbols, &context,
        ));

        // For instr_plain, also check atomic, memory64, and arity (linear format)
        if kind_str == "instr_plain" {
            let text = &source[node.byte_range()];
            let first_token = text.split_whitespace().next().unwrap_or("");

            if config.check_atomics {
                diagnostics.extend(
                    crate::diagnostics_core::memory_checks::check_atomic_operation(
                        &node,
                        first_token,
                    ),
                );
            }

            let parent_is_expr1 = node
                .parent()
                .map(|p| p.kind() == "expr1_plain")
                .unwrap_or(false);
            if !parent_is_expr1 {
                if config.check_memory64 {
                    diagnostics.extend(
                        crate::diagnostics_core::memory_checks::check_memory64_for_instruction(
                            &node,
                            source,
                            symbols,
                            first_token,
                        ),
                    );
                }
                diagnostics.extend(
                    crate::diagnostics_core::arity::check_instruction_parameter_count(
                        &node, source,
                    ),
                );
            }
        }

        // Only recurse into nested expr nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "expr" {
                walk_tree_for_diagnostics(child, source, symbols, config, diagnostics);
            }
        }
        return;
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_diagnostics(child, source, symbols, config, diagnostics);
    }
}

/// Check if an ERROR node is in a value type context (params, results, locals, globals)
fn is_value_type_context(node: &Node) -> bool {
    if let Some(parent) = node.parent() {
        let pk = parent.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let pk = &*pk;
        matches!(
            pk,
            "func_type_params_many"
                | "func_type_params_one"
                | "func_type_results"
                | "func_locals_many"
                | "func_locals_one"
                | "global_type_imm"
                | "global_type_mut"
        )
    } else {
        false
    }
}

/// Check if a function node has an inline import
fn has_inline_import(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = &*ck;
        if ck == "import" {
            return true;
        }
    }
    false
}

/// Check for unused locals and parameters in a function.
fn check_unused_locals(
    func_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let func_line = func_node.start_position().row as u32;
    let func = match symbols.find_function_containing_line(func_line) {
        Some(f) => f,
        None => return,
    };

    // Total number of params + locals
    let num_params = func.parameters.len();
    let num_locals = func.locals.len();
    let total = num_params + num_locals;
    if total == 0 {
        return;
    }

    // Collect all used indices by scanning the function body
    let mut used_indices: HashSet<usize> = HashSet::new();
    collect_local_uses(func_node, source, func, &mut used_indices);

    // Check parameters
    for param in &func.parameters {
        if param.name.is_none() {
            continue; // Can't meaningfully warn on unnamed params
        }
        if !used_indices.contains(&param.index) {
            if let Some(range) = param.range {
                let name = param.name.as_ref().unwrap();
                diagnostics.push(Diagnostic::hint(
                    range,
                    format!("Unused parameter {}", name),
                ));
            }
        }
    }

    // Check locals
    for local in &func.locals {
        if local.name.is_none() {
            continue; // Can't meaningfully warn on unnamed locals
        }
        let absolute_idx = num_params + local.index;
        if !used_indices.contains(&absolute_idx) {
            if let Some(range) = local.range {
                let name = local.name.as_ref().unwrap();
                diagnostics.push(Diagnostic::warning(range, format!("Unused local {}", name)));
            }
        }
    }
}

/// Recursively scan a node tree for local.get/local.set/local.tee references
/// and record which local indices are used.
fn collect_local_uses(
    node: &Node,
    source: &str,
    func: &crate::symbols::Function,
    used: &mut HashSet<usize>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = &*kind;

    if kind == "instr_plain" || kind == "expr1_plain" {
        let text = &source[node.byte_range()];
        let first_token = text.split_whitespace().next().unwrap_or("");
        if matches!(first_token, "local.get" | "local.set" | "local.tee") {
            // Find the index/identifier child
            if let Some(idx) = resolve_local_index(node, source, func) {
                used.insert(idx);
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_local_uses(&child, source, func, used);
    }
}

/// Resolve a text reference (name or numeric index) to a local index.
fn resolve_local_text(text: &str, func: &crate::symbols::Function) -> Option<usize> {
    if text.starts_with('$') {
        for param in &func.parameters {
            if param.name.as_deref() == Some(text) {
                return Some(param.index);
            }
        }
        for local in &func.locals {
            if local.name.as_deref() == Some(text) {
                return Some(func.parameters.len() + local.index);
            }
        }
    } else if let Ok(idx) = text.parse::<usize>() {
        return Some(idx);
    }
    None
}

/// Resolve the local index referenced by a local.get/set/tee instruction.
fn resolve_local_index(
    node: &Node,
    source: &str,
    func: &crate::symbols::Function,
) -> Option<usize> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = &*ck;

        if ck == "index" || ck == "identifier" || ck == "nat" {
            let text = source[child.byte_range()].trim();
            if let Some(idx) = resolve_local_text(text, func) {
                return Some(idx);
            }
            // Check inside index node for identifier/nat children
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                let ik = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ik = &*ik;
                if ik == "identifier" || ik == "nat" {
                    let id_text = source[idx_child.byte_range()].trim();
                    if let Some(idx) = resolve_local_text(id_text, func) {
                        return Some(idx);
                    }
                }
            }
        }
    }
    None
}
