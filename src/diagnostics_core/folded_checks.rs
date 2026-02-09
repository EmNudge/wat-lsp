//! Shared folded-expression operand count validation for both native and WASM builds.
//!
//! This module validates that folded (S-expression) instructions like
//! `(i32.add (i32.const 1) (i32.const 2))` have the correct number of operands.

// Allow useless_asref because kind.as_ref() is needed for WASM (String -> &str)
// but is a no-op for native (&str -> &str)
#![allow(clippy::useless_asref)]

use crate::core::types::Diagnostic;
use crate::instruction_metadata::{infer_simd_instruction_arity, OperandMode};
use crate::symbols::{SymbolTable, TypeKind};
use crate::utils::node_to_range;

use super::references::is_nested_in_expr;
use super::semantic::get_arity_map;

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check operand count in folded expressions (expr1_plain nodes).
///
/// Returns diagnostics for instructions with too many or too few operands.
pub fn check_folded_operand_count(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    // First child should be instr_plain
    let first_child = match children.first() {
        Some(c) if c.kind() == "instr_plain" => c,
        _ => return diagnostics,
    };

    // Get the instruction name from instr_plain
    let mut instr_cursor = first_child.walk();
    let instr_children: Vec<_> = first_child.children(&mut instr_cursor).collect();
    if instr_children.is_empty() {
        return diagnostics;
    }

    let instr_kind = instr_children[0].kind();
    let instr_name = match instr_kind.as_ref() {
        "op_const" => {
            let mut const_cursor = instr_children[0].walk();
            let const_children: Vec<_> = instr_children[0].children(&mut const_cursor).collect();
            if const_children.is_empty() {
                return diagnostics;
            }
            source[const_children[0].byte_range()].trim().to_string()
        }
        _ => source[instr_children[0].byte_range()].trim().to_string(),
    };

    // Count expr children (these are the operands)
    let operand_count = children
        .iter()
        .skip(1)
        .filter(|c| c.kind() == "expr")
        .count();

    // Resolve arity: try explicit map first, then SIMD pattern fallback
    let arity_map = get_arity_map();
    let resolved = if let Some(arity) = arity_map.get(instr_name.as_str()) {
        Some((arity.operand_mode, true))
    } else {
        infer_simd_instruction_arity(&instr_name).map(|(c, _)| (OperandMode::Fixed(c), false))
    };

    if let Some((operand_mode, check_dynamic)) = resolved {
        match operand_mode {
            OperandMode::Fixed(expected) => {
                if operand_count > expected {
                    diagnostics.push(Diagnostic::error(
                        node_to_range(node),
                        format!(
                            "Instruction '{}' expects at most {} operands, but got {}",
                            instr_name, expected, operand_count
                        ),
                    ));
                } else if operand_count > 0 && operand_count < expected && is_nested_in_expr(node) {
                    diagnostics.push(Diagnostic::error(
                        node_to_range(node),
                        format!(
                            "Instruction '{}' expects {} operands, but got {}",
                            instr_name, expected, operand_count
                        ),
                    ));
                }
            }
            OperandMode::Dynamic if check_dynamic => {
                validate_dynamic_operands(
                    node,
                    &instr_name,
                    operand_count,
                    symbols,
                    source,
                    &mut diagnostics,
                );
            }
            _ => {}
        }
    }

    diagnostics
}

