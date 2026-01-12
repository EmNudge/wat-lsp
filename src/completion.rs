use crate::symbols::*;
use crate::utils::{
    determine_context_from_line, find_containing_function, get_line_at_position, InstructionContext,
};
use once_cell::sync::Lazy;
use regex::Regex;
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

#[cfg(test)]
mod tests;

pub fn provide_completion(
    document: &str,
    symbols: &SymbolTable,
    _tree: &Tree, // Kept for API compatibility; completion uses line-based context detection
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
        if let Some(func) = find_containing_function(symbols, position.into()) {
            for param in &func.parameters {
                if let Some(ref name) = param.name {
                    let insert_text = format!("(local.get {})", name);
                    completions.push(CompletionItem {
                        label: format!("l{}", name),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(format!("(param) {}", param.param_type)),
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
                        detail: Some(format!("(local) {}", local.var_type)),
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
        if let Some(func) = find_containing_function(symbols, position.into()) {
            for param in &func.parameters {
                if let Some(ref name) = param.name {
                    let insert_text = format!("(local.set {} $0)", name);
                    completions.push(CompletionItem {
                        label: format!("l={}", name),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(format!("(param) {}", param.param_type)),
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
                        detail: Some(format!("(local) {}", local.var_type)),
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
                    detail: Some(format!("(global) {}", global.var_type)),
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
                        detail: Some(format!("(global mut) {}", global.var_type)),
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
        completions.push(make_completion("get", "Get local variable value"));
        completions.push(make_completion("set", "Set local variable value"));
        completions.push(make_completion("tee", "Set local and return value"));
        return completions;
    }

    if line_prefix.ends_with("global.") {
        completions.push(make_completion("get", "Get global variable value"));
        completions.push(make_completion("set", "Set global variable value"));
        return completions;
    }

    if line_prefix.ends_with("memory.") {
        completions.push(make_completion("size", "Get memory size in pages"));
        completions.push(make_completion("grow", "Grow memory by delta pages"));
        completions.push(make_completion("fill", "Fill memory region"));
        completions.push(make_completion("copy", "Copy memory region"));
        completions.push(make_completion(
            "init",
            "Initialize memory from data segment",
        ));
        return completions;
    }

    if line_prefix.ends_with("table.") {
        completions.push(make_completion("get", "Get table element"));
        completions.push(make_completion("set", "Set table element"));
        completions.push(make_completion("size", "Get table size"));
        completions.push(make_completion("grow", "Grow table"));
        completions.push(make_completion("fill", "Fill table entries"));
        completions.push(make_completion("copy", "Copy table elements"));
        completions.push(make_completion(
            "init",
            "Initialize table from element segment",
        ));
        return completions;
    }

    // Dollar sign completions (variables and functions)
    if line_prefix.ends_with("$") {
        // For completion, prefer line-based detection since code is incomplete while typing.
        // AST-based detection can walk up to function level and give wrong context.
        // Line-based detection is more reliable for completion scenarios.
        let context = determine_context_from_line(line_prefix);

        match context {
            InstructionContext::Call => {
                // Function completions
                for func in &symbols.functions {
                    if let Some(ref name) = func.name {
                        let params_str = func
                            .parameters
                            .iter()
                            .map(|p| p.param_type.to_string())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let results_str = func
                            .results
                            .iter()
                            .map(|r| r.to_string())
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
            InstructionContext::Global => {
                // Global variable completions
                for global in &symbols.globals {
                    if let Some(ref name) = global.name {
                        completions.push(CompletionItem {
                            label: name[1..].to_string(), // Remove $ prefix
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(format!(
                                "(global{}) {}",
                                if global.is_mutable { " mut" } else { "" },
                                global.var_type
                            )),
                            ..Default::default()
                        });
                    }
                }
            }
            InstructionContext::Local => {
                // Local variable completions
                if let Some(func) = find_containing_function(symbols, position.into()) {
                    for param in &func.parameters {
                        if let Some(ref name) = param.name {
                            completions.push(CompletionItem {
                                label: name[1..].to_string(), // Remove $ prefix
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("(param) {}", param.param_type)),
                                ..Default::default()
                            });
                        }
                    }
                    for local in &func.locals {
                        if let Some(ref name) = local.name {
                            completions.push(CompletionItem {
                                label: name[1..].to_string(), // Remove $ prefix
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!("(local) {}", local.var_type)),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            InstructionContext::Branch => {
                // Block label completions
                if let Some(func) = find_containing_function(symbols, position.into()) {
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
            // For Block, Table, Memory, Type, Tag, Function, and General contexts,
            // show all completions since we don't have special handling for these
            _ => {
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
        completions.push(make_completion("param", "Parameter documentation"));
        completions.push(make_completion("result", "Result documentation"));
        completions.push(make_completion("function", "Function documentation"));
        completions.push(make_completion("todo", "TODO marker"));
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
        completions.push(make_completion(name, desc));
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
            completions.push(make_completion(name, desc));
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
            completions.push(make_completion(name, desc));
        }
    }

    completions
}

fn get_keyword_completions() -> Vec<CompletionItem> {
    vec![
        make_completion("func", "Function declaration"),
        make_completion("param", "Function parameter"),
        make_completion("result", "Function result"),
        make_completion("local", "Local variable"),
        make_completion("global", "Global variable"),
        make_completion("block", "Block statement"),
        make_completion("loop", "Loop statement"),
        make_completion("if", "Conditional statement"),
        make_completion("call", "Call function"),
        make_completion("return", "Return from function"),
        make_completion("drop", "Drop value from stack"),
    ]
}

fn make_completion(label: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(detail.to_string()),
        ..Default::default()
    }
}

static NUMBER_CONST_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([\d._]+)((?:i|f)(?:32|64))$").unwrap());
