use crate::core::types::{Position, Range};
use crate::symbols::*;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::{Node, Tree};

#[cfg(feature = "native")]
use crate::tree_sitter_bindings::create_parser;

// For WASM, we'll use the facade types
#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::{Node, Tree};

#[cfg(test)]
mod tests;

/// Parse a WAT document and extract symbols (PUBLIC API - unchanged)
#[cfg(feature = "native")]
pub fn parse_document(text: &str) -> Result<SymbolTable, String> {
    let mut parser = create_parser();
    let tree = parser
        .parse(text, None)
        .ok_or_else(|| "Failed to parse document".to_string())?;

    extract_symbols(&tree, text)
}

/// Parse a WAT document from a pre-parsed tree (works for both native and WASM)
pub fn parse_document_from_tree(tree: &Tree, text: &str) -> Result<SymbolTable, String> {
    extract_symbols(tree, text)
}

/// Extract all symbols from the parse tree
fn extract_symbols(tree: &Tree, source: &str) -> Result<SymbolTable, String> {
    let mut symbol_table = SymbolTable::new();
    let root = tree.root_node();

    // Extract imports FIRST - imports get indices before regular declarations
    // Returns counters for each kind of import
    let import_counts = extract_imports(&root, source, &mut symbol_table);

    // Extract in order: globals, types, tables, memories, functions
    // (Order matters for index assignment)
    // Pass import counts to offset the indices
    extract_globals_with_offset(&root, source, &mut symbol_table, import_counts.globals);
    extract_types(&root, source, &mut symbol_table);
    extract_tables_with_offset(&root, source, &mut symbol_table, import_counts.tables);
    extract_memories_with_offset(&root, source, &mut symbol_table, import_counts.memories);
    extract_tags_with_offset(&root, source, &mut symbol_table, import_counts.tags);
    extract_functions_with_offset(&root, source, &mut symbol_table, import_counts.functions);

    // Extract data and elem segments
    extract_data_segments(&root, source, &mut symbol_table);
    extract_elem_segments(&root, source, &mut symbol_table);

    Ok(symbol_table)
}

/// Counters for imported items
struct ImportCounts {
    functions: usize,
    globals: usize,
    tables: usize,
    memories: usize,
    tags: usize,
}

/// Extract imports from the module
fn extract_imports(root: &Node, source: &str, symbol_table: &mut SymbolTable) -> ImportCounts {
    let mut counts = ImportCounts {
        functions: 0,
        globals: 0,
        tables: 0,
        memories: 0,
        tags: 0,
    };

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_import" {
                            extract_single_import(&field_child, source, symbol_table, &mut counts);
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_import" {
                    extract_single_import(&field_child, source, symbol_table, &mut counts);
                }
            }
        }
    }

    counts
}

