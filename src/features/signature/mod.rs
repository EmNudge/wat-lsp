use crate::symbols::*;
use crate::utils::{format_function_signature, get_line_at_position, node_at_position};
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

#[cfg(test)]
mod tests;

pub fn provide_signature_help(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
) -> Option<SignatureHelp> {
    // Try AST-based approach first
    let call_info = if let Some(node) = node_at_position(tree, document, position.into()) {
        find_function_call_ast(node, document)
    } else {
        None
    };

    // Fall back to string-based approach for incomplete code
    let call_info = call_info.or_else(|| {
        let line = get_line_at_position(document, position.line as usize)?;
        let line_prefix = &line[..position.character.min(line.len() as u32) as usize];
        find_function_call(line_prefix)
    })?;

    match call_info.call_type {
        CallType::Direct => provide_direct_call_signature(symbols, &call_info),
        CallType::CallRef | CallType::ReturnCallRef => {
            provide_call_ref_signature(symbols, &call_info)
        }
    }
}

/// Provide signature help for direct function calls (call $func)
fn provide_direct_call_signature(
    symbols: &SymbolTable,
    call_info: &CallInfo,
) -> Option<SignatureHelp> {
    // Look up the function in the symbol table
    let func = if call_info.name.starts_with('$') {
        symbols.get_function_by_name(&call_info.name)?
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_function_by_index(index)?
    } else {
        return None;
    };

    // Build signature information
    let label = format_function_signature(func);

    let mut parameters = Vec::new();
    for param in &func.parameters {
        let param_label = if let Some(ref name) = param.name {
            format!("({} {})", name, param.param_type)
        } else {
            format!("(param {})", param.param_type)
        };
        parameters.push(ParameterInformation {
            label: ParameterLabel::Simple(param_label),
            documentation: None,
        });
    }

    // Determine which parameter we're currently on based on comma count
    let active_parameter = call_info.arg_text.matches(',').count() as u32;

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(parameters),
            active_parameter: Some(active_parameter.min(func.parameters.len() as u32)),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_parameter.min(func.parameters.len() as u32)),
    })
}

