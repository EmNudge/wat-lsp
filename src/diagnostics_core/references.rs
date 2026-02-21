//! Shared reference checking functions for both native and WASM builds.
//!
//! This module provides platform-agnostic functions for checking undefined
//! references in WAT code.
#![allow(clippy::borrow_deref_ref)]

use crate::core::types::{Diagnostic, Position};
use crate::parser::normalize_identifier;
use crate::symbols::SymbolTable;
use crate::utils::{find_containing_function, node_to_range, InstructionContext, STRUCT_OPS};

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check references in a catch_clause node (try_table syntax)
/// For (catch $tag $label) and (catch_ref $tag $label): first index is tag, second is label
/// For (catch_all $label) and (catch_all_ref $label): single index is label
pub fn check_catch_clause_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let text = &source[node.byte_range()];

    // Determine if this is catch/catch_ref (has tag) or catch_all/catch_all_ref (no tag)
    let has_tag =
        text.contains("catch_ref") || (text.contains("catch") && !text.contains("catch_all"));

    // Collect all index children
    let mut cursor = node.walk();
    let indices: Vec<_> = node
        .children(&mut cursor)
        .filter(|c| {
            node_kind!(kind = c);
            kind == "index"
        })
        .collect();

    for (i, index_node) in indices.iter().enumerate() {
        // First index is tag (if has_tag), remaining are labels
        let is_tag_reference = has_tag && i == 0;
        let context = if is_tag_reference {
            InstructionContext::Tag
        } else {
            InstructionContext::Branch
        };
        diagnostics.extend(find_undefined_identifiers(
            index_node, source, symbols, &context,
        ));
    }

    diagnostics
}

/// Check the first index child of a node against the given instruction context.
/// Used for nodes that contain a single index reference (start, try_catch, try_delegate).
pub fn check_first_index_reference(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    context: &InstructionContext,
) -> Vec<Diagnostic> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);
        if kind == "index" {
            return find_undefined_identifiers(&child, source, symbols, context);
        }
    }
    vec![]
}

/// Find and validate only the first index identifier in a node
/// Used for instructions like struct.get where only the first index is a type
fn find_first_index_identifier(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    context: &InstructionContext,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "index" {
            // Found the first index, check its identifier
            diagnostics.extend(find_undefined_identifiers(&child, source, symbols, context));
            return diagnostics; // Only check the first one
        }
        // Recurse into instr_plain to find the index
        if kind == "instr_plain" {
            return find_first_index_identifier(&child, source, symbols, context);
        }
    }

    diagnostics
}

