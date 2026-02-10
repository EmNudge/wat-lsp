//! Shared diagnostic tree walk for both native and WASM builds.
//!
//! This module provides the unified tree walk that orchestrates all semantic
//! diagnostic checks in a single pass over the AST. Both native and WASM
//! builds delegate to this shared implementation.

use crate::core::types::Diagnostic;
use crate::symbols::{SymbolTable, ValueType};
use crate::utils::{determine_instruction_context_at_node, InstructionContext};

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

        if !found_instr_list && !expected_results.is_empty() {
            diagnostics.push(Diagnostic::error(
                crate::utils::node_to_range(&node),
                format!(
                    "Type mismatch: function body leaves 0 values on stack but signature requires {}",
                    expected_results.len()
                ),
            ));
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
            crate::diagnostics_core::references::check_try_catch_clause_references(
                &node, source, symbols,
            ),
        );
        // Continue recursing to check nested instructions
    }

    // Special handling for start directive
    if kind_str == "module_field_start" {
        diagnostics.extend(crate::diagnostics_core::references::check_start_references(
            &node, source, symbols,
        ));
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
            crate::diagnostics_core::references::check_try_delegate_clause_references(
                &node, source, symbols,
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

    // Check SIMD lane indices and alignment constraints
    if kind_str == "instr_plain" {
        diagnostics.extend(crate::diagnostics_core::simd_checks::check_simd_lane_index(
            &node, source,
        ));
        diagnostics.extend(crate::diagnostics_core::alignment_checks::check_alignment(
            &node, source,
        ));
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