/// Provide signature help for indirect calls via typed function references (call_ref $type)
fn provide_call_ref_signature(
    symbols: &SymbolTable,
    call_info: &CallInfo,
) -> Option<SignatureHelp> {
    // Look up the type in the symbol table
    let type_def = if call_info.name.starts_with('$') {
        symbols.get_type_by_name(&call_info.name)?
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_type_by_index(index)?
    } else {
        return None;
    };

    // The type must be a function type
    let (params, results) = match &type_def.kind {
        TypeKind::Func { params, results } => (params, results),
        _ => return None, // Not a function type
    };

    // Build signature label
    let call_kind = match call_info.call_type {
        CallType::CallRef => "call_ref",
        CallType::ReturnCallRef => "return_call_ref",
        _ => "call_ref",
    };
    let type_index_str = type_def.index.to_string();
    let type_name = type_def.name.as_deref().unwrap_or(&type_index_str);
    let params_str = params
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let results_str = results
        .iter()
        .map(|r| r.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let label = format!(
        "({} {}) (param {}) (result {}) + funcref",
        call_kind,
        type_name,
        if params_str.is_empty() {
            "none"
        } else {
            &params_str
        },
        if results_str.is_empty() {
            "none"
        } else {
            &results_str
        }
    );

    // Build parameter information
    let mut parameters = Vec::new();
    for (i, param_type) in params.iter().enumerate() {
        parameters.push(ParameterInformation {
            label: ParameterLabel::Simple(format!("(param{} {})", i, param_type)),
            documentation: None,
        });
    }
    // The last argument is always the funcref
    parameters.push(ParameterInformation {
        label: ParameterLabel::Simple("(funcref)".to_string()),
        documentation: Some(Documentation::String(
            "Function reference to call".to_string(),
        )),
    });

    // Determine which parameter we're currently on
    let active_parameter = call_info.arg_text.matches(',').count() as u32;
    let param_count = parameters.len() as u32;

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: Some(Documentation::String(format!(
                "Indirect call via typed function reference. The last argument must be a function reference of type {}.",
                type_name
            ))),
            parameters: Some(parameters),
            active_parameter: Some(active_parameter.min(param_count)),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_parameter.min(params.len() as u32 + 1)),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CallType {
    Direct,        // call $func
    CallRef,       // call_ref $type
    ReturnCallRef, // return_call_ref $type
}

struct CallInfo {
    name: String,
    arg_text: String,
    call_type: CallType,
}

/// Find function call using AST analysis
fn find_function_call_ast(node: tree_sitter::Node, document: &str) -> Option<CallInfo> {
    // Walk up the tree to find a call instruction
    let mut current = node;

    loop {
        let kind = current.kind();

        // Check if this is a call instruction
        if kind == "instr_plain" || kind == "expr1_plain" {
            let instr_text = &document[current.byte_range()];

            // Determine the call type based on the instruction text
            let call_type = if instr_text.contains("return_call_ref ") {
                Some(CallType::ReturnCallRef)
            } else if instr_text.contains("call_ref ") {
                Some(CallType::CallRef)
            } else if instr_text.contains("call ") && !instr_text.contains("call_") {
                Some(CallType::Direct)
            } else {
                None
            };

            if let Some(call_type) = call_type {
                // Extract the function/type name from the call instruction
                let mut name = None;
                let mut arg_count = 0;

                // Iterate through children to find the function name and count arguments
                let mut cursor = current.walk();
                for child in current.children(&mut cursor) {
                    let child_kind = child.kind();

                    // The function/type name is in an identifier or index node
                    if child_kind == "index" || child_kind == "identifier" {
                        if name.is_none() {
                            // First identifier/index after the operator is the function/type name
                            name = Some(&document[child.byte_range()]);
                        } else {
                            // Subsequent identifiers/indices are arguments
                            arg_count += 1;
                        }
                    }
                }

                if let Some(func_name) = name {
                    // Create arg_text with appropriate number of commas
                    let arg_text = if arg_count > 0 {
                        vec![","; arg_count - 1].join("")
                    } else {
                        String::new()
                    };

                    return Some(CallInfo {
                        name: func_name.to_string(),
                        arg_text,
                        call_type,
                    });
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

    None
}

fn find_function_call(line_prefix: &str) -> Option<CallInfo> {
    // Look for pattern: call $name( or call_ref $type( or return_call_ref $type(
    // We need to find the most recent unmatched opening paren after the call keyword

    let mut depth = 0;
    let mut paren_pos: Option<usize> = None;

    let chars: Vec<char> = line_prefix.chars().collect();

    // Scan backwards to find the call instruction
    for i in (0..chars.len()).rev() {
        match chars[i] {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(i);
                    break;
                } else {
                    depth -= 1;
                }
            }
            _ => {}
        }
    }

    let paren_pos = paren_pos?;

    // Now look backwards from paren to find call keyword and function/type name
    let before_paren = &line_prefix[..paren_pos];

    // Extract the call instruction and function/type name
    let call_pattern = before_paren.trim_end();

    // Try to find return_call_ref first (most specific)
    if let Some(call_idx) = call_pattern.rfind("return_call_ref ") {
        let after_call = call_pattern[call_idx + 16..].trim_start();
        if let Some(name) = extract_name_from_call(after_call) {
            let arg_text = line_prefix[paren_pos + 1..].to_string();
            return Some(CallInfo {
                name,
                arg_text,
                call_type: CallType::ReturnCallRef,
            });
        }
    }

    // Try call_ref next
    if let Some(call_idx) = call_pattern.rfind("call_ref ") {
        let after_call = call_pattern[call_idx + 9..].trim_start();
        if let Some(name) = extract_name_from_call(after_call) {
            let arg_text = line_prefix[paren_pos + 1..].to_string();
            return Some(CallInfo {
                name,
                arg_text,
                call_type: CallType::CallRef,
            });
        }
    }

    // Finally try regular call (but not call_indirect or call_ref)
    if let Some(call_idx) = call_pattern.rfind("call ") {
        // Make sure this isn't part of call_indirect or call_ref
        let before_call = &call_pattern[..call_idx];
        if !before_call.ends_with("return_") && !before_call.ends_with('_') {
            let after_call = call_pattern[call_idx + 5..].trim_start();
            if let Some(name) = extract_name_from_call(after_call) {
                let arg_text = line_prefix[paren_pos + 1..].to_string();
                return Some(CallInfo {
                    name,
                    arg_text,
                    call_type: CallType::Direct,
                });
            }
        }
    }

    None
}

/// Extract the function/type name from text after a call keyword
fn extract_name_from_call(after_call: &str) -> Option<String> {
    // Extract function name/index (stop at whitespace or paren)
    let name_end = after_call
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(after_call.len());

    let name = after_call[..name_end].to_string();
    if !name.is_empty() {
        Some(name)
    } else {
        None
    }
}