/// Extract a single import declaration
fn extract_single_import(
    import_node: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    counts: &mut ImportCounts,
) {
    let mut cursor = import_node.walk();

    // Find the import_desc wrapper which contains the actual import descriptor
    for child in import_node.children(&mut cursor) {
        if child.kind() == "import_desc" {
            // Look inside import_desc for the specific import type
            let mut desc_cursor = child.walk();
            for desc_child in child.children(&mut desc_cursor) {
                let kind = desc_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let kind = kind.as_str();
                match kind {
                    "import_desc_func_type" | "import_desc_type_use" => {
                        // Imported function: (func $name? ...)
                        if let Some(func) =
                            extract_imported_function(&desc_child, source, counts.functions)
                        {
                            symbol_table.add_function(func);
                            counts.functions += 1;
                        }
                    }
                    "import_desc_global_type" => {
                        // Imported global: (global $name? global_type)
                        if let Some(global) =
                            extract_imported_global(&desc_child, source, counts.globals)
                        {
                            symbol_table.add_global(global);
                            counts.globals += 1;
                        }
                    }
                    "import_desc_table_type" => {
                        // Imported table: (table $name? table_type)
                        if let Some(table) =
                            extract_imported_table(&desc_child, source, counts.tables)
                        {
                            symbol_table.add_table(table);
                            counts.tables += 1;
                        }
                    }
                    "import_desc_memory_type" => {
                        // Imported memory: (memory $name? memory_type)
                        if let Some(memory) =
                            extract_imported_memory(&desc_child, source, counts.memories)
                        {
                            symbol_table.add_memory(memory);
                            counts.memories += 1;
                        }
                    }
                    "import_desc_tag_type" => {
                        // Imported tag: (tag $name? (param ...)?)
                        if let Some(tag) = extract_imported_tag(&desc_child, source, counts.tags) {
                            symbol_table.add_tag(tag);
                            counts.tags += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Extract an imported function
fn extract_imported_function(desc_node: &Node, source: &str, index: usize) -> Option<Function> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(desc_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    // For imports, parameters and results can be in multiple func_type nodes:
    // import_desc_func_type -> func_type (params) + func_type (results)
    // Or they could be combined. We need to accumulate all params and results.
    let mut parameters = Vec::new();
    let mut results = Vec::new();

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "func_type" {
            // Extract from the func_type node and accumulate
            let new_params = extract_parameters(&child, source);
            let new_results = extract_results(&child, source);
            if !new_params.is_empty() {
                // Re-index parameters to continue the sequence
                let base_idx = parameters.len();
                for mut p in new_params {
                    p.index += base_idx;
                    parameters.push(p);
                }
            }
            if !new_results.is_empty() {
                results.extend(new_results);
            }
        } else if child.kind() == "func_type_params" {
            // Direct params
            let new_params = extract_parameters(desc_node, source);
            if !new_params.is_empty() {
                let base_idx = parameters.len();
                for mut p in new_params {
                    p.index += base_idx;
                    parameters.push(p);
                }
            }
        } else if child.kind() == "func_type_results" {
            let new_results = extract_results(desc_node, source);
            results.extend(new_results);
        }
    }

    // Also check for params directly under desc_node (fallback)
    if parameters.is_empty() {
        parameters = extract_parameters(desc_node, source);
    }
    if results.is_empty() {
        results = extract_results(desc_node, source);
    }

    Some(Function {
        name,
        index,
        parameters,
        results,
        locals: Vec::new(), // Imported functions have no locals
        blocks: Vec::new(), // Imported functions have no blocks
        line: desc_node.range().start_point.row as u32,
        end_line: desc_node.range().end_point.row as u32,
        start_byte: desc_node.start_byte(),
        end_byte: desc_node.end_byte(),
        range: name_range,
    })
}

/// Extract an imported global
fn extract_imported_global(desc_node: &Node, source: &str, index: usize) -> Option<Global> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(desc_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut is_mutable = false;
    let mut var_type = ValueType::Unknown;

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "global_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "global_type_mut" {
                    is_mutable = true;
                    let mut mut_cursor = type_child.walk();
                    for mut_child in type_child.children(&mut mut_cursor) {
                        if mut_child.kind() == "value_type" {
                            var_type = extract_value_type(&mut_child, source);
                        }
                    }
                } else if type_child.kind() == "value_type" {
                    var_type = extract_value_type(&type_child, source);
                }
            }
        }
    }

    Some(Global {
        name,
        index,
        var_type,
        is_mutable,
        initial_value: None,
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract an imported table
fn extract_imported_table(desc_node: &Node, source: &str, index: usize) -> Option<Table> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(desc_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut ref_type = ValueType::Funcref;
    let mut min_limit = 0;
    let mut max_limit = None;

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "table_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "limits" {
                    let mut limits_cursor = type_child.walk();
                    let mut nat_index = 0;
                    for limit_child in type_child.children(&mut limits_cursor) {
                        if limit_child.kind() == "nat" {
                            let text = node_text(&limit_child, source);
                            if let Ok(num) = text.parse::<u32>() {
                                if nat_index == 0 {
                                    min_limit = num;
                                } else {
                                    max_limit = Some(num);
                                }
                                nat_index += 1;
                            }
                        }
                    }
                } else if type_child.kind() == "ref_type" {
                    ref_type = extract_ref_type(&type_child, source);
                }
            }
        }
    }

    Some(Table {
        name,
        index,
        ref_type,
        limits: (min_limit, max_limit),
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract an imported memory
fn extract_imported_memory(desc_node: &Node, source: &str, index: usize) -> Option<Memory> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(desc_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut min_limit = 0;
    let mut max_limit = None;

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "memory_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "limits" {
                    let mut limits_cursor = type_child.walk();
                    let mut nat_index = 0;
                    for limit_child in type_child.children(&mut limits_cursor) {
                        if limit_child.kind() == "nat" {
                            let text = node_text(&limit_child, source);
                            if let Ok(num) = text.parse::<u32>() {
                                if nat_index == 0 {
                                    min_limit = num;
                                } else {
                                    max_limit = Some(num);
                                }
                                nat_index += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Some(Memory {
        name,
        index,
        limits: (min_limit, max_limit),
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract an imported tag
fn extract_imported_tag(desc_node: &Node, source: &str, index: usize) -> Option<Tag> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(desc_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut params = Vec::new();

    // Look for func_type_params in the import_desc_tag_type node
    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "func_type_params" {
            let mut params_cursor = child.walk();
            for param_child in child.children(&mut params_cursor) {
                if param_child.kind() == "func_type_params_one"
                    || param_child.kind() == "func_type_params_many"
                {
                    let mut param_type_cursor = param_child.walk();
                    for type_child in param_child.children(&mut param_type_cursor) {
                        if type_child.kind() == "value_type" {
                            params.push(extract_value_type(&type_child, source));
                        }
                    }
                } else if param_child.kind() == "value_type" {
                    // Direct value_type children
                    params.push(extract_value_type(&param_child, source));
                }
            }
        }
    }

    Some(Tag {
        name,
        index,
        params,
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Convert a tree-sitter node to an LSP Range
fn node_to_range(node: &Node) -> Range {
    let start_point = node.range().start_point;
    let end_point = node.range().end_point;
    Range {
        start: Position {
            line: start_point.row as u32,
            character: start_point.column as u32,
        },
        end: Position {
            line: end_point.row as u32,
            character: end_point.column as u32,
        },
    }
}

/// Find identifier child node (returns the node itself, not just text)
#[cfg(feature = "native")]
fn find_identifier_node<'a>(node: &'a Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    #[allow(clippy::manual_find)]
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(child);
        }
    }
    None
}

/// Find identifier child node (WASM version - no lifetime parameter needed)
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_identifier_node(node: &Node) -> Option<Node> {
    let mut cursor = node.walk();
    #[allow(clippy::manual_find)]
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(child);
        }
    }
    None
}

/// Extract function symbols using tree-sitter AST traversal (with offset for imports)
fn extract_functions_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut func_index = start_index;

    // Track seen function names to deduplicate (error recovery can create duplicates)
    // Also track by byte range for unnamed functions
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_ranges: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();

    // Walk through all children of root (could be module or direct module_field)
    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            // Inside module, look for module_field children
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_func" {
                            if let Some(func) = extract_function(&field_child, source, func_index) {
                                // Skip duplicates from error recovery
                                let is_duplicate = if let Some(ref name) = func.name {
                                    seen_names.contains(name)
                                } else {
                                    let range = (func.start_byte, func.end_byte);
                                    seen_ranges.contains(&range)
                                };

                                if !is_duplicate {
                                    if let Some(ref name) = func.name {
                                        seen_names.insert(name.clone());
                                    } else {
                                        seen_ranges.insert((func.start_byte, func.end_byte));
                                    }
                                    symbol_table.add_function(func);
                                    func_index += 1;
                                }
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            // Direct module_field (for standalone functions)
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_func" {
                    if let Some(func) = extract_function(&field_child, source, func_index) {
                        let is_duplicate = if let Some(ref name) = func.name {
                            seen_names.contains(name)
                        } else {
                            let range = (func.start_byte, func.end_byte);
                            seen_ranges.contains(&range)
                        };

                        if !is_duplicate {
                            if let Some(ref name) = func.name {
                                seen_names.insert(name.clone());
                            } else {
                                seen_ranges.insert((func.start_byte, func.end_byte));
                            }
                            symbol_table.add_function(func);
                            func_index += 1;
                        }
                    }
                }
            }
        }
    }
}

/// Extract a single function from a func node
fn extract_function(func_node: &Node, source: &str, index: usize) -> Option<Function> {
    let range = func_node.range();

    // Try to find function name and its range
    let (name, name_range) = if let Some(id_node) = find_identifier_node(func_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    // Extract parameters, results, locals, and blocks
    let parameters = extract_parameters(func_node, source);
    let results = extract_results(func_node, source);
    let locals = extract_locals(func_node, source);
    let blocks = extract_blocks(func_node, source);

    Some(Function {
        name,
        index,
        parameters,
        results,
        locals,
        blocks,
        line: range.start_point.row as u32,
        end_line: range.end_point.row as u32,
        start_byte: func_node.start_byte(),
        end_byte: func_node.end_byte(),
        range: name_range,
    })
}

/// Extract parameters from a function node
fn extract_parameters(func_node: &Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    let mut cursor = func_node.walk();
    let mut param_index = 0;

    // Traverse child nodes looking for func_type_params
    for child in func_node.children(&mut cursor) {
        if child.kind() == "func_type_params" {
            // Extract param from func_type_params node
            let mut params_cursor = child.walk();
            for param_child in child.children(&mut params_cursor) {
                if param_child.kind() == "func_type_params_one" {
                    // Named parameter: (param $name type)
                    let mut name_opt = None;
                    let mut type_opt = None;
                    let mut range_opt = None;

                    let mut one_cursor = param_child.walk();
                    for one_child in param_child.children(&mut one_cursor) {
                        if one_child.kind() == "identifier" {
                            name_opt = Some(node_text(&one_child, source));
                            range_opt = Some(node_to_range(&one_child));
                        } else if one_child.kind() == "value_type" {
                            type_opt = Some(extract_value_type(&one_child, source));
                        }
                    }

                    if let Some(param_type) = type_opt {
                        parameters.push(Parameter {
                            name: name_opt,
                            param_type,
                            index: param_index,
                            range: range_opt,
                        });
                        param_index += 1;
                    }
                } else if param_child.kind() == "func_type_params_many" {
                    // Unnamed parameters: (param type type ...)
                    let mut many_cursor = param_child.walk();
                    for many_child in param_child.children(&mut many_cursor) {
                        if many_child.kind() == "value_type" {
                            parameters.push(Parameter {
                                name: None,
                                param_type: extract_value_type(&many_child, source),
                                index: param_index,
                                range: None,
                            });
                            param_index += 1;
                        }
                    }
                }
            }
        }
    }

    parameters
}

/// Extract result types from a function node
fn extract_results(func_node: &Node, source: &str) -> Vec<ValueType> {
    let mut results = Vec::new();
    let mut cursor = func_node.walk();

    for child in func_node.children(&mut cursor) {
        if child.kind() == "func_type_results" {
            // Extract value_type from func_type_results
            let mut results_cursor = child.walk();
            for result_child in child.children(&mut results_cursor) {
                if result_child.kind() == "value_type" {
                    results.push(extract_value_type(&result_child, source));
                }
            }
        }
    }

    results
}

/// Extract local variables from a function node
fn extract_locals(func_node: &Node, source: &str) -> Vec<Variable> {
    let mut locals = Vec::new();
    let mut cursor = func_node.walk();
    let mut local_index = 0;

    for child in func_node.children(&mut cursor) {
        if child.kind() == "func_locals" {
            // func_locals can be func_locals_one or func_locals_many
            let mut locals_cursor = child.walk();
            for locals_child in child.children(&mut locals_cursor) {
                if locals_child.kind() == "func_locals_one" {
                    // Named local: (local $name type)
                    let mut name_opt = None;
                    let mut type_opt = None;
                    let mut range_opt = None;

                    let mut one_cursor = locals_child.walk();
                    for one_child in locals_child.children(&mut one_cursor) {
                        if one_child.kind() == "identifier" {
                            name_opt = Some(node_text(&one_child, source));
                            range_opt = Some(node_to_range(&one_child));
                        } else if one_child.kind() == "value_type" {
                            type_opt = Some(extract_value_type(&one_child, source));
                        }
                    }

                    if let Some(var_type) = type_opt {
                        locals.push(Variable {
                            name: name_opt,
                            var_type,
                            is_mutable: true,
                            initial_value: None,
                            index: local_index,
                            range: range_opt,
                        });
                        local_index += 1;
                    }
                } else if locals_child.kind() == "func_locals_many" {
                    // Unnamed locals: (local type type ...)
                    let mut many_cursor = locals_child.walk();
                    for many_child in locals_child.children(&mut many_cursor) {
                        if many_child.kind() == "value_type" {
                            locals.push(Variable {
                                name: None,
                                var_type: extract_value_type(&many_child, source),
                                is_mutable: true,
                                initial_value: None,
                                index: local_index,
                                range: None,
                            });
                            local_index += 1;
                        }
                    }
                }
            }
        }
    }

    locals
}

/// Extract block labels from a function node
fn extract_blocks(func_node: &Node, source: &str) -> Vec<BlockLabel> {
    let mut blocks = Vec::new();

    // Recursively traverse function body looking for labeled blocks
    visit_node_for_blocks(func_node, source, &mut blocks);

    blocks
}

/// Recursively visit nodes to find labeled blocks/loops/ifs
fn visit_node_for_blocks(node: &Node, source: &str, blocks: &mut Vec<BlockLabel>) {
    let kind = node.kind();

    // Check both statement form (block_block) and expression form (expr1_block)
    if kind == "block_block"
        || kind == "block_loop"
        || kind == "block_if"
        || kind == "block_try"
        || kind == "block_try_table"
        || kind == "expr1_block"
        || kind == "expr1_loop"
        || kind == "expr1_if"
        || kind == "expr1_try"
    {
        // Check if it has a label
        if let Some(id_node) = find_identifier_node(node) {
            let label = node_text(&id_node, source);
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let kind = kind.as_str();
            let block_type = match kind {
                "block_block" | "expr1_block" => "block",
                "block_loop" | "expr1_loop" => "loop",
                "block_if" | "expr1_if" => "if",
                "block_try" | "expr1_try" => "try",
                "block_try_table" => "try_table",
                _ => "unknown",
            }
            .to_string();

            blocks.push(BlockLabel {
                label,
                block_type,
                line: node.range().start_point.row as u32,
                range: Some(node_to_range(&id_node)),
            });
        }
    }

    // Recurse to children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_node_for_blocks(&child, source, blocks);
    }
}

/// Extract global variables (with offset for imports)
fn extract_globals_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut global_index = start_index;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_global" {
                            if let Some(global) = extract_global(&field_child, source, global_index)
                            {
                                symbol_table.add_global(global);
                                global_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_global" {
                    if let Some(global) = extract_global(&field_child, source, global_index) {
                        symbol_table.add_global(global);
                        global_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single global from a global node
fn extract_global(global_node: &Node, source: &str, index: usize) -> Option<Global> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(global_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut is_mutable = false;
    let mut var_type = ValueType::Unknown;

    let mut cursor = global_node.walk();
    for child in global_node.children(&mut cursor) {
        if child.kind() == "global_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "global_type_mut" {
                    is_mutable = true;
                    // value_type is nested inside global_type_mut
                    let mut mut_cursor = type_child.walk();
                    for mut_child in type_child.children(&mut mut_cursor) {
                        if mut_child.kind() == "value_type" {
                            var_type = extract_value_type(&mut_child, source);
                        }
                    }
                } else if type_child.kind() == "value_type" {
                    // Non-mutable globals have value_type directly under global_type
                    var_type = extract_value_type(&type_child, source);
                }
            }
        }
    }

    Some(Global {
        name,
        index,
        var_type,
        is_mutable,
        initial_value: None,
        line: global_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract type definitions
fn extract_types(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut type_index = 0;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_type" {
                            if let Some(type_def) = extract_type(&field_child, source, type_index) {
                                symbol_table.add_type(type_def);
                                type_index += 1;
                            }
                        } else if field_child.kind() == "module_field_rec" {
                            // Extract types from rec group
                            type_index =
                                extract_rec_types(&field_child, source, symbol_table, type_index);
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_type" {
                    if let Some(type_def) = extract_type(&field_child, source, type_index) {
                        symbol_table.add_type(type_def);
                        type_index += 1;
                    }
                } else if field_child.kind() == "module_field_rec" {
                    // Extract types from rec group
                    type_index = extract_rec_types(&field_child, source, symbol_table, type_index);
                }
            }
        }
    }
}

/// Extract types from a rec group
fn extract_rec_types(
    rec_node: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    mut type_index: usize,
) -> usize {
    // The rec node's children are flattened: "(", "rec", "(", "type", id?, type_field, ")", "(", "type", ..., ")", ")"
    // We need to parse this as a sequence, looking for patterns: "(" "type" [id] type_field ")"

    let mut cursor = rec_node.walk();
    let children_vec: Vec<_> = rec_node.children(&mut cursor).collect();

    let mut i = 0;
    while i < children_vec.len() {
        let child = &children_vec[i];

        // Look for pattern: "(" followed by "type"
        if child.kind() == "(" && i + 1 < children_vec.len() && children_vec[i + 1].kind() == "type"
        {
            // Found start of a type definition
            i += 2; // Skip "(" and "type"

            // Check for optional identifier
            let (name, name_range) =
                if i < children_vec.len() && children_vec[i].kind() == "identifier" {
                    let id_node = &children_vec[i];
                    i += 1;
                    (
                        Some(node_text(id_node, source)),
                        Some(node_to_range(id_node)),
                    )
                } else {
                    (None, None)
                };

            // Next should be the type_field or direct struct_type/array_type
            if i < children_vec.len() {
                let type_node = &children_vec[i];
                if type_node.kind() == "struct_type"
                    || type_node.kind() == "array_type"
                    || type_node.kind() == "type_field"
                {
                    // Extract the type
                    if let Some(type_def) = extract_type_from_single_node(
                        type_node, source, type_index, name, name_range,
                    ) {
                        symbol_table.add_type(type_def);
                        type_index += 1;
                    }
                    i += 1;
                }
            }

            // Skip until we find the closing ")"
            while i < children_vec.len() && children_vec[i].kind() != ")" {
                i += 1;
            }
            if i < children_vec.len() && children_vec[i].kind() == ")" {
                i += 1; // Skip the ")"
            }
        } else {
            i += 1;
        }
    }

    type_index
}

/// Extract a type from a single node (struct_type, array_type, or type_field)
fn extract_type_from_single_node(
    type_node: &Node,
    source: &str,
    index: usize,
    name: Option<String>,
    name_range: Option<Range>,
) -> Option<TypeDef> {
    let mut kind = TypeKind::Func {
        params: Vec::new(),
        results: Vec::new(),
    };

    let type_kind = type_node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let type_kind = type_kind.as_str();
    match type_kind {
        "struct_type" => {
            // Extract struct fields
            let mut fields = Vec::new();
            let mut cursor = type_node.walk();
            for child in type_node.children(&mut cursor) {
                if child.kind() == "field_type" {
                    let (field_name, field_type, mutable) = extract_field_type(&child, source);
                    fields.push((field_name, field_type, mutable));
                }
            }
            kind = TypeKind::Struct { fields };
        }
        "array_type" => {
            // Extract array field
            let mut element_type = ValueType::Unknown;
            let mut mutable = false;
            let mut cursor = type_node.walk();
            for child in type_node.children(&mut cursor) {
                if child.kind() == "field_type" {
                    let (_, field_type, mut_flag) = extract_field_type(&child, source);
                    element_type = field_type;
                    mutable = mut_flag;
                }
            }
            kind = TypeKind::Array {
                element_type,
                mutable,
            };
        }
        "type_field" => {
            // This is a func type inside a type_field wrapper
            let mut parameters = Vec::new();
            let mut results = Vec::new();
            let mut cursor = type_node.walk();
            for child in type_node.children(&mut cursor) {
                if child.kind() == "func_type" {
                    let params = extract_parameters(&child, source);
                    for param in params {
                        parameters.push(param.param_type);
                    }
                    results.extend(extract_results(&child, source));
                }
            }
            if !parameters.is_empty() || !results.is_empty() {
                kind = TypeKind::Func {
                    params: parameters,
                    results,
                };
            }
        }
        _ => return None,
    }

    Some(TypeDef {
        name,
        index,
        kind,
        line: type_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract field_type information (name, type, mutability)
fn extract_field_type(field_node: &Node, source: &str) -> (Option<String>, ValueType, bool) {
    let mut field_name = None;
    let mut field_type = ValueType::Unknown;
    let mut mutable = false;

    let mut cursor = field_node.walk();
    for child in field_node.children(&mut cursor) {
        if child.kind() == "identifier" {
            field_name = Some(node_text(&child, source));
        } else if child.kind() == "value_type" {
            field_type = extract_value_type(&child, source);
        }
    }

    let text = node_text(field_node, source);
    if text.contains("(mut") || text.contains(" mut ") {
        mutable = true;
    }

    (field_name, field_type, mutable)
}

/// Extract a single type definition from a type node
fn extract_type(type_node: &Node, source: &str, index: usize) -> Option<TypeDef> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(type_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut parameters = Vec::new();
    let mut results = Vec::new();
    let mut kind = TypeKind::Func {
        params: Vec::new(),
        results: Vec::new(),
    };

    let mut cursor = type_node.walk();
    for child in type_node.children(&mut cursor) {
        // Check if this child is directly a struct_type or array_type
        // (happens when type_field is just the type itself)
        if child.kind() == "struct_type" {
            // Extract struct fields directly
            let mut fields = Vec::new();
            let mut struct_cursor = child.walk();

            for struct_child in child.children(&mut struct_cursor) {
                if struct_child.kind() == "field_type" {
                    let mut field_name = None;
                    let mut field_type = ValueType::Unknown;
                    let mut mutable = false;

                    let mut fc = struct_child.walk();
                    for c in struct_child.children(&mut fc) {
                        if c.kind() == "identifier" {
                            field_name = Some(node_text(&c, source));
                        } else if c.kind() == "value_type" {
                            field_type = extract_value_type(&c, source);
                        }
                    }

                    let text = node_text(&struct_child, source);
                    if text.contains("(mut") || text.contains(" mut ") {
                        mutable = true;
                    }

                    fields.push((field_name, field_type, mutable));
                }
            }
            kind = TypeKind::Struct { fields };
        } else if child.kind() == "array_type" {
            // Extract array field directly
            let mut element_type = ValueType::Unknown;
            let mut mutable = false;

            let mut array_cursor = child.walk();
            for array_child in child.children(&mut array_cursor) {
                if array_child.kind() == "field_type" {
                    let mut fc = array_child.walk();
                    for c in array_child.children(&mut fc) {
                        if c.kind() == "value_type" {
                            element_type = extract_value_type(&c, source);
                        }
                    }
                    let text = node_text(&array_child, source);
                    if text.contains("(mut") || text.contains(" mut ") {
                        mutable = true;
                    }
                }
            }
            kind = TypeKind::Array {
                element_type,
                mutable,
            };
        } else if child.kind() == "type_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "func_type" {
                    // Extract parameters and results from func_type
                    // Accumulate across all func_type children
                    let params = extract_parameters(&field_child, source);
                    for param in params {
                        parameters.push(param.param_type);
                    }
                    results.extend(extract_results(&field_child, source));
                } else if field_child.kind() == "struct_type" {
                    // Extract struct fields
                    let mut fields = Vec::new();
                    let mut struct_cursor = field_child.walk();

                    for struct_child in field_child.children(&mut struct_cursor) {
                        if struct_child.kind() == "field_type" {
                            // field_type: (field $id? type ...)
                            let mut field_name = None;
                            let mut field_type = ValueType::Unknown;
                            let mut mutable = false;

                            let mut fc = struct_child.walk();
                            for c in struct_child.children(&mut fc) {
                                if c.kind() == "identifier" {
                                    field_name = Some(node_text(&c, source));
                                } else if c.kind() == "value_type" {
                                    field_type = extract_value_type(&c, source);
                                } else if c.kind() == "mut" {
                                    // If we see 'mut' directly (depends on grammar)
                                    mutable = true;
                                }
                                // Check for (mut type) pattern if it exists as a node
                                // Current grammar is tricky, doing best effort
                            }

                            // Check source text for "mut" if structure is unclear
                            let text = node_text(&struct_child, source);
                            if text.contains("(mut") || text.contains(" mut ") {
                                mutable = true;
                            }

                            fields.push((field_name, field_type, mutable));
                        }
                    }
                    kind = TypeKind::Struct { fields };
                } else if field_child.kind() == "array_type" {
                    // Extract array field
                    // array_type: (array field_type)
                    let mut element_type = ValueType::Unknown;
                    let mut mutable = false;

                    let mut array_cursor = field_child.walk();
                    for array_child in field_child.children(&mut array_cursor) {
                        if array_child.kind() == "field_type" {
                            // Extract type from field_type
                            let mut fc = array_child.walk();
                            for c in array_child.children(&mut fc) {
                                if c.kind() == "value_type" {
                                    element_type = extract_value_type(&c, source);
                                }
                            }
                            let text = node_text(&array_child, source);
                            if text.contains("(mut") || text.contains(" mut ") {
                                mutable = true;
                            }
                        }
                    }
                    kind = TypeKind::Array {
                        element_type,
                        mutable,
                    };
                }
            }
        }
    }

    // Update kind if we accumulated func parameters/results
    if !parameters.is_empty() || !results.is_empty() {
        kind = TypeKind::Func {
            params: parameters,
            results,
        };
    }

    Some(TypeDef {
        name,
        index,
        kind,
        line: type_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract table definitions (with offset for imports)
fn extract_tables_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut table_index = start_index;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_table" {
                            if let Some(table) = extract_table(&field_child, source, table_index) {
                                symbol_table.add_table(table);
                                table_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_table" {
                    if let Some(table) = extract_table(&field_child, source, table_index) {
                        symbol_table.add_table(table);
                        table_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single table from a table node
fn extract_table(table_node: &Node, source: &str, index: usize) -> Option<Table> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(table_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut ref_type = ValueType::Funcref;
    let mut min_limit = 0;
    let mut max_limit = None;

    let mut cursor = table_node.walk();
    for child in table_node.children(&mut cursor) {
        // Handle table_fields_type wrapper
        if child.kind() == "table_fields_type" {
            let mut fields_cursor = child.walk();
            for fields_child in child.children(&mut fields_cursor) {
                if fields_child.kind() == "table_type" {
                    let mut type_cursor = fields_child.walk();
                    for type_child in fields_child.children(&mut type_cursor) {
                        if type_child.kind() == "limits" {
                            let mut limits_cursor = type_child.walk();
                            let mut nat_index = 0;
                            for limit_child in type_child.children(&mut limits_cursor) {
                                if limit_child.kind() == "nat" {
                                    let text = node_text(&limit_child, source);
                                    if let Ok(num) = text.parse::<u32>() {
                                        if nat_index == 0 {
                                            min_limit = num;
                                        } else {
                                            max_limit = Some(num);
                                        }
                                        nat_index += 1;
                                    }
                                }
                            }
                        } else if type_child.kind() == "ref_type" {
                            // Extract ref type from nested structure
                            ref_type = extract_ref_type(&type_child, source);
                        }
                    }
                }
            }
        } else if child.kind() == "table_type" {
            // Handle direct table_type (without wrapper)
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "limits" {
                    let mut limits_cursor = type_child.walk();
                    let mut nat_index = 0;
                    for limit_child in type_child.children(&mut limits_cursor) {
                        if limit_child.kind() == "nat" {
                            let text = node_text(&limit_child, source);
                            if let Ok(num) = text.parse::<u32>() {
                                if nat_index == 0 {
                                    min_limit = num;
                                } else {
                                    max_limit = Some(num);
                                }
                                nat_index += 1;
                            }
                        }
                    }
                } else if type_child.kind() == "ref_type" {
                    ref_type = extract_ref_type(&type_child, source);
                }
            }
        }
    }

    Some(Table {
        name,
        index,
        ref_type,
        limits: (min_limit, max_limit),
        line: table_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract memory definitions (with offset for imports)
fn extract_memories_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut memory_index = start_index;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_memory" {
                            if let Some(memory) = extract_memory(&field_child, source, memory_index)
                            {
                                symbol_table.add_memory(memory);
                                memory_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_memory" {
                    if let Some(memory) = extract_memory(&field_child, source, memory_index) {
                        symbol_table.add_memory(memory);
                        memory_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single memory from a memory node
fn extract_memory(memory_node: &Node, source: &str, index: usize) -> Option<Memory> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(memory_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut min_limit = 0;
    let mut max_limit = None;

    let mut cursor = memory_node.walk();
    for child in memory_node.children(&mut cursor) {
        if child.kind() == "memory_fields_type" {
            let mut fields_cursor = child.walk();
            for fields_child in child.children(&mut fields_cursor) {
                if fields_child.kind() == "memory_type" {
                    let mut type_cursor = fields_child.walk();
                    for type_child in fields_child.children(&mut type_cursor) {
                        if type_child.kind() == "limits" {
                            let mut limits_cursor = type_child.walk();
                            let mut nat_index = 0;
                            for limit_child in type_child.children(&mut limits_cursor) {
                                if limit_child.kind() == "nat" {
                                    let text = node_text(&limit_child, source);
                                    if let Ok(num) = text.parse::<u32>() {
                                        if nat_index == 0 {
                                            min_limit = num;
                                        } else {
                                            max_limit = Some(num);
                                        }
                                        nat_index += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "memory_type" {
            // Handle direct memory_type (without wrapper)
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "limits" {
                    let mut limits_cursor = type_child.walk();
                    let mut nat_index = 0;
                    for limit_child in type_child.children(&mut limits_cursor) {
                        if limit_child.kind() == "nat" {
                            let text = node_text(&limit_child, source);
                            if let Ok(num) = text.parse::<u32>() {
                                if nat_index == 0 {
                                    min_limit = num;
                                } else {
                                    max_limit = Some(num);
                                }
                                nat_index += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Some(Memory {
        name,
        index,
        limits: (min_limit, max_limit),
        line: memory_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract tag definitions
fn extract_tags_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    offset: usize,
) {
    let mut cursor = root.walk();
    let mut tag_index = offset;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_tag" {
                            if let Some(tag) = extract_tag(&field_child, source, tag_index) {
                                symbol_table.add_tag(tag);
                                tag_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_tag" {
                    if let Some(tag) = extract_tag(&field_child, source, tag_index) {
                        symbol_table.add_tag(tag);
                        tag_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single tag from a tag node
fn extract_tag(tag_node: &Node, source: &str, index: usize) -> Option<Tag> {
    let (name, name_range) = if let Some(id_node) = find_identifier_node(tag_node) {
        (
            Some(node_text(&id_node, source)),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    };

    let mut params = Vec::new();

    // func_type_params is a direct child of module_field_tag (no tag_type wrapper)
    let mut cursor = tag_node.walk();
    for child in tag_node.children(&mut cursor) {
        if child.kind() == "func_type_params" {
            let mut params_cursor = child.walk();
            for param_child in child.children(&mut params_cursor) {
                if param_child.kind() == "func_type_params_one"
                    || param_child.kind() == "func_type_params_many"
                {
                    let mut param_type_cursor = param_child.walk();
                    for type_child in param_child.children(&mut param_type_cursor) {
                        if type_child.kind() == "value_type" {
                            params.push(extract_value_type(&type_child, source));
                        }
                    }
                }
            }
        }
    }

    Some(Tag {
        name,
        index,
        params,
        line: tag_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Helper: Extract text from a node
fn node_text(node: &Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

/// Extract value type from a value_type node (handles nested structure)
fn extract_value_type(value_type_node: &Node, source: &str) -> ValueType {
    // Check strict match first (for direct children like "i32" in some contexts)
    let text = node_text(value_type_node, source);
    match text.as_str() {
        "i32" => return ValueType::I32,
        "i64" => return ValueType::I64,
        "f32" => return ValueType::F32,
        "f64" => return ValueType::F64,
        "v128" => return ValueType::V128,
        "i8" => return ValueType::I8,
        "i16" => return ValueType::I16,
        "funcref" => return ValueType::Funcref,
        "externref" => return ValueType::Externref,
        "structref" => return ValueType::Structref,
        "arrayref" => return ValueType::Arrayref,
        "i31ref" => return ValueType::I31ref,
        "anyref" => return ValueType::Anyref,
        "eqref" => return ValueType::Eqref,
        "nullref" => return ValueType::Nullref,
        "nullfuncref" => return ValueType::NullFuncref,
        "nullexternref" => return ValueType::NullExternref,
        _ => {}
    }

    let mut cursor = value_type_node.walk();
    for child in value_type_node.children(&mut cursor) {
        let child_kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let child_kind = child_kind.as_str();
        match child_kind {
            "value_type_num_type" => {
                let text = node_text(&child, source);
                // Handle i8/i16 which might be direct children or nested
                if text == "i8" {
                    return ValueType::I8;
                }
                if text == "i16" {
                    return ValueType::I16;
                }

                let mut num_cursor = child.walk();
                for num_child in child.children(&mut num_cursor) {
                    let num_kind = num_child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let num_kind = num_kind.as_str();
                    match num_kind {
                        "num_type_i32" => return ValueType::I32,
                        "num_type_i64" => return ValueType::I64,
                        "num_type_f32" => return ValueType::F32,
                        "num_type_f64" => return ValueType::F64,
                        "num_type_v128" => return ValueType::V128,
                        _ => {
                            if node_text(&num_child, source) == "i8" {
                                return ValueType::I8;
                            }
                            if node_text(&num_child, source) == "i16" {
                                return ValueType::I16;
                            }
                        }
                    }
                }
            }
            "value_type_ref_type" => {
                // Delegate to extract_ref_type
                return extract_ref_type(&child, source);
            }
            // Direct matches for some parser structures
            "ref_type" => return extract_ref_type(&child, source),
            _ => {
                // Check if child itself is a type keyword
                let text = node_text(&child, source);
                if let Some(vt) = simple_type_from_str(&text) {
                    return vt;
                }
            }
        }
    }
    ValueType::Unknown
}

fn simple_type_from_str(s: &str) -> Option<ValueType> {
    match s {
        "i32" => Some(ValueType::I32),
        "i64" => Some(ValueType::I64),
        "f32" => Some(ValueType::F32),
        "f64" => Some(ValueType::F64),
        "v128" => Some(ValueType::V128),
        "i8" => Some(ValueType::I8),
        "i16" => Some(ValueType::I16),
        "funcref" => Some(ValueType::Funcref),
        "externref" => Some(ValueType::Externref),
        "structref" => Some(ValueType::Structref),
        "arrayref" => Some(ValueType::Arrayref),
        "i31ref" => Some(ValueType::I31ref),
        "anyref" => Some(ValueType::Anyref),
        "eqref" => Some(ValueType::Eqref),
        "nullref" => Some(ValueType::Nullref),
        "nullfuncref" => Some(ValueType::NullFuncref),
        "nullexternref" => Some(ValueType::NullExternref),
        _ => None,
    }
}

/// Extract reference type from a ref_type node (handles nested structure)
fn extract_ref_type(ref_type_node: &Node, source: &str) -> ValueType {
    // Check strict match first
    let text = node_text(ref_type_node, source);
    if let Some(vt) = simple_type_from_str(&text) {
        return vt;
    }

    let mut cursor = ref_type_node.walk();
    for child in ref_type_node.children(&mut cursor) {
        let child_kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let child_kind = child_kind.as_str();
        match child_kind {
            "ref_type_funcref" => return ValueType::Funcref,
            "ref_type_externref" => return ValueType::Externref,
            "ref_type_concrete" => {
                // (ref null? $index)
                // Find index child
                let mut concrete_cursor = child.walk();
                let mut index_val = None;
                let mut nullable = false;

                for c in child.children(&mut concrete_cursor) {
                    if c.kind() == "index" {
                        // Parse index (could be numeric or identifier)
                        // For now we just store 0 or some indicator, or we need to resolve it?
                        // ValueType::Ref(u32) stores a numeric index.
                        // But if it's an identifier, we can't resolve it yet without symbol table lookup.
                        // However, Symbols extractor runs in passes. Globals using Ref types might refer to types not yet seen?
                        // No, Types are extracted before globals.
                        // But we don't have access to the SymbolTable here easily.
                        // We will rely on later resolution or just store 0 for now if identifier.
                        // Improving this requires major refactor to pass SymbolTable.
                        // For now, let's just return Ref(0) if we can't parse, or Ref(index).
                        let idx_text = node_text(&c, source);
                        if let Ok(idx) = idx_text.parse::<u32>() {
                            index_val = Some(idx);
                        } else {
                            // It's a named index ($type).
                            // We arguably should store the name in ValueType, but ValueType::Ref takes u32.
                            // Let's stick to parsing what we can.
                        }
                    } else if c.kind() == "null" {
                        // "null" keyword
                        nullable = true;
                    } else if node_text(&c, source) == "null" {
                        nullable = true;
                    }
                }

                if let Some(idx) = index_val {
                    return if nullable {
                        ValueType::RefNull(idx)
                    } else {
                        ValueType::Ref(idx)
                    };
                }
                // Fallback for named types: map to generic StructRef or similar?
                // Or maybe we treat Ref(0) as "some typed ref".
                return ValueType::Structref; // Safe fallback?
            }
            "ref_type_ref" => {
                // (ref null? kind) or (ref null? $type)
                // Similar logic
                return ValueType::Structref; // Fallback
            }
            _ => {
                let text = node_text(&child, source);
                if let Some(vt) = simple_type_from_str(&text) {
                    return vt;
                }
            }
        }
    }

    // Check for direct string matches in children (e.g. "anyref")
    for child in ref_type_node.children(&mut cursor) {
        let text = node_text(&child, source);
        if let Some(vt) = simple_type_from_str(&text) {
            return vt;
        }
    }

    ValueType::Funcref // Default to funcref logic if all else fails
}

/// Extract data segments from the module
fn extract_data_segments(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut data_index = 0;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_data" {
                            if let Some(data) =
                                extract_data_segment(&field_child, source, data_index)
                            {
                                symbol_table.add_data(data);
                                data_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_data" {
                    if let Some(data) = extract_data_segment(&field_child, source, data_index) {
                        symbol_table.add_data(data);
                        data_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single data segment
fn extract_data_segment(data_node: &Node, source: &str, index: usize) -> Option<DataSegment> {
    // Data segment identifiers are wrapped in an "index" node
    let (name, name_range) = find_identifier_in_data_or_elem(data_node, source);

    // Extract string content - look for string literals
    let mut content = String::new();
    let mut byte_length = 0;
    let text = node_text(data_node, source);

    // Find quoted strings in the data segment
    let mut in_string = false;
    let mut escape_next = false;
    for c in text.chars() {
        if escape_next {
            content.push(c);
            byte_length += 1;
            escape_next = false;
        } else if c == '\\' && in_string {
            escape_next = true;
            content.push(c);
        } else if c == '"' {
            in_string = !in_string;
        } else if in_string {
            content.push(c);
            byte_length += 1;
        }
    }

    // Passive segments have names
    let is_passive = name.is_some();

    Some(DataSegment {
        name,
        index,
        content,
        byte_length,
        is_passive,
        line: data_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract elem segments from the module
fn extract_elem_segments(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut elem_index = 0;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_elem" {
                            if let Some(elem) =
                                extract_elem_segment(&field_child, source, elem_index)
                            {
                                symbol_table.add_elem(elem);
                                elem_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_elem" {
                    if let Some(elem) = extract_elem_segment(&field_child, source, elem_index) {
                        symbol_table.add_elem(elem);
                        elem_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single elem segment
fn extract_elem_segment(elem_node: &Node, source: &str, index: usize) -> Option<ElemSegment> {
    // Elem segment identifiers are wrapped in an "index" node
    let (name, name_range) = find_identifier_in_data_or_elem(elem_node, source);

    // Extract function names from elem segment
    let mut func_names = Vec::new();
    let mut table_name = None;

    // Look for index nodes which could be function references
    let text = node_text(elem_node, source);

    // Find table name if present (table $name)
    if let Some(table_pos) = text.find("(table ") {
        let after_table = &text[table_pos + 7..];
        if let Some(dollar_pos) = after_table.find('$') {
            let name_start = dollar_pos;
            let name_end = after_table[name_start + 1..]
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .map(|i| name_start + i + 1)
                .unwrap_or(after_table.len());
            table_name = Some(after_table[name_start..name_end].to_string());
        }
    }

    // Find function references after "func" keyword
    if let Some(func_pos) = text.find(" func ") {
        let after_func = &text[func_pos + 6..];
        // Extract all $identifiers
        let mut remaining = after_func;
        while let Some(dollar_pos) = remaining.find('$') {
            let name_start = dollar_pos;
            let name_end = remaining[name_start + 1..]
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .map(|i| name_start + i + 1)
                .unwrap_or(remaining.len());
            let func_name = &remaining[name_start..name_end];
            func_names.push(func_name.to_string());
            remaining = &remaining[name_end..];
        }
    }

    Some(ElemSegment {
        name,
        index,
        func_names,
        table_name,
        line: elem_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Find identifier in data or elem segment nodes.
/// These have identifiers wrapped in an "index" node, unlike other constructs.
fn find_identifier_in_data_or_elem(node: &Node, source: &str) -> (Option<String>, Option<Range>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Direct identifier (shouldn't happen, but handle it)
        if child.kind() == "identifier" {
            return (Some(node_text(&child, source)), Some(node_to_range(&child)));
        }
        // Index node that contains the identifier
        if child.kind() == "index" {
            let mut index_cursor = child.walk();
            for index_child in child.children(&mut index_cursor) {
                if index_child.kind() == "identifier" {
                    return (
                        Some(node_text(&index_child, source)),
                        Some(node_to_range(&index_child)),
                    );
                }
            }
        }
    }
    (None, None)
}
