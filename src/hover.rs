use crate::symbols::*;
use crate::utils::{
    find_containing_function, get_line_at_position, get_word_at_position, node_at_position,
};
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq)]
enum HoverContext {
    Call,     // Inside call instruction
    Global,   // Inside global.get/set
    Local,    // Inside local.get/set/tee
    Branch,   // Inside br/br_if
    Block,    // Inside block/loop
    Table,    // Inside table operation
    Type,     // Inside type definition/use
    Function, // Inside function definition
    General,  // General context
}

/// Determine hover context from AST node
fn determine_hover_context(node: tree_sitter::Node, document: &str) -> HoverContext {
    let mut current = node;

    loop {
        let kind = current.kind();

        // Check for instruction contexts
        if kind == "instr_plain" || kind == "expr1_plain" {
            // Get the text of the instruction to determine its type
            let instr_text = &document[current.byte_range()];

            if instr_text.contains("call") {
                return HoverContext::Call;
            } else if instr_text.contains("local.") {
                return HoverContext::Local;
            } else if instr_text.contains("global.") {
                return HoverContext::Global;
            } else if instr_text.starts_with("br") || instr_text.contains(" br") {
                return HoverContext::Branch;
            } else if instr_text.contains("table.") {
                return HoverContext::Table;
            }
        }

        // Check for block/loop contexts
        if kind == "instr_block" || kind == "instr_loop" {
            return HoverContext::Block;
        }

        // Check for function definition
        if kind == "module_field_func" {
            return HoverContext::Function;
        }

        // Check for type definition
        if kind == "module_field_type" || kind == "type_use" {
            return HoverContext::Type;
        }

        // Walk up the tree
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    HoverContext::General
}

/// Determine context from line text (fallback for incomplete code)
fn determine_context_from_line(line: &str) -> HoverContext {
    if line.contains("call") {
        HoverContext::Call
    } else if line.contains("global") {
        HoverContext::Global
    } else if line.contains("local") {
        HoverContext::Local
    } else if line.contains("br") {
        HoverContext::Branch
    } else if line.contains("block") || line.contains("loop") {
        HoverContext::Block
    } else if line.contains("table") {
        HoverContext::Table
    } else if line.contains("type") {
        HoverContext::Type
    } else if line.contains("func") {
        HoverContext::Function
    } else {
        HoverContext::General
    }
}

pub fn provide_hover(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
) -> Option<Hover> {
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
        let ast_context = determine_hover_context(node, document);
        if ast_context == HoverContext::General {
            // Fallback to line-based detection for incomplete code
            if let Some(line) = get_line_at_position(document, position.line as usize) {
                determine_context_from_line(line)
            } else {
                HoverContext::General
            }
        } else {
            ast_context
        }
    } else {
        // AST node not found, use line-based detection
        if let Some(line) = get_line_at_position(document, position.line as usize) {
            determine_context_from_line(line)
        } else {
            HoverContext::General
        }
    };

    // Check for function
    if context == HoverContext::Call || context == HoverContext::Function {
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

    // Check for global
    if context == HoverContext::Global {
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

    // Check for local/param
    if context == HoverContext::Local {
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

    // Check for block labels
    if context == HoverContext::Branch || context == HoverContext::Block {
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

    // Check for table
    if context == HoverContext::Table {
        if let Some(table) = symbols.get_table_by_name(word) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```wat\n(table {} {} {})\n```",
                        word,
                        table.limits.0,
                        table.ref_type.to_str()
                    ),
                }),
                range: None,
            });
        }
    }

    // Check for type
    if context == HoverContext::Type {
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
        let ast_context = determine_hover_context(node, document);
        if ast_context == HoverContext::General {
            // Fallback to line-based detection for incomplete code
            if let Some(line) = get_line_at_position(document, position.line as usize) {
                determine_context_from_line(line)
            } else {
                HoverContext::General
            }
        } else {
            ast_context
        }
    } else {
        // AST node not found, use line-based detection
        if let Some(line) = get_line_at_position(document, position.line as usize) {
            determine_context_from_line(line)
        } else {
            HoverContext::General
        }
    };

    // Check context
    if context == HoverContext::Call {
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

    if context == HoverContext::Global {
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

    if context == HoverContext::Local {
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