/// Validate operand count for instructions with dynamic arity (call, throw, struct.new, call_ref, etc.)
fn validate_dynamic_operands(
    node: &Node,
    instr_name: &str,
    operand_count: usize,
    symbols: &SymbolTable,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match instr_name {
        "struct.new" => {
            if let Some(type_name) = extract_instruction_index(node, source) {
                let type_def = if type_name.starts_with('$') {
                    symbols.get_type_by_name(&type_name)
                } else if let Ok(idx) = type_name.parse::<usize>() {
                    symbols.get_type_by_index(idx)
                } else {
                    None
                };
                if let Some(type_def) = type_def {
                    if let TypeKind::Struct { fields } = &type_def.kind {
                        let expected = fields.len();
                        let type_display = type_def.name.as_deref().unwrap_or(&type_name);
                        emit_operand_diagnostic(
                            node,
                            instr_name,
                            operand_count,
                            expected,
                            Some(&format!("fields of struct {}", type_display)),
                            diagnostics,
                        );
                    }
                }
            }
        }
        "call" | "return_call" => {
            if let Some(func_name) = extract_instruction_index(node, source) {
                let func = if func_name.starts_with('$') {
                    symbols.get_function_by_name(&func_name)
                } else if let Ok(idx) = func_name.parse::<usize>() {
                    symbols.get_function_by_index(idx)
                } else {
                    None
                };
                if let Some(func) = func {
                    let expected = func.parameters.len();
                    let func_display = func.name.as_deref().unwrap_or(&func_name);
                    emit_operand_diagnostic(
                        node,
                        instr_name,
                        operand_count,
                        expected,
                        Some(&format!("params of function {}", func_display)),
                        diagnostics,
                    );
                }
            }
        }
        "throw" => {
            if let Some(tag_name) = extract_instruction_index(node, source) {
                let tag = if tag_name.starts_with('$') {
                    symbols.get_tag_by_name(&tag_name)
                } else if let Ok(idx) = tag_name.parse::<usize>() {
                    symbols.get_tag_by_index(idx)
                } else {
                    None
                };
                if let Some(tag) = tag {
                    let expected = tag.params.len();
                    let tag_display = tag.name.as_deref().unwrap_or(&tag_name);
                    emit_operand_diagnostic(
                        node,
                        instr_name,
                        operand_count,
                        expected,
                        Some(&format!("params of tag {}", tag_display)),
                        diagnostics,
                    );
                }
            }
        }
        "call_ref" | "return_call_ref" => {
            if let Some(type_name) = extract_instruction_index(node, source) {
                let type_def = if type_name.starts_with('$') {
                    symbols.get_type_by_name(&type_name)
                } else if let Ok(idx) = type_name.parse::<usize>() {
                    symbols.get_type_by_index(idx)
                } else {
                    None
                };
                if let Some(type_def) = type_def {
                    if let TypeKind::Func { params, .. } = &type_def.kind {
                        // Expected: params + 1 for the funcref
                        let expected = params.len() + 1;
                        let type_display = type_def.name.as_deref().unwrap_or(&type_name);
                        emit_operand_diagnostic(
                            node,
                            instr_name,
                            operand_count,
                            expected,
                            Some(&format!(
                                "{} params + funcref for type {}",
                                params.len(),
                                type_display
                            )),
                            diagnostics,
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

/// Emit a diagnostic for operand count mismatch (too many or too few in nested context).
fn emit_operand_diagnostic(
    node: &Node,
    instr_name: &str,
    actual: usize,
    expected: usize,
    context_msg: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if actual > expected {
        let msg = if let Some(ctx) = context_msg {
            format!(
                "Instruction '{}' expects at most {} operands ({}), but got {}",
                instr_name, expected, ctx, actual
            )
        } else {
            format!(
                "Instruction '{}' expects at most {} operands, but got {}",
                instr_name, expected, actual
            )
        };
        diagnostics.push(Diagnostic::error(node_to_range(node), msg));
    } else if actual > 0 && actual < expected && is_nested_in_expr(node) {
        diagnostics.push(Diagnostic::error(
            node_to_range(node),
            format!(
                "Instruction '{}' expects {} operands, but got {}",
                instr_name, expected, actual
            ),
        ));
    }
}

/// Extract the type/function/tag index from a folded expression node.
///
/// Navigates: expr1_plain → instr_plain → (op_... index/identifier)
fn extract_instruction_index(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "instr_plain" {
            let mut instr_cursor = child.walk();
            for instr_child in child.children(&mut instr_cursor) {
                if instr_child.kind() == "index" || instr_child.kind() == "identifier" {
                    return Some(source[instr_child.byte_range()].trim().to_string());
                }
                // Also check inside op_ nodes
                if instr_child.kind().starts_with("op_") {
                    let mut op_cursor = instr_child.walk();
                    for op_child in instr_child.children(&mut op_cursor) {
                        if op_child.kind() == "index" || op_child.kind() == "identifier" {
                            return Some(source[op_child.byte_range()].trim().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}
