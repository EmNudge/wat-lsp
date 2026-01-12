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

struct CallInfo {
    name: String,
    arg_text: String,
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

            if instr_text.contains("call ") {
                // Extract the function name from the call instruction
                // The structure should be: call <identifier or index>
                let mut name = None;
                let mut arg_count = 0;

                // Iterate through children to find the function name and count arguments
                let mut cursor = current.walk();
                for child in current.children(&mut cursor) {
                    let child_kind = child.kind();

                    // The function name is in an identifier or index node
                    if child_kind == "index" || child_kind == "identifier" {
                        if name.is_none() {
                            // First identifier/index after the operator is the function name
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
    // Look for pattern: call $name( or call index(
    // We need to find the most recent unmatched opening paren after "call"

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

    // Now look backwards from paren to find "call" keyword and function name
    let before_paren = &line_prefix[..paren_pos];

    // Extract the call instruction and function name
    let call_pattern = before_paren.trim_end();

    // Look for "call $name" or "call index"
    if let Some(call_idx) = call_pattern.rfind("call ") {
        let after_call = &call_pattern[call_idx + 5..].trim_start();

        // Extract function name/index (stop at whitespace or paren)
        let name_end = after_call
            .find(|c: char| c.is_whitespace() || c == '(')
            .unwrap_or(after_call.len());

        let name = after_call[..name_end].to_string();

        if !name.is_empty() {
            // Get the argument text (everything between the opening paren and cursor)
            let arg_text = line_prefix[paren_pos + 1..].to_string();

            return Some(CallInfo { name, arg_text });
        }
    }

    None
}
