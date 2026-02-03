use crate::core::types::{CompletionItem, CompletionItemKind, InsertTextFormat, Position};
use crate::symbols::*;
use crate::utils::{
    determine_context_from_line, find_containing_function, get_line_at_position, InstructionContext,
};
use once_cell::sync::Lazy;
use regex::Regex;

#[cfg(all(test, feature = "native"))]
mod tests;

/// Provide completion items at the given position in the document.
/// This is the core completion function using protocol-independent types.
pub fn provide_completion(
    document: &str,
    symbols: &SymbolTable,
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

        completions.push(
            CompletionItem::new(format!("{}{}", number, type_str))
                .with_kind(CompletionItemKind::Snippet)
                .with_detail(format!("Expand to: {}", insert_text))
                .with_insert_text(insert_text),
        );
        return completions;
    }

    // Emmet-like local.get expansion (e.g., l$var -> (local.get $var))
    if line_prefix.ends_with("l$") {
        if let Some(func) = find_containing_function(symbols, position) {
            for param in &func.parameters {
                if let Some(ref name) = param.name {
                    let insert_text = format!("(local.get {})", name);
                    completions.push(
                        CompletionItem::new(format!("l{}", name))
                            .with_kind(CompletionItemKind::Snippet)
                            .with_detail(format!("(param) {}", param.param_type))
                            .with_insert_text(insert_text.clone())
                            .with_documentation(format!("Expands to: {}", insert_text)),
                    );
                }
            }
            for local in &func.locals {
                if let Some(ref name) = local.name {
                    let insert_text = format!("(local.get {})", name);
                    completions.push(
                        CompletionItem::new(format!("l{}", name))
                            .with_kind(CompletionItemKind::Snippet)
                            .with_detail(format!("(local) {}", local.var_type))
                            .with_insert_text(insert_text.clone())
                            .with_documentation(format!("Expands to: {}", insert_text)),
                    );
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
                    completions.push(
                        CompletionItem::new(format!("l={}", name))
                            .with_kind(CompletionItemKind::Snippet)
                            .with_detail(format!("(param) {}", param.param_type))
                            .with_insert_text_format(InsertTextFormat::Snippet)
                            .with_insert_text(insert_text),
                    );
                }
            }
            for local in &func.locals {
                if let Some(ref name) = local.name {
                    let insert_text = format!("(local.set {} $0)", name);
                    completions.push(
                        CompletionItem::new(format!("l={}", name))
                            .with_kind(CompletionItemKind::Snippet)
                            .with_detail(format!("(local) {}", local.var_type))
                            .with_insert_text_format(InsertTextFormat::Snippet)
                            .with_insert_text(insert_text),
                    );
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
                completions.push(
                    CompletionItem::new(format!("g{}", name))
                        .with_kind(CompletionItemKind::Snippet)
                        .with_detail(format!("(global) {}", global.var_type))
                        .with_insert_text(insert_text.clone())
                        .with_documentation(format!("Expands to: {}", insert_text)),
                );
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
                    completions.push(
                        CompletionItem::new(format!("g={}", name))
                            .with_kind(CompletionItemKind::Snippet)
                            .with_detail(format!("(global mut) {}", global.var_type))
                            .with_insert_text_format(InsertTextFormat::Snippet)
                            .with_insert_text(insert_text),
                    );
                }
            }
        }
        return completions;
    }

    // Annotation completion (e.g., (@ -> @name, @producers, @custom)
    if line_prefix.ends_with("(@") {
        completions.extend(get_annotation_completions());
        return completions;
    }

    // Type-prefixed instruction completion (e.g., i32., f64.)
    for type_prefix in ["i32", "i64", "f32", "f64"] {
        if line_prefix.ends_with(&format!("{}.", type_prefix)) {
            completions.extend(get_type_completions(type_prefix));
            return completions;
        }
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

    // Reference instruction completions
    if line_prefix.ends_with("ref.") {
        completions.push(make_completion("null", "Create null reference"));
        completions.push(make_completion("is_null", "Check if reference is null"));
        completions.push(make_completion("func", "Create function reference"));
        completions.push(make_completion("eq", "Compare two references for equality"));
        completions.push(make_completion(
            "as_non_null",
            "Assert reference is not null",
        ));
        completions.push(make_completion(
            "test",
            "Test if reference is of given type",
        ));
        completions.push(make_completion("cast", "Cast reference to given type"));
        completions.push(make_completion("i31", "Create i31 reference from i32"));
        return completions;
    }

    // Struct instruction completions (WasmGC)
    if line_prefix.ends_with("struct.") {
        completions.push(make_completion("new", "Create new struct"));
        completions.push(make_completion(
            "new_default",
            "Create struct with default values",
        ));
        completions.push(make_completion("get", "Get struct field"));
        completions.push(make_completion("get_s", "Get struct field (signed)"));
        completions.push(make_completion("get_u", "Get struct field (unsigned)"));
        completions.push(make_completion("set", "Set struct field"));
        return completions;
    }

    // Array instruction completions (WasmGC)
    if line_prefix.ends_with("array.") {
        completions.push(make_completion("new", "Create new array"));
        completions.push(make_completion("new_fixed", "Create fixed-size array"));
        completions.push(make_completion(
            "new_default",
            "Create array with default values",
        ));
        completions.push(make_completion(
            "new_data",
            "Create array from data segment",
        ));
        completions.push(make_completion(
            "new_elem",
            "Create array from element segment",
        ));
        completions.push(make_completion("get", "Get array element"));
        completions.push(make_completion("get_s", "Get array element (signed)"));
        completions.push(make_completion("get_u", "Get array element (unsigned)"));
        completions.push(make_completion("set", "Set array element"));
        completions.push(make_completion("len", "Get array length"));
        completions.push(make_completion("fill", "Fill array with value"));
        completions.push(make_completion("copy", "Copy array elements"));
        completions.push(make_completion(
            "init_data",
            "Initialize array from data segment",
        ));
        completions.push(make_completion(
            "init_elem",
            "Initialize array from element segment",
        ));
        return completions;
    }

    // i31 instruction completions
    if line_prefix.ends_with("i31.") {
        completions.push(make_completion("get_s", "Get i31 value (signed)"));
        completions.push(make_completion("get_u", "Get i31 value (unsigned)"));
        return completions;
    }

    // Branch-on-null/non-null completions
    if line_prefix.ends_with("br_on_") {
        completions.push(make_completion("null", "Branch if reference is null"));
        completions.push(make_completion(
            "non_null",
            "Branch if reference is not null",
        ));
        completions.push(make_completion("cast", "Branch if cast succeeds"));
        completions.push(make_completion("cast_fail", "Branch if cast fails"));
        return completions;
    }

    // Any/extern conversion completions
    if line_prefix.ends_with("any.") {
        completions.push(make_completion(
            "convert_extern",
            "Convert externref to anyref",
        ));
        return completions;
    }

    if line_prefix.ends_with("extern.") {
        completions.push(make_completion(
            "convert_any",
            "Convert anyref to externref",
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

                        completions.push(
                            CompletionItem::new(&name[1..]) // Remove $ prefix
                                .with_kind(CompletionItemKind::Function)
                                .with_detail(format!(
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
                        );
                    }
                }
            }
            InstructionContext::Global => {
                // Global variable completions
                for global in &symbols.globals {
                    if let Some(ref name) = global.name {
                        completions.push(
                            CompletionItem::new(&name[1..]) // Remove $ prefix
                                .with_kind(CompletionItemKind::Variable)
                                .with_detail(format!(
                                    "(global{}) {}",
                                    if global.is_mutable { " mut" } else { "" },
                                    global.var_type
                                )),
                        );
                    }
                }
            }
            InstructionContext::Local => {
                // Local variable completions
                if let Some(func) = find_containing_function(symbols, position) {
                    for param in &func.parameters {
                        if let Some(ref name) = param.name {
                            completions.push(
                                CompletionItem::new(&name[1..]) // Remove $ prefix
                                    .with_kind(CompletionItemKind::Variable)
                                    .with_detail(format!("(param) {}", param.param_type)),
                            );
                        }
                    }
                    for local in &func.locals {
                        if let Some(ref name) = local.name {
                            completions.push(
                                CompletionItem::new(&name[1..]) // Remove $ prefix
                                    .with_kind(CompletionItemKind::Variable)
                                    .with_detail(format!("(local) {}", local.var_type)),
                            );
                        }
                    }
                }
            }
            InstructionContext::Branch => {
                // Block label completions
                if let Some(func) = find_containing_function(symbols, position) {
                    for block in &func.blocks {
                        completions.push(
                            CompletionItem::new(&block.label[1..]) // Remove $ prefix
                                .with_kind(CompletionItemKind::Constant)
                                .with_detail(format!(
                                    "{} at line {}",
                                    block.block_type,
                                    block.line + 1
                                )),
                        );
                    }
                }
            }
            // For Block, Table, Memory, Type, Tag, Function, and General contexts,
            // show all completions since we don't have special handling for these
            _ => {
                // General $ completions - show everything
                for func in &symbols.functions {
                    if let Some(ref name) = func.name {
                        completions.push(
                            CompletionItem::new(&name[1..])
                                .with_kind(CompletionItemKind::Function)
                                .with_detail("function"),
                        );
                    }
                }
                for global in &symbols.globals {
                    if let Some(ref name) = global.name {
                        completions.push(
                            CompletionItem::new(&name[1..])
                                .with_kind(CompletionItemKind::Variable)
                                .with_detail("global"),
                        );
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
        // Core declarations
        make_completion("func", "Function declaration"),
        make_completion("param", "Function parameter"),
        make_completion("result", "Function result"),
        make_completion("local", "Local variable"),
        make_completion("global", "Global variable"),
        make_completion("type", "Type declaration"),
        make_completion("memory", "Memory declaration"),
        make_completion("table", "Table declaration"),
        make_completion("elem", "Element segment"),
        make_completion("data", "Data segment"),
        make_completion("import", "Import declaration"),
        make_completion("export", "Export declaration"),
        // Control flow
        make_completion("block", "Block statement"),
        make_completion("loop", "Loop statement"),
        make_completion("if", "Conditional statement"),
        make_completion("else", "Else clause"),
        make_completion("br", "Branch to label"),
        make_completion("br_if", "Conditional branch"),
        make_completion("br_table", "Branch table"),
        make_completion("return", "Return from function"),
        // Function calls
        make_completion("call", "Call function"),
        make_completion("call_indirect", "Indirect call via table"),
        make_completion("call_ref", "Call via typed function reference"),
        make_completion("return_call", "Tail call function"),
        make_completion("return_call_indirect", "Tail call via table"),
        make_completion("return_call_ref", "Tail call via typed reference"),
        // Stack operations
        make_completion("drop", "Drop value from stack"),
        make_completion("select", "Select between two values"),
        make_completion("nop", "No operation"),
        make_completion("unreachable", "Trap unconditionally"),
        // WasmGC types
        make_completion("struct", "Struct type definition"),
        make_completion("array", "Array type definition"),
        make_completion("rec", "Recursive type group"),
        make_completion("sub", "Subtype definition"),
        // Exception handling
        make_completion("tag", "Exception tag"),
        make_completion("throw", "Throw exception"),
        make_completion("throw_ref", "Throw exception reference"),
        make_completion("try_table", "Try-catch table block"),
    ]
}

fn make_completion(label: &str, detail: &str) -> CompletionItem {
    CompletionItem::new(label)
        .with_kind(CompletionItemKind::Keyword)
        .with_detail(detail)
}

static NUMBER_CONST_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([\d._]+)((?:i|f)(?:32|64))$").unwrap());

/// Get completions for annotation names
fn get_annotation_completions() -> Vec<CompletionItem> {
    vec![
        CompletionItem::new("name")
            .with_kind(CompletionItemKind::Property)
            .with_detail("Custom name section")
            .with_documentation("Provides human-readable names for debugging"),
        CompletionItem::new("producers")
            .with_kind(CompletionItemKind::Property)
            .with_detail("Producer metadata")
            .with_documentation("Records information about tools that produced this module"),
        CompletionItem::new("custom")
            .with_kind(CompletionItemKind::Property)
            .with_detail("Custom section")
            .with_documentation("Embed arbitrary data in the WebAssembly binary"),
        CompletionItem::new("interface")
            .with_kind(CompletionItemKind::Property)
            .with_detail("Component model interface")
            .with_documentation("Used in WebAssembly Component Model"),
        CompletionItem::new("use")
            .with_kind(CompletionItemKind::Property)
            .with_detail("Component model import")
            .with_documentation("References types from other interfaces"),
        CompletionItem::new("metadata.code.branch_hint")
            .with_kind(CompletionItemKind::Property)
            .with_detail("Branch hint metadata")
            .with_documentation("Provides hints for branch optimization"),
        CompletionItem::new("dylink")
            .with_kind(CompletionItemKind::Property)
            .with_detail("Dynamic linking metadata")
            .with_documentation("Contains information for dynamic linking"),
    ]
}
