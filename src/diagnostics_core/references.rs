//! Shared reference checking functions for both native and WASM builds.
//!
//! This module provides platform-agnostic functions for checking undefined
//! references in WAT code.
#![allow(clippy::borrow_deref_ref)]

use crate::core::types::{Diagnostic, Position};
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
            #[cfg(feature = "native")]
            let kind = c.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let kind = c.kind();
            kind == "index"
        })
        .collect();

    for (i, index_node) in indices.iter().enumerate() {
        // Find the identifier within the index
        let mut idx_cursor = index_node.walk();
        for child in index_node.children(&mut idx_cursor) {
            #[cfg(feature = "native")]
            let kind = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let kind = child.kind();

            if kind == "identifier" {
                let identifier_name = &source[child.byte_range()];
                if !identifier_name.starts_with('$') {
                    continue;
                }

                let start_point = child.start_position();
                let position = Position::new(start_point.row as u32, start_point.column as u32);

                // First index is tag (if has_tag), remaining are labels
                let is_tag_reference = has_tag && i == 0;

                let is_defined = if is_tag_reference {
                    symbols.get_tag_by_name(identifier_name).is_some()
                } else {
                    // Label reference - check block labels in containing function
                    if let Some(func) = find_containing_function(symbols, position) {
                        func.blocks.iter().any(|block| {
                            format!("${}", block.label) == identifier_name
                                || block.label == identifier_name
                        })
                    } else {
                        false
                    }
                };

                if !is_defined {
                    let context = if is_tag_reference {
                        InstructionContext::Tag
                    } else {
                        InstructionContext::Branch
                    };
                    let diagnostic =
                        create_undefined_reference_diagnostic(&child, identifier_name, &context);
                    diagnostics.push(diagnostic);
                }
            }
        }
    }

    diagnostics
}

/// Check references in a try_catch_clause node (legacy try syntax)
/// For (catch $tag instr*): the index is a tag reference
pub fn check_try_catch_clause_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Find the index child which contains the tag reference
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "index" {
            // Find the identifier within the index
            let mut idx_cursor = child.walk();
            for idx_child in child.children(&mut idx_cursor) {
                #[cfg(feature = "native")]
                let idx_kind = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let idx_kind = idx_child.kind();

                if idx_kind == "identifier" {
                    let identifier_name = &source[idx_child.byte_range()];
                    if !identifier_name.starts_with('$') {
                        continue;
                    }

                    // Check if the tag is defined
                    if symbols.get_tag_by_name(identifier_name).is_none() {
                        let diagnostic = create_undefined_reference_diagnostic(
                            &idx_child,
                            identifier_name,
                            &InstructionContext::Tag,
                        );
                        diagnostics.push(diagnostic);
                    }
                }
            }
            // Only one index in try_catch_clause
            break;
        }
    }

    diagnostics
}

/// Check references in a try_delegate_clause node (legacy try syntax)
/// For (delegate $label): the index is a label reference
pub fn check_try_delegate_clause_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Find the index child which contains the label reference
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "index" {
            // Find the identifier within the index
            let mut idx_cursor = child.walk();
            for idx_child in child.children(&mut idx_cursor) {
                #[cfg(feature = "native")]
                let idx_kind = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let idx_kind = idx_child.kind();

                if idx_kind == "identifier" {
                    let identifier_name = &source[idx_child.byte_range()];
                    if !identifier_name.starts_with('$') {
                        continue;
                    }

                    let start_point = idx_child.start_position();
                    let position = Position::new(start_point.row as u32, start_point.column as u32);

                    // Check if the label is defined
                    let is_defined = if let Some(func) = find_containing_function(symbols, position)
                    {
                        func.blocks.iter().any(|block| {
                            format!("${}", block.label) == identifier_name
                                || block.label == identifier_name
                        })
                    } else {
                        false
                    };

                    if !is_defined {
                        let diagnostic = create_undefined_reference_diagnostic(
                            &idx_child,
                            identifier_name,
                            &InstructionContext::Branch,
                        );
                        diagnostics.push(diagnostic);
                    }
                }
            }
            // Only one index in try_delegate_clause
            break;
        }
    }

    diagnostics
}

/// Check the function reference in a module_field_start node.
/// The start directive takes a single function index: (start $func) or (start 0)
pub fn check_start_references(node: &Node, source: &str, symbols: &SymbolTable) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "index" {
            let mut idx_cursor = child.walk();
            for idx_child in child.children(&mut idx_cursor) {
                #[cfg(feature = "native")]
                let idx_kind = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let idx_kind = idx_child.kind();

                if idx_kind == "identifier" {
                    let identifier_name = &source[idx_child.byte_range()];
                    if !identifier_name.starts_with('$') {
                        continue;
                    }

                    if symbols.get_function_by_name(identifier_name).is_none() {
                        let diagnostic = create_undefined_reference_diagnostic(
                            &idx_child,
                            identifier_name,
                            &InstructionContext::Call,
                        );
                        diagnostics.push(diagnostic);
                    }
                }
            }
            break;
        }
    }

    diagnostics
}

/// Find and validate only the first index identifier in a node
/// Used for instructions like struct.get where only the first index is a type
pub fn find_first_index_identifier(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    context: &InstructionContext,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

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
pub fn find_undefined_identifiers(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    context: &InstructionContext,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    #[cfg(feature = "native")]
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = node.kind();

    if kind == "identifier" {
        let identifier_name = &source[node.byte_range()];

        // Only check identifiers that start with $
        if !identifier_name.starts_with('$') {
            return diagnostics;
        }

        // Find the containing function for this reference (needed for locals and labels)
        let start_point = node.start_position();
        let position = Position::new(start_point.row as u32, start_point.column as u32);

        let is_defined = match context {
            InstructionContext::Branch | InstructionContext::Block => {
                // Check if label exists in containing function
                if let Some(func) = find_containing_function(symbols, position) {
                    func.blocks.iter().any(|block| {
                        format!("${}", block.label) == identifier_name
                            || block.label == identifier_name
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
                        .any(|p| p.name.as_ref() == Some(&identifier_name.to_string()))
                        || func
                            .locals
                            .iter()
                            .any(|l| l.name.as_ref() == Some(&identifier_name.to_string()))
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

    // Multi-index instructions: table.init takes (elem_idx, table_idx),
    // memory.init takes (data_idx, memory_idx). The context from
    // determine_instruction_context_at_node applies only to the first index.
    if first_token == "table.init" || first_token == "memory.init" {
        let second_context = if first_token == "table.init" {
            InstructionContext::Table
        } else {
            InstructionContext::Memory
        };
        return check_multi_index_instruction(node, source, symbols, context, &second_context);
    }

    find_undefined_identifiers(node, source, symbols, context)
}

/// Check a multi-index instruction where the first and second index have different contexts.
/// E.g. `table.init $elem $table` or `memory.init $data $memory`.
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
pub fn create_undefined_reference_diagnostic(
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
    #[cfg(feature = "native")]
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = node.kind();

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
                #[cfg(feature = "native")]
                let child_kind = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let child_kind = child.kind();

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
        #[cfg(feature = "native")]
        let child_kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let child_kind = child.kind();

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
        #[cfg(feature = "native")]
        let kind = parent.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = parent.kind();

        if kind == "expr" {
            // Found parent expr, check if its parent is also expr
            if let Some(grandparent) = parent.parent() {
                #[cfg(feature = "native")]
                let gp_kind = grandparent.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let gp_kind = grandparent.kind();

                if gp_kind == "expr" || gp_kind.starts_with("expr1_") {
                    return true;
                }
            }
        }
        current = parent.parent();
    }
    false
}