/// Recursively find identifier nodes and check if they're defined
fn find_undefined_identifiers(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    context: &InstructionContext,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    node_kind!(kind = node);

    if kind == "identifier" {
        let raw_name = &source[node.byte_range()];

        // Only check identifiers that start with $
        if !raw_name.starts_with('$') {
            return diagnostics;
        }

        // Normalize quoted identifiers: $"fh" → $fh
        let normalized = normalize_identifier(raw_name);
        let identifier_name: &str = &normalized;

        // Find the containing function for this reference (needed for locals and labels)
        let start_point = node.start_position();
        let position = Position::new(start_point.row as u32, start_point.column as u32);

        let is_defined = match context {
            InstructionContext::Branch | InstructionContext::Block => {
                // Check if label exists in containing function
                if let Some(func) = find_containing_function(symbols, position) {
                    func.blocks.iter().any(|block| {
                        block.label == identifier_name
                            || identifier_name
                                .strip_prefix('$')
                                .is_some_and(|s| s == block.label)
                    })
                } else {
                    false
                }
            }
            InstructionContext::Call => {
                // Check if function exists
                symbols.get_function_by_name(identifier_name).is_some()
            }
            InstructionContext::Local => {
                // Check if local or parameter exists in containing function
                if let Some(func) = find_containing_function(symbols, position) {
                    func.parameters
                        .iter()
                        .any(|p| p.name.as_deref() == Some(identifier_name))
                        || func
                            .locals
                            .iter()
                            .any(|l| l.name.as_deref() == Some(identifier_name))
                } else {
                    false
                }
            }
            InstructionContext::Global => {
                // Check if global exists
                symbols.get_global_by_name(identifier_name).is_some()
            }
            InstructionContext::Table => {
                // Check if table exists
                symbols.get_table_by_name(identifier_name).is_some()
            }
            InstructionContext::Memory => {
                // Check if memory exists
                symbols.get_memory_by_name(identifier_name).is_some()
            }
            InstructionContext::Type => {
                // Check if type exists
                symbols.get_type_by_name(identifier_name).is_some()
            }
            InstructionContext::Tag => {
                // Check if tag exists
                symbols.get_tag_by_name(identifier_name).is_some()
            }
            InstructionContext::Data => {
                // Check if data segment exists
                symbols.get_data_by_name(identifier_name).is_some()
            }
            InstructionContext::Elem => {
                // Check if elem segment exists
                symbols.get_elem_by_name(identifier_name).is_some()
            }
            InstructionContext::Function | InstructionContext::General => true, // Don't flag function definitions or unknowns
        };

        if !is_defined {
            let diagnostic = create_undefined_reference_diagnostic(node, identifier_name, context);
            diagnostics.push(diagnostic);
        }
        return diagnostics;
    }

    // Check numeric indices (nat nodes inside index parents)
    if kind == "nat" {
        // Only validate nat nodes that are inside an index node
        let in_index = node
            .parent()
            .map(|p| {
                node_kind!(pk = p);
                pk == "index"
            })
            .unwrap_or(false);

        if in_index {
            let nat_text = &source[node.byte_range()];
            if let Ok(idx) = nat_text.parse::<usize>() {
                let start_point = node.start_position();
                let position = Position::new(start_point.row as u32, start_point.column as u32);

                let is_valid = match context {
                    InstructionContext::Call => idx < symbols.functions.len(),
                    InstructionContext::Local => {
                        if let Some(func) = find_containing_function(symbols, position) {
                            idx < func.parameters.len() + func.locals.len()
                        } else {
                            true
                        }
                    }
                    InstructionContext::Global => idx < symbols.globals.len(),
                    InstructionContext::Table => idx < symbols.tables.len(),
                    InstructionContext::Memory => idx < symbols.memories.len(),
                    InstructionContext::Tag => idx < symbols.tags.len(),
                    InstructionContext::Data => idx < symbols.data_segments.len(),
                    InstructionContext::Elem => idx < symbols.elem_segments.len(),
                    // Type indices can't be reliably validated because implicit types
                    // (created by function signatures) aren't tracked in the symbol table.
                    // Branch indices are depth-relative, not absolute — skip.
                    InstructionContext::Type
                    | InstructionContext::Branch
                    | InstructionContext::Block
                    | InstructionContext::Function
                    | InstructionContext::General => true,
                };

                if !is_valid {
                    let range = node_to_range(node);
                    let message = match context {
                        InstructionContext::Call => format!("unknown function {}", idx),
                        InstructionContext::Local => format!("unknown local {}", idx),
                        InstructionContext::Global => format!("unknown global {}", idx),
                        InstructionContext::Table => format!("unknown table {}", idx),
                        InstructionContext::Memory => format!("unknown memory {}", idx),
                        InstructionContext::Tag => format!("unknown tag {}", idx),
                        InstructionContext::Data => format!("unknown data segment {}", idx),
                        InstructionContext::Elem => format!("unknown elem segment {}", idx),
                        _ => unreachable!(),
                    };
                    diagnostics.push(Diagnostic::error(range, message));
                }
            }
        }
        return diagnostics;
    }

    // Don't recurse into nested expr nodes - they contain nested instructions
    // that will be checked separately with their own context
    if kind == "expr" {
        return diagnostics;
    }

    // Recursively check children (but not nested expressions)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        diagnostics.extend(find_undefined_identifiers(&child, source, symbols, context));
    }

    diagnostics
}

