use crate::symbols::*;
use crate::tree_sitter_bindings::create_parser;
use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Tree};

#[cfg(test)]
mod tests;

/// Parse a WAT document and extract symbols (PUBLIC API - unchanged)
pub fn parse_document(text: &str) -> Result<SymbolTable, String> {
    let mut parser = create_parser();
    let tree = parser
        .parse(text, None)
        .ok_or_else(|| "Failed to parse document".to_string())?;

    extract_symbols(&tree, text)
}

/// Extract all symbols from the parse tree
fn extract_symbols(tree: &Tree, source: &str) -> Result<SymbolTable, String> {
    let mut symbol_table = SymbolTable::new();
    let root = tree.root_node();

    // Extract in order: globals, types, tables, functions
    // (Order matters for index assignment)
    extract_globals(&root, source, &mut symbol_table);
    extract_types(&root, source, &mut symbol_table);
    extract_tables(&root, source, &mut symbol_table);
    extract_functions(&root, source, &mut symbol_table);

    Ok(symbol_table)
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

/// Extract function symbols using tree-sitter AST traversal
fn extract_functions(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut func_index = 0;

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
                                symbol_table.add_function(func);
                                func_index += 1;
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
                        symbol_table.add_function(func);
                        func_index += 1;
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
        || kind == "expr1_block"
        || kind == "expr1_loop"
        || kind == "expr1_if"
    {
        // Check if it has a label
        if let Some(id_node) = find_identifier_node(node) {
            let label = node_text(&id_node, source);
            let block_type = match kind {
                "block_block" | "expr1_block" => "block",
                "block_loop" | "expr1_loop" => "loop",
                "block_if" | "expr1_if" => "if",
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

/// Extract global variables
fn extract_globals(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut global_index = 0;

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
                }
            }
        }
    }
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

    let mut cursor = type_node.walk();
    for child in type_node.children(&mut cursor) {
        if child.kind() == "type_field" {
            // type_field wraps func_type nodes
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "func_type" {
                    // Extract parameters and results from func_type
                    // Pass the func_type node to extract_parameters (it will find func_type_params children)
                    let params = extract_parameters(&field_child, source);
                    for param in params {
                        parameters.push(param.param_type);
                    }

                    // Extract results
                    results.extend(extract_results(&field_child, source));
                }
            }
        }
    }

    Some(TypeDef {
        name,
        index,
        parameters,
        results,
        line: type_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract table definitions
fn extract_tables(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut table_index = 0;

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

/// Helper: Extract text from a node
fn node_text(node: &Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

/// Extract value type from a value_type node (handles nested structure)
fn extract_value_type(value_type_node: &Node, _source: &str) -> ValueType {
    let mut cursor = value_type_node.walk();
    for child in value_type_node.children(&mut cursor) {
        match child.kind() {
            "value_type_num_type" => {
                // Numeric type: i32, i64, f32, f64
                let mut num_cursor = child.walk();
                for num_child in child.children(&mut num_cursor) {
                    match num_child.kind() {
                        "num_type_i32" => return ValueType::I32,
                        "num_type_i64" => return ValueType::I64,
                        "num_type_f32" => return ValueType::F32,
                        "num_type_f64" => return ValueType::F64,
                        _ => {}
                    }
                }
            }
            "value_type_ref_type" => {
                // Reference type: funcref, externref
                let mut ref_cursor = child.walk();
                for ref_child in child.children(&mut ref_cursor) {
                    match ref_child.kind() {
                        "ref_type_funcref" => return ValueType::Funcref,
                        "ref_type_externref" => return ValueType::Externref,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    ValueType::Unknown
}

/// Extract reference type from a ref_type node (handles nested structure)
fn extract_ref_type(ref_type_node: &Node, _source: &str) -> ValueType {
    let mut cursor = ref_type_node.walk();
    for child in ref_type_node.children(&mut cursor) {
        match child.kind() {
            "ref_type_funcref" => return ValueType::Funcref,
            "ref_type_externref" => return ValueType::Externref,
            _ => {}
        }
    }
    ValueType::Funcref // Default to funcref
}
