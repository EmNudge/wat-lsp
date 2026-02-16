//! Shared call-info extraction for signature help.
//!
//! Contains `CallType`, `CallInfo`, `find_function_call_ast`, `find_function_call`,
//! and `extract_name_from_call`. Used by both the native LSP signature provider
//! and the WASM API signature provider.

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CallType {
    Direct,        // call $func
    CallRef,       // call_ref $type
    ReturnCallRef, // return_call_ref $type
}

pub struct CallInfo {
    pub name: String,
    pub arg_text: String,
    pub call_type: CallType,
}

/// Find function call using AST analysis.
///
/// Walks up the tree from `node` to find a call instruction and extracts the
/// function/type name and argument count.
pub fn find_function_call_ast(node: &Node, document: &str) -> Option<CallInfo> {
    let mut current = {
        #[cfg(feature = "native")]
        {
            *node
        }
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        {
            node.clone()
        }
    };

    loop {
        let kind = current.kind();

        if kind == "instr_plain" || kind == "expr1_plain" {
            let instr_text = &document[current.byte_range()];

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
                let mut name = None;
                let mut arg_count = 0;

                let mut cursor = current.walk();
                for child in current.children(&mut cursor) {
                    let child_kind = child.kind();

                    if child_kind == "index" || child_kind == "identifier" {
                        if name.is_none() {
                            name = Some(&document[child.byte_range()]);
                        } else {
                            arg_count += 1;
                        }
                    }
                    // call_indirect uses type_use (type $t), extract index from it
                    if child_kind == "type_use" && name.is_none() {
                        let mut inner_cursor = child.walk();
                        for inner_child in child.children(&mut inner_cursor) {
                            if inner_child.kind() == "index" || inner_child.kind() == "identifier" {
                                name = Some(&document[inner_child.byte_range()]);
                                break;
                            }
                        }
                    }
                }

                if let Some(func_name) = name {
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

        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    None
}

/// Find function call using string-based analysis (fallback for incomplete code).
pub fn find_function_call(line_prefix: &str) -> Option<CallInfo> {
    let mut depth = 0;
    let mut paren_pos: Option<usize> = None;

    let chars: Vec<char> = line_prefix.chars().collect();

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

    let before_paren = &line_prefix[..paren_pos];
    let call_pattern = before_paren.trim_end();

    // Try return_call_ref first (most specific)
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

    // Try call_ref
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

    // Try regular call
    if let Some(call_idx) = call_pattern.rfind("call ") {
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

/// Extract the function/type name from text after a call keyword.
pub fn extract_name_from_call(after_call: &str) -> Option<String> {
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