/// Check if references in this instruction are defined
pub fn check_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    context: &InstructionContext,
) -> Vec<Diagnostic> {
    let text = &source[node.byte_range()];
    let first_token = text.split_whitespace().next().unwrap_or("");

    // For struct.get/struct.set, only the first index is a type reference
    // The second index is a field reference which we don't validate yet
    if *context == InstructionContext::Type && STRUCT_OPS.contains(&first_token) {
        // Only validate the first index child
        return find_first_index_identifier(node, source, symbols, context);
    }

    // Multi-index instructions: table.init / memory.init
    // WAT text format: (table.init $table $elem) or (table.init $elem) — abbreviated
    // With 1 index: it's elem/data (table/memory defaults to 0)
    // With 2 indices: first is table/memory, second is elem/data
    if first_token == "table.init" || first_token == "memory.init" {
        let secondary = if first_token == "table.init" {
            InstructionContext::Table
        } else {
            InstructionContext::Memory
        };
        return check_init_instruction(node, source, symbols, context, &secondary);
    }

    // memory.copy and table.copy take two indices of the same context (dest, src)
    if first_token == "memory.copy" || first_token == "table.copy" {
        return check_multi_index_instruction(node, source, symbols, context, context);
    }

    // array.new_data / array.init_data: first index is Type, second is Data
    if first_token == "array.new_data" || first_token == "array.init_data" {
        return check_multi_index_instruction(
            node,
            source,
            symbols,
            &InstructionContext::Type,
            &InstructionContext::Data,
        );
    }

    // array.new_elem / array.init_elem: first index is Type, second is Elem
    if first_token == "array.new_elem" || first_token == "array.init_elem" {
        return check_multi_index_instruction(
            node,
            source,
            symbols,
            &InstructionContext::Type,
            &InstructionContext::Elem,
        );
    }

    // br_on_cast / br_on_cast_fail: first index is Branch, rest are heap types (not validated)
    if first_token == "br_on_cast" || first_token == "br_on_cast_fail" {
        return check_first_index_reference(node, source, symbols, &InstructionContext::Branch);
    }

    find_undefined_identifiers(node, source, symbols, context)
}

/// Check table.init / memory.init where the index order depends on count:
/// - 1 index: elem/data (table/memory defaults to 0)
/// - 2 indices: first is table/memory, second is elem/data
fn check_init_instruction(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    primary_context: &InstructionContext,   // Elem or Data
    secondary_context: &InstructionContext, // Table or Memory
) -> Vec<Diagnostic> {
    // Count indices first
    let mut total_indices = 0usize;
    find_indices_recursive(node, &mut |_| {
        total_indices += 1;
    });

    let mut diagnostics = Vec::new();
    let mut index_count = 0usize;
    find_indices_recursive(node, &mut |index_node| {
        let ctx = if total_indices == 1 {
            // Abbreviated: single index is primary (elem/data)
            primary_context
        } else if index_count == 0 {
            // Full form: first is secondary (table/memory)
            secondary_context
        } else {
            // Full form: second is primary (elem/data)
            primary_context
        };
        diagnostics.extend(find_undefined_identifiers(index_node, source, symbols, ctx));
        index_count += 1;
    });

    diagnostics
}

/// Check a multi-index instruction where the first and second index have different contexts.
fn check_multi_index_instruction(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    first_context: &InstructionContext,
    second_context: &InstructionContext,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut index_count = 0;

    // Walk the node tree to find index nodes. They may be nested several
    // levels deep: expr1_plain → instr_plain → op_table_init → index
    find_indices_recursive(node, &mut |index_node| {
        let ctx = if index_count == 0 {
            first_context
        } else {
            second_context
        };
        diagnostics.extend(find_undefined_identifiers(index_node, source, symbols, ctx));
        index_count += 1;
    });

    diagnostics
}

/// Recursively find all `index` nodes within instruction wrapper nodes.
/// Recurses into `instr_plain` and `op_*` nodes but stops at `expr` nodes
/// (which are operand sub-expressions, not part of the instruction itself).
fn find_indices_recursive(node: &Node, callback: &mut impl FnMut(&Node)) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "index" {
            callback(&child);
        } else if kind == "instr_plain" || kind.starts_with("op_") {
            find_indices_recursive(&child, callback);
        }
    }
}

