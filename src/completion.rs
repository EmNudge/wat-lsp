use crate::symbols::*;
use crate::utils::{find_containing_function, get_line_at_position, node_at_position};
use once_cell::sync::Lazy;
use regex::Regex;
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

#[cfg(test)]
mod tests;

pub fn provide_completion(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    let line = match get_line_at_position(document, position.line as usize) {
        Some(l) => l,
        None => return completions,
    };

    let line_prefix = &line[..position.character.min(line.len() as u32) as usize];

    // Emmet-like number constant expansion (e.g., 5i32 -> (i32.const 5))
    if let Some(caps) = NUMBER_CONST_REGEX.captures(line_prefix) {
        let number = caps.get(1).unwrap().as_str();
        let type_str = caps.get(2).unwrap().as_str();
        let clean_number = number.replace('_', "");
        let insert_text = format!("({}.const {})", type_str, clean_number);

        completions.push(CompletionItem {
            label: format!("{}{}", number, type_str),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some(format!("Expand to: {}", insert_text)),
            insert_text: Some(insert_text),
            ..Default::default()
        });
        return completions;
    }

    // Emmet-like local.get expansion (e.g., l$var -> (local.get $var))
    if line_prefix.ends_with("l$") {
        if let Some(func) = find_containing_function(symbols, position) {
            for param in &func.parameters {
                if let Some(ref name) = param.name {
                    let insert_text = format!("(local.get {})", name);
                    completions.push(CompletionItem {
                        label: format!("l{}", name),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(format!("(param) {}", param.param_type.to_str())),
                        insert_text: Some(insert_text.clone()),
                        documentation: Some(Documentation::String(format!(
                            "Expands to: {}",
                            insert_text
                        ))),
                        ..Default::default()
                    });
                }
            }
            for local in &func.locals {
                if let Some(ref name) = local.name {
                    let insert_text = format!("(local.get {})", name);
                    completions.push(CompletionItem {
                        label: format!("l{}", name),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(format!("(local) {}", local.var_type.to_str())),
                        insert_text: Some(insert_text.clone()),
                        documentation: Some(Documentation::String(format!(
                            "Expands to: {}",
                            insert_text
                        ))),
                        ..Default::default()
                    });
                }
            }
        }
        return completions;
    }

    // Emmet-like local.set expansion (e.g., l=$var -> (local.set $var ))
    if line_prefix.ends_with("l=$") {
        if let Some(func) = find_containing_function(symbols, position) {
            for param in &func.parameters {
                if let Some(ref name) = param.name {
                    let insert_text = format!("(local.set {} $0)", name);
                    completions.push(CompletionItem {
                        label: format!("l={}", name),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(format!("(param) {}", param.param_type.to_str())),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        insert_text: Some(insert_text.clone()),
                        ..Default::default()
                    });
                }
            }
            for local in &func.locals {
                if let Some(ref name) = local.name {
                    let insert_text = format!("(local.set {} $0)", name);
                    completions.push(CompletionItem {
                        label: format!("l={}", name),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(format!("(local) {}", local.var_type.to_str())),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        insert_text: Some(insert_text.clone()),
                        ..Default::default()
                    });
                }
            }
        }
        return completions;
    }

    // Emmet-like global.get expansion (e.g., g$var -> (global.get $var))
    if line_prefix.ends_with("g$") {
        for global in &symbols.globals {
            if let Some(ref name) = global.name {
                let insert_text = format!("(global.get {})", name);
                completions.push(CompletionItem {
                    label: format!("g{}", name),
                    kind: Some(CompletionItemKind::SNIPPET),
                    detail: Some(format!("(global) {}", global.var_type.to_str())),
                    insert_text: Some(insert_text.clone()),
                    documentation: Some(Documentation::String(format!(
                        "Expands to: {}",
                        insert_text
                    ))),
                    ..Default::default()
                });
            }
        }
        return completions;
    }

    // Emmet-like global.set expansion (e.g., g=$var -> (global.set $var ))
    if line_prefix.ends_with("g=$") {
        for global in &symbols.globals {
            if let Some(ref name) = global.name {
                if global.is_mutable {
                    let insert_text = format!("(global.set {} $0)", name);
                    completions.push(CompletionItem {
                        label: format!("g={}", name),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(format!("(global mut) {}", global.var_type.to_str())),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        insert_text: Some(insert_text.clone()),
                        ..Default::default()
                    });
                }
            }
        }
        return completions;
    }

    // Type-prefixed instruction completion (e.g., i32., f64.)
    if line_prefix.ends_with("i32.") {
        completions.extend(get_type_completions("i32"));
        return completions;
    }
    if line_prefix.ends_with("i64.") {
        completions.extend(get_type_completions("i64"));
        return completions;
    }
    if line_prefix.ends_with("f32.") {
        completions.extend(get_type_completions("f32"));
        return completions;
    }
    if line_prefix.ends_with("f64.") {
        completions.extend(get_type_completions("f64"));
        return completions;
    }

    // Instruction prefix completions
    if line_prefix.ends_with("local.") {
        completions.push(make_completion(
            "get",
            "local.get",
            "Get local variable value",
        ));
        completions.push(make_completion(
            "set",
            "local.set",
            "Set local variable value",
        ));
        completions.push(make_completion(
            "tee",
            "local.tee",
            "Set local and return value",
        ));
        return completions;
    }

    if line_prefix.ends_with("global.") {
        completions.push(make_completion(
            "get",
            "global.get",
            "Get global variable value",
        ));
        completions.push(make_completion(
            "set",
            "global.set",
            "Set global variable value",
        ));
        return completions;
    }

    if line_prefix.ends_with("memory.") {
        completions.push(make_completion(
            "size",
            "memory.size",
            "Get memory size in pages",
        ));
        completions.push(make_completion(
            "grow",
            "memory.grow",
            "Grow memory by delta pages",
        ));
        completions.push(make_completion("fill", "memory.fill", "Fill memory region"));
        completions.push(make_completion("copy", "memory.copy", "Copy memory region"));
        completions.push(make_completion(
            "init",
            "memory.init",
            "Initialize memory from data segment",
        ));
        return completions;
    }

    if line_prefix.ends_with("table.") {
        completions.push(make_completion("get", "table.get", "Get table element"));
        completions.push(make_completion("set", "table.set", "Set table element"));
        completions.push(make_completion("size", "table.size", "Get table size"));
        completions.push(make_completion("grow", "table.grow", "Grow table"));
        completions.push(make_completion("fill", "table.fill", "Fill table entries"));
        completions.push(make_completion("copy", "table.copy", "Copy table elements"));
        completions.push(make_completion(
            "init",
            "table.init",
            "Initialize table from element segment",
        ));
        return completions;
    }

    // Dollar sign completions (variables and functions)
    if line_prefix.ends_with("$") {
        // Try AST-based context detection, but fall back to string matching for incomplete code
        let context = if let Some(node) = node_at_position(tree, document, position) {
            let ast_context = determine_dollar_context(node);
            // If AST gives us General context, fall back to string matching
            if ast_context == DollarContext::General {
                determine_context_from_line(line_prefix)
            } else {
                ast_context
            }
        } else {
            // No valid AST node, use string matching
            determine_context_from_line(line_prefix)
        };

        match context {
            DollarContext::Call => {
                // Function completions
                for func in &symbols.functions {
                    if let Some(ref name) = func.name {
                        let params_str = func
                            .parameters
                            .iter()
                            .map(|p| p.param_type.to_str().to_string())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let results_str = func
                            .results
                            .iter()
                            .map(|r| r.to_str())
                            .collect::<Vec<_>>()
                            .join(" ");

                        completions.push(CompletionItem {
                            label: name[1..].to_string(), // Remove $ prefix
                            kind: Some(CompletionItemKind::FUNCTION),
                            detail: Some(format!(
                                "({}) -> ({})",
                                if params_str.is_empty() {
                                    "no params"
                                } else {
                                    &params_str
                                },
                                if results_str.is_empty() {
                                    "no result"
                                } else {
                                    &results_str
                                }
                            )),
                            ..Default::default()
                        });
                    }
                }
            }
            DollarContext::Global => {
                // Global variable completions
                for global in &symbols.globals {
                    if let Some(ref name) = global.name {
                        completions.push(CompletionItem {
                            label: name[1..].to_string(), // Remove $ prefix
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(format!(
                                "(global{}) {}",
                                if global.is_mutable { " mut" } else { "" },
                                global.var_type.to_str()
                            )),
                            ..Default::default()
                        });
                    }
                }
            }
            DollarContext::Local => {
                // Local variable completions
                if let Some(func) = find_containing_function(symbols, position) {
                    for param in &func.parameters {
                        if let Some(ref name) = param.name {
                            completions.push(CompletionItem {
                                label: name[1..].to_string(), // Remove $ prefix
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("(param) {}", param.param_type.to_str())),
                                ..Default::default()
                            });
                        }
                    }
                    for local in &func.locals {
                        if let Some(ref name) = local.name {
                            completions.push(CompletionItem {
                                label: name[1..].to_string(), // Remove $ prefix
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("(local) {}", local.var_type.to_str())),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            DollarContext::Branch => {
                // Block label completions
                if let Some(func) = find_containing_function(symbols, position) {
                    for block in &func.blocks {
                        completions.push(CompletionItem {
                            label: block.label[1..].to_string(), // Remove $ prefix
                            kind: Some(CompletionItemKind::CONSTANT),
                            detail: Some(format!(
                                "{} at line {}",
                                block.block_type,
                                block.line + 1
                            )),
                            ..Default::default()
                        });
                    }
                }
            }
            DollarContext::General => {
                // General $ completions - show everything
                for func in &symbols.functions {
                    if let Some(ref name) = func.name {
                        completions.push(CompletionItem {
                            label: name[1..].to_string(),
                            kind: Some(CompletionItemKind::FUNCTION),
                            detail: Some("function".to_string()),
                            ..Default::default()
                        });
                    }
                }
                for global in &symbols.globals {
                    if let Some(ref name) = global.name {
                        completions.push(CompletionItem {
                            label: name[1..].to_string(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some("global".to_string()),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        return completions;
    }

    // JSDoc tag completions
    if line_prefix.ends_with("@") {
        completions.push(make_completion(
            "param",
            "@param",
            "Parameter documentation",
        ));
        completions.push(make_completion("result", "@result", "Result documentation"));
        completions.push(make_completion(
            "function",
            "@function",
            "Function documentation",
        ));
        completions.push(make_completion("todo", "@todo", "TODO marker"));
        return completions;
    }

    // General keyword completions
    if !line_prefix.trim().is_empty() {
        completions.extend(get_keyword_completions());
    }

    completions
}

fn get_type_completions(type_prefix: &str) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    let common = vec![
        ("const", "Create constant value"),
        ("add", "Add two values"),
        ("sub", "Subtract two values"),
        ("mul", "Multiply two values"),
        ("eq", "Check equality"),
        ("ne", "Check inequality"),
        ("load", "Load from memory"),
        ("store", "Store to memory"),
    ];

    for (name, desc) in common {
        completions.push(make_completion(
            name,
            &format!("{}.{}", type_prefix, name),
            desc,
        ));
    }

    if type_prefix.starts_with('i') {
        // Integer-specific instructions
        let int_ops = vec![
            ("div_s", "Signed division"),
            ("div_u", "Unsigned division"),
            ("rem_s", "Signed remainder"),
            ("rem_u", "Unsigned remainder"),
            ("and", "Bitwise AND"),
            ("or", "Bitwise OR"),
            ("xor", "Bitwise XOR"),
            ("shl", "Shift left"),
            ("shr_s", "Signed shift right"),
            ("shr_u", "Unsigned shift right"),
            ("rotl", "Rotate left"),
            ("rotr", "Rotate right"),
            ("clz", "Count leading zeros"),
            ("ctz", "Count trailing zeros"),
            ("popcnt", "Population count"),
            ("eqz", "Check if zero"),
            ("lt_s", "Signed less than"),
            ("lt_u", "Unsigned less than"),
            ("gt_s", "Signed greater than"),
            ("gt_u", "Unsigned greater than"),
            ("le_s", "Signed less or equal"),
            ("le_u", "Unsigned less or equal"),
            ("ge_s", "Signed greater or equal"),
            ("ge_u", "Unsigned greater or equal"),
        ];
        for (name, desc) in int_ops {
            completions.push(make_completion(
                name,
                &format!("{}.{}", type_prefix, name),
                desc,
            ));
        }
    } else {
        // Float-specific instructions
        let float_ops = vec![
            ("div", "Divide two values"),
            ("sqrt", "Square root"),
            ("min", "Minimum of two values"),
            ("max", "Maximum of two values"),
            ("abs", "Absolute value"),
            ("neg", "Negate value"),
            ("ceil", "Round up"),
            ("floor", "Round down"),
            ("trunc", "Round toward zero"),
            ("nearest", "Round to nearest"),
            ("copysign", "Copy sign"),
            ("lt", "Less than"),
            ("gt", "Greater than"),
            ("le", "Less or equal"),
            ("ge", "Greater or equal"),
        ];
        for (name, desc) in float_ops {
            completions.push(make_completion(
                name,
                &format!("{}.{}", type_prefix, name),
                desc,
            ));
        }
    }

    completions
}

fn get_keyword_completions() -> Vec<CompletionItem> {
    vec![
        make_completion("func", "func", "Function declaration"),
        make_completion("param", "param", "Function parameter"),
        make_completion("result", "result", "Function result"),
        make_completion("local", "local", "Local variable"),
        make_completion("global", "global", "Global variable"),
        make_completion("block", "block", "Block statement"),
        make_completion("loop", "loop", "Loop statement"),
        make_completion("if", "if", "Conditional statement"),
        make_completion("call", "call", "Call function"),
        make_completion("return", "return", "Return from function"),
        make_completion("drop", "drop", "Drop value from stack"),
    ]
}

fn make_completion(label: &str, insert_text: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(detail.to_string()),
        insert_text: Some(insert_text.to_string()),
        ..Default::default()
    }
}

static NUMBER_CONST_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([\d._]+)((?:i|f)(?:32|64))$").unwrap());

/// Context for dollar sign completions
#[derive(Debug, PartialEq)]
enum DollarContext {
    Call,    // Inside call instruction
    Global,  // Inside global.get/set
    Local,   // Inside local.get/set/tee
    Branch,  // Inside br/br_if
    General, // General context, show all
}

/// Determine the context for dollar sign completion using AST
fn determine_dollar_context(node: tree_sitter::Node) -> DollarContext {
    // Look for instruction ancestors to determine context
    let mut current = node;
    loop {
        let kind = current.kind();

        // Check if we're in a plain instruction
        if kind == "instr_plain" {
            // Check the operator
            if let Some(op_child) = current.child_by_field_name("op") {
                let op_kind = op_child.kind();
                if op_kind.starts_with("op_call") {
                    return DollarContext::Call;
                } else if op_kind.contains("local") {
                    return DollarContext::Local;
                } else if op_kind.contains("global") {
                    return DollarContext::Global;
                } else if op_kind.starts_with("op_br") {
                    return DollarContext::Branch;
                }
            }

            // Also check children for operators
            let mut cursor = current.walk();
            for child in current.children(&mut cursor) {
                let child_kind = child.kind();
                if child_kind.starts_with("op_call") {
                    return DollarContext::Call;
                } else if child_kind.contains("local") {
                    return DollarContext::Local;
                } else if child_kind.contains("global") {
                    return DollarContext::Global;
                } else if child_kind.starts_with("op_br") {
                    return DollarContext::Branch;
                }
            }
        }

        // Move to parent
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    DollarContext::General
}

/// Fallback: Determine context from line text (for incomplete/malformed code)
fn determine_context_from_line(line_prefix: &str) -> DollarContext {
    if line_prefix.contains("call ") {
        DollarContext::Call
    } else if line_prefix.contains("global.") {
        DollarContext::Global
    } else if line_prefix.contains("local.") {
        DollarContext::Local
    } else if line_prefix.contains("br") {
        DollarContext::Branch
    } else {
        DollarContext::General
    }
}
