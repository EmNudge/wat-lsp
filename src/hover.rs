use crate::symbols::*;
use crate::utils::{
    determine_context_from_line, determine_instruction_context, find_containing_function,
    get_line_at_position, get_word_at_position, is_inside_comment, node_at_position,
    InstructionContext,
};
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

#[cfg(test)]
mod tests;

pub fn provide_hover(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
) -> Option<Hover> {
    // Don't provide hover for content inside comments
    if is_inside_comment(tree, document, position) {
        return None;
    }

    let word = get_word_at_position(document, position)?;

    // Check if it's an instruction
    if let Some(doc) = get_instruction_doc(&word) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: doc,
            }),
            range: None,
        });
    }

    // Check if it's a variable/function reference
    if word.starts_with('$') {
        return provide_symbol_hover(&word, symbols, document, tree, position);
    }

    // Check for numeric indices
    if word.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(index) = word.parse::<usize>() {
            return provide_index_hover(index, symbols, document, tree, position);
        }
    }

    None
}

fn provide_symbol_hover(
    word: &str,
    symbols: &SymbolTable,
    document: &str,
    tree: &Tree,
    position: Position,
) -> Option<Hover> {
    // Determine context using AST, with fallback to line matching
    let context = if let Some(node) = node_at_position(tree, document, position) {
        let ast_context = determine_instruction_context(node, document);
        let line_context = get_line_at_position(document, position.line as usize)
            .map(determine_context_from_line)
            .unwrap_or(InstructionContext::General);

        // Use line-based context for catch clauses (AST may return Block due to grammar issues)
        if ast_context == InstructionContext::General
            || (ast_context == InstructionContext::Block && line_context == InstructionContext::Tag)
        {
            line_context
        } else {
            ast_context
        }
    } else {
        // AST node not found, use line-based detection
        if let Some(line) = get_line_at_position(document, position.line as usize) {
            determine_context_from_line(line)
        } else {
            InstructionContext::General
        }
    };

    // Check for function
    if context == InstructionContext::Call || context == InstructionContext::Function {
        if let Some(func) = symbols.get_function_by_name(word) {
            let signature = format_function_signature(func);
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n{}\n```", signature),
                }),
                range: None,
            });
        }
    }

    // Check for global (both usage and declaration)
    if context == InstructionContext::Global {
        if let Some(global) = symbols.get_global_by_name(word) {
            let mut info = format!(
                "```wat\n(global {} {}{})\n```",
                word,
                if global.is_mutable { "mut " } else { "" },
                global.var_type.to_str()
            );
            if let Some(ref val) = global.initial_value {
                info.push_str(&format!("\n\nInitial value: `{}`", val));
            }
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info,
                }),
                range: None,
            });
        }
    }

    // Check for local/param (both usage and declaration in Function context)
    if context == InstructionContext::Local || context == InstructionContext::Function {
        if let Some(func) = find_containing_function(symbols, position) {
            // Check params
            for param in &func.parameters {
                if param.name.as_deref() == Some(word) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```wat\n(param {} {})\n```",
                                word,
                                param.param_type.to_str()
                            ),
                        }),
                        range: None,
                    });
                }
            }
            // Check locals
            for local in &func.locals {
                if local.name.as_deref() == Some(word) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```wat\n(local {} {})\n```",
                                word,
                                local.var_type.to_str()
                            ),
                        }),
                        range: None,
                    });
                }
            }
        }
    }

    // Check for block labels (both usage and declaration)
    if context == InstructionContext::Branch || context == InstructionContext::Block {
        if let Some(func) = find_containing_function(symbols, position) {
            for block in &func.blocks {
                if block.label == word {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```wat\n({} {})\n```\nDefined at line {}",
                                block.block_type,
                                block.label,
                                block.line + 1
                            ),
                        }),
                        range: None,
                    });
                }
            }
        }
    }

    // Check for table (including call_indirect context)
    if context == InstructionContext::Table || context == InstructionContext::Call {
        if let Some(table) = symbols.get_table_by_name(word) {
            let limits_str = match table.limits.1 {
                Some(max) => format!("{} {}", table.limits.0, max),
                None => table.limits.0.to_string(),
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```wat\n(table {} {} {})\n```",
                        word,
                        limits_str,
                        table.ref_type.to_str()
                    ),
                }),
                range: None,
            });
        }
    }

    // Check for memory (both declaration and references)
    if context == InstructionContext::Memory || context == InstructionContext::General {
        if let Some(memory) = symbols.get_memory_by_name(word) {
            let limits_str = match memory.limits.1 {
                Some(max) => format!("{} {}", memory.limits.0, max),
                None => memory.limits.0.to_string(),
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n(memory {} {})\n```", word, limits_str),
                }),
                range: None,
            });
        }
    }

    // Check for type
    if context == InstructionContext::Type {
        if let Some(type_def) = symbols.get_type_by_name(word) {
            let sig = match &type_def.kind {
                TypeKind::Func { params, results } => {
                    let p_str = params
                        .iter()
                        .map(|t| t.to_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let r_str = results
                        .iter()
                        .map(|t| t.to_str())
                        .collect::<Vec<_>>()
                        .join(" ");

                    let mut s = format!("(type {}", word);
                    if !p_str.is_empty() {
                        s.push_str(&format!(" (param {})", p_str));
                    }
                    if !r_str.is_empty() {
                        s.push_str(&format!(" (result {})", r_str));
                    }
                    s.push(')');
                    s
                }
                TypeKind::Struct { fields } => {
                    format!("(type {} (struct ... {} fields))", word, fields.len())
                }
                TypeKind::Array {
                    element_type,
                    mutable,
                } => {
                    format!(
                        "(type {} (array {} {}))",
                        word,
                        if *mutable { "(mut ...)" } else { "..." },
                        element_type.to_str()
                    )
                }
            };

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n{}\n```", sig),
                }),
                range: None,
            });
        }

        // Check for struct field reference in Type context (struct.get/set $type $field)
        if let Some(hover) = provide_struct_field_hover(word, symbols, document, position) {
            return Some(hover);
        }
    }

    // Check for tag (throw, catch, tag definition)
    if context == InstructionContext::Tag {
        if let Some(tag) = symbols.get_tag_by_name(word) {
            let params_str = if tag.params.is_empty() {
                String::new()
            } else {
                format!(
                    " (param {})",
                    tag.params
                        .iter()
                        .map(|t| t.to_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n(tag {}{})\n```", word, params_str),
                }),
                range: None,
            });
        }
    }

    // Check for data segment
    if context == InstructionContext::Data {
        if let Some(data) = symbols.get_data_by_name(word) {
            let preview = if data.content.len() > 32 {
                format!("{}...", &data.content[..32])
            } else {
                data.content.clone()
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```wat\n(data {} \"{}\")\n```\nLength: {} bytes",
                        word, preview, data.byte_length
                    ),
                }),
                range: None,
            });
        }
    }

    // Check for elem segment
    if context == InstructionContext::Elem {
        if let Some(elem) = symbols.get_elem_by_name(word) {
            let funcs_preview = if elem.func_names.len() > 4 {
                format!(
                    "{} ... ({} total)",
                    elem.func_names[..4].join(" "),
                    elem.func_names.len()
                )
            } else {
                elem.func_names.join(" ")
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n(elem {} func {})\n```", word, funcs_preview),
                }),
                range: None,
            });
        }
    }

    // Fallback: try all symbol types when context is General
    // This handles declaration sites and other edge cases
    if context == InstructionContext::General {
        // Try function
        if let Some(func) = symbols.get_function_by_name(word) {
            let signature = format_function_signature(func);
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n{}\n```", signature),
                }),
                range: None,
            });
        }

        // Try global
        if let Some(global) = symbols.get_global_by_name(word) {
            let mut info = format!(
                "```wat\n(global {} {}{})\n```",
                word,
                if global.is_mutable { "mut " } else { "" },
                global.var_type.to_str()
            );
            if let Some(ref val) = global.initial_value {
                info.push_str(&format!("\n\nInitial value: `{}`", val));
            }
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info,
                }),
                range: None,
            });
        }

        // Try table
        if let Some(table) = symbols.get_table_by_name(word) {
            let limits_str = match table.limits.1 {
                Some(max) => format!("{} {}", table.limits.0, max),
                None => table.limits.0.to_string(),
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```wat\n(table {} {} {})\n```",
                        word,
                        limits_str,
                        table.ref_type.to_str()
                    ),
                }),
                range: None,
            });
        }

        // Try type
        if let Some(type_def) = symbols.get_type_by_name(word) {
            let sig = match &type_def.kind {
                TypeKind::Func { params, results } => {
                    let p_str = params
                        .iter()
                        .map(|t| t.to_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let r_str = results
                        .iter()
                        .map(|t| t.to_str())
                        .collect::<Vec<_>>()
                        .join(" ");

                    let mut s = format!("(type {}", word);
                    if !p_str.is_empty() {
                        s.push_str(&format!(" (param {})", p_str));
                    }
                    if !r_str.is_empty() {
                        s.push_str(&format!(" (result {})", r_str));
                    }
                    s.push(')');
                    s
                }
                TypeKind::Struct { fields } => {
                    format!("(type {} (struct ... {} fields))", word, fields.len())
                }
                TypeKind::Array {
                    element_type,
                    mutable,
                } => {
                    format!(
                        "(type {} (array {} {}))",
                        word,
                        if *mutable { "(mut ...)" } else { "..." },
                        element_type.to_str()
                    )
                }
            };

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n{}\n```", sig),
                }),
                range: None,
            });
        }

        // Try local/param in containing function
        if let Some(func) = find_containing_function(symbols, position) {
            for param in &func.parameters {
                if param.name.as_deref() == Some(word) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```wat\n(param {} {})\n```",
                                word,
                                param.param_type.to_str()
                            ),
                        }),
                        range: None,
                    });
                }
            }
            for local in &func.locals {
                if local.name.as_deref() == Some(word) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```wat\n(local {} {})\n```",
                                word,
                                local.var_type.to_str()
                            ),
                        }),
                        range: None,
                    });
                }
            }
            // Try block labels
            for block in &func.blocks {
                if block.label == word {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```wat\n({} {})\n```\nDefined at line {}",
                                block.block_type,
                                block.label,
                                block.line + 1
                            ),
                        }),
                        range: None,
                    });
                }
            }
        }

        // Try tag
        if let Some(tag) = symbols.get_tag_by_name(word) {
            let params_str = if tag.params.is_empty() {
                String::new()
            } else {
                format!(
                    " (param {})",
                    tag.params
                        .iter()
                        .map(|t| t.to_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n(tag {}{})\n```", word, params_str),
                }),
                range: None,
            });
        }

        // Try data segment
        if let Some(data) = symbols.get_data_by_name(word) {
            let preview = if data.content.len() > 32 {
                format!("{}...", &data.content[..32])
            } else {
                data.content.clone()
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```wat\n(data {} \"{}\")\n```\nLength: {} bytes",
                        word, preview, data.byte_length
                    ),
                }),
                range: None,
            });
        }

        // Try elem segment
        if let Some(elem) = symbols.get_elem_by_name(word) {
            let funcs_preview = if elem.func_names.len() > 4 {
                format!(
                    "{} ... ({} total)",
                    elem.func_names[..4].join(" "),
                    elem.func_names.len()
                )
            } else {
                elem.func_names.join(" ")
            };
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n(elem {} func {})\n```", word, funcs_preview),
                }),
                range: None,
            });
        }

        // Try struct field (e.g., field definition inside a struct type)
        if let Some(hover) = provide_struct_field_hover(word, symbols, document, position) {
            return Some(hover);
        }
    }

    None
}

/// Provide hover for struct field references.
/// This handles cases like (struct.get $point $x) where $x is a field name.
fn provide_struct_field_hover(
    field_name: &str,
    symbols: &SymbolTable,
    document: &str,
    position: Position,
) -> Option<Hover> {
    // Get the line to find the type name preceding the field
    let line = get_line_at_position(document, position.line as usize)?;

    // Look for struct operations that have type and field: struct.get, struct.set
    // Pattern: struct.get $typename $fieldname or struct.set $typename $fieldname
    let struct_ops = ["struct.get", "struct.set", "struct.get_s", "struct.get_u"];

    for op in &struct_ops {
        if let Some(op_pos) = line.find(op) {
            // Find the text after the operation
            let after_op = &line[op_pos + op.len()..];

            // Look for $typename pattern
            if let Some(type_start) = after_op.find('$') {
                let type_rest = &after_op[type_start..];
                // Extract type name
                let type_end = type_rest[1..]
                    .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                    .map(|i| i + 1)
                    .unwrap_or(type_rest.len());
                let type_name = &type_rest[..type_end];

                // Check if we're on the field name (second identifier)
                if let Some(type_def) = symbols.get_type_by_name(type_name) {
                    if let TypeKind::Struct { fields } = &type_def.kind {
                        for (idx, (fname, ftype, mutable)) in fields.iter().enumerate() {
                            if fname.as_deref() == Some(field_name) {
                                return Some(Hover {
                                    contents: HoverContents::Markup(MarkupContent {
                                        kind: MarkupKind::Markdown,
                                        value: format!(
                                            "```wat\n(field {} {} {})\n```\nField {} of {}",
                                            field_name,
                                            if *mutable { "(mut " } else { "" },
                                            ftype.to_str(),
                                            idx,
                                            type_name
                                        ),
                                    }),
                                    range: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Also check if we're on a field definition line inside a type definition
    // Pattern: (field $name type) or (field $name (mut type))
    if line.contains("(field") && line.contains(field_name) {
        // Try to find the containing type by looking upward in the document
        let lines: Vec<&str> = document.lines().collect();
        let current_line = position.line as usize;

        // Look backwards for the type definition
        for i in (0..current_line).rev() {
            let prev_line = lines.get(i)?;
            if prev_line.contains("(type") && prev_line.contains("$") {
                // Found a type definition, extract the name
                if let Some(type_start) = prev_line.find('$') {
                    let type_rest = &prev_line[type_start..];
                    let type_end = type_rest[1..]
                        .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                        .map(|i| i + 1)
                        .unwrap_or(type_rest.len());
                    let type_name = &type_rest[..type_end];

                    if let Some(type_def) = symbols.get_type_by_name(type_name) {
                        if let TypeKind::Struct { fields } = &type_def.kind {
                            for (idx, (fname, ftype, mutable)) in fields.iter().enumerate() {
                                if fname.as_deref() == Some(field_name) {
                                    return Some(Hover {
                                        contents: HoverContents::Markup(MarkupContent {
                                            kind: MarkupKind::Markdown,
                                            value: format!(
                                                "```wat\n(field {} {}{})\n```\nField {} of {}",
                                                field_name,
                                                if *mutable { "(mut " } else { "" },
                                                ftype.to_str(),
                                                idx,
                                                type_name
                                            ),
                                        }),
                                        range: None,
                                    });
                                }
                            }
                        }
                    }
                }
                break;
            }
        }
    }

    None
}

fn provide_index_hover(
    index: usize,
    symbols: &SymbolTable,
    document: &str,
    tree: &Tree,
    position: Position,
) -> Option<Hover> {
    // Determine context using AST, with fallback to line matching
    let context = if let Some(node) = node_at_position(tree, document, position) {
        let ast_context = determine_instruction_context(node, document);
        if ast_context == InstructionContext::General {
            // Fallback to line-based detection for incomplete code
            if let Some(line) = get_line_at_position(document, position.line as usize) {
                determine_context_from_line(line)
            } else {
                InstructionContext::General
            }
        } else {
            ast_context
        }
    } else {
        // AST node not found, use line-based detection
        if let Some(line) = get_line_at_position(document, position.line as usize) {
            determine_context_from_line(line)
        } else {
            InstructionContext::General
        }
    };

    // Check context
    if context == InstructionContext::Call {
        if let Some(func) = symbols.get_function_by_index(index) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```wat\n{}\n```", format_function_signature(func)),
                }),
                range: None,
            });
        }
    }

    if context == InstructionContext::Global {
        if let Some(global) = symbols.get_global_by_index(index) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```wat\n(global {}{} {})\n```",
                        global.name.as_deref().unwrap_or(""),
                        if global.is_mutable { " mut" } else { "" },
                        global.var_type.to_str()
                    ),
                }),
                range: None,
            });
        }
    }

    if context == InstructionContext::Local {
        if let Some(func) = find_containing_function(symbols, position) {
            let total_params = func.parameters.len();
            if index < total_params {
                let param = &func.parameters[index];
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "```wat\n(param {} {})\n```",
                            param.name.as_deref().unwrap_or(&index.to_string()),
                            param.param_type.to_str()
                        ),
                    }),
                    range: None,
                });
            } else {
                let local_index = index - total_params;
                if let Some(local) = func.locals.get(local_index) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "```wat\n(local {} {})\n```",
                                local.name.as_deref().unwrap_or(&local_index.to_string()),
                                local.var_type.to_str()
                            ),
                        }),
                        range: None,
                    });
                }
            }
        }
    }

    None
}

fn format_function_signature(func: &Function) -> String {
    let mut sig = String::from("(func");

    if let Some(ref name) = func.name {
        sig.push_str(&format!(" {}", name));
    }

    for param in &func.parameters {
        sig.push_str(" (param");
        if let Some(ref name) = param.name {
            sig.push_str(&format!(" {}", name));
        }
        sig.push_str(&format!(" {})", param.param_type.to_str()));
    }

    if !func.results.is_empty() {
        sig.push_str(" (result");
        for result in &func.results {
            sig.push_str(&format!(" {}", result.to_str()));
        }
        sig.push(')');
    }

    sig.push(')');
    sig
}

fn get_instruction_doc(word: &str) -> Option<String> {
    INSTRUCTION_DOCS.get(word).map(|s| s.to_string())
}

// Include the auto-generated instruction documentation
include!(concat!(env!("OUT_DIR"), "/instruction_docs.rs"));