/// Create a diagnostic for an undefined reference
fn create_undefined_reference_diagnostic(
    node: &Node,
    identifier_name: &str,
    context: &InstructionContext,
) -> Diagnostic {
    let range = node_to_range(node);

    let message = match context {
        InstructionContext::Branch | InstructionContext::Block => {
            format!("Undefined label '{}'", identifier_name)
        }
        InstructionContext::Call => format!("Undefined function '{}'", identifier_name),
        InstructionContext::Local => format!("Undefined local or parameter '{}'", identifier_name),
        InstructionContext::Global => format!("Undefined global '{}'", identifier_name),
        InstructionContext::Table => format!("Undefined table '{}'", identifier_name),
        InstructionContext::Memory => format!("Undefined memory '{}'", identifier_name),
        InstructionContext::Type => format!("Undefined type '{}'", identifier_name),
        InstructionContext::Tag => format!("Undefined tag '{}'", identifier_name),
        InstructionContext::Data => format!("Undefined data segment '{}'", identifier_name),
        InstructionContext::Elem => format!("Undefined elem segment '{}'", identifier_name),
        InstructionContext::Function | InstructionContext::General => {
            format!("Undefined reference '{}'", identifier_name)
        }
    };

    Diagnostic::error(range, message)
}

/// Check module-level references: export descriptors, table_use, memory_use, elem_list,
/// and abbreviated elem segments (module_field_elem with bare index children).
///
/// These are module-level constructs that reference symbols (functions, globals, tables,
/// memories, tags) but aren't inside instructions, so the normal instruction-context
/// checking doesn't cover them.
pub fn check_module_level_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    node_kind!(kind = node);

    let context = match &*kind {
        "export_desc_func" => InstructionContext::Call,
        "export_desc_global" => InstructionContext::Global,
        "export_desc_table" => InstructionContext::Table,
        "export_desc_memory" => InstructionContext::Memory,
        "export_desc_tag" => InstructionContext::Tag,
        "table_use" => InstructionContext::Table,
        "memory_use" => InstructionContext::Memory,
        "elem_list" | "module_field_elem" => {
            // elem_list: contains index nodes (func $a $b variant) or elem_expr children
            //   (elem_expr contain instructions checked by the tree walk — skip those).
            // module_field_elem: in the abbreviated form `(elem (offset) $a $b)`, bare
            //   index nodes appear directly under module_field_elem without an elem_list.
            //   The first index child (before offset) is the elem segment's own name — skip it.
            let mut diagnostics = Vec::new();
            let mut cursor = node.walk();
            let is_field = &*kind == "module_field_elem";
            let mut seen_offset = false;
            for child in node.children(&mut cursor) {
                node_kind!(child_kind = child);

                if is_field {
                    // Track when we pass the offset — only index nodes after
                    // offset/table_use/elem_list are function references.
                    if child_kind == "offset" {
                        seen_offset = true;
                        continue;
                    }
                    // Skip non-index children and the optional name index before offset
                    if child_kind != "index" || !seen_offset {
                        continue;
                    }
                } else if child_kind != "index" {
                    continue;
                }

                diagnostics.extend(find_undefined_identifiers(
                    &child,
                    source,
                    symbols,
                    &InstructionContext::Call,
                ));
            }
            return diagnostics;
        }
        _ => return vec![],
    };

    // For export descriptors and table_use/memory_use, find the index child
    // and validate its identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(child_kind = child);

        if child_kind == "index" {
            return find_undefined_identifiers(&child, source, symbols, &context);
        }
    }

    vec![]
}

/// Check if a node is nested inside another expr (meaning it can't use stack values)
pub fn is_nested_in_expr(node: &Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        node_kind!(kind = parent);

        if kind == "expr" {
            // Found parent expr, check if its parent is also expr
            if let Some(grandparent) = parent.parent() {
                node_kind!(gp_kind = grandparent);

                if gp_kind == "expr" || gp_kind.starts_with("expr1_") {
                    return true;
                }
            }
        }
        current = parent.parent();
    }
    false
}
