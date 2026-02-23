//! Shared instruction arity validation for both native and WASM builds.
//!
//! Validates that instructions in linear format have the correct number
//! of immediate parameters (index nodes, constants, etc.).

use crate::core::types::Diagnostic;
use crate::instruction_metadata::lookup_instruction_arity;
use crate::utils::node_to_range;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check if an instruction has the correct number of parameters (linear format)
pub(crate) fn check_instruction_parameter_count(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.is_empty() {
        return;
    }

    let first_child = &children[0];
    node_kind!(instr_kind_ref = first_child);

    match instr_kind_ref {
        "op_index" | "op_index_opt" | "op_gc" | "op_exception" => {
            let instr_name = source[first_child.byte_range()].trim();
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| {
                    node_kind!(k_ref = c);
                    k_ref == "index" || k_ref == "ref_type"
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        "op_const" => {
            let mut op_const_cursor = first_child.walk();
            let op_const_children: Vec<_> = first_child.children(&mut op_const_cursor).collect();

            if op_const_children.is_empty() {
                return;
            }

            let instr_name = source[op_const_children[0].byte_range()].trim();
            let param_count = op_const_children
                .iter()
                .skip(1)
                .filter(|c| {
                    node_kind!(k_ref = c);
                    matches!(k_ref, "int" | "float")
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        "op_nullary" => {
            let instr_name = source[first_child.byte_range()].trim();
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| {
                    node_kind!(k_ref = c);
                    matches!(k_ref, "index" | "expr")
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        k if k.starts_with("op_") => {
            let instr_name = source[first_child.byte_range()].trim();
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| {
                    node_kind!(k_ref = c);
                    k_ref == "index"
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        _ => {}
    }
}

fn validate_instruction_arity(
    instr_name: &str,
    param_count: usize,
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(arity) = lookup_instruction_arity(instr_name) {
        if !arity.is_valid(param_count) {
            let range = node_to_range(node);
            let param_word = if param_count == 1 {
                "parameter"
            } else {
                "parameters"
            };
            diagnostics.push(Diagnostic::error(
                range,
                format!(
                    "Instruction '{}' expects {} parameter(s), but got {} {}",
                    instr_name,
                    arity.expected_message(),
                    param_count,
                    param_word
                ),
            ));
        }
    }
}
