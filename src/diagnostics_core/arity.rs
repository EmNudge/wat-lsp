//! Shared instruction arity validation for both native and WASM builds.
//!
//! Validates that instructions in linear format have the correct number
//! of immediate parameters (index nodes, constants, etc.).

use crate::core::types::Diagnostic;
use crate::instruction_metadata::get_instruction_arity_map;
use crate::utils::node_to_range;
use std::sync::OnceLock;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

static INSTRUCTION_ARITY: OnceLock<
    std::collections::HashMap<&'static str, crate::instruction_metadata::InstructionArity>,
> = OnceLock::new();

fn get_arity_map(
) -> &'static std::collections::HashMap<&'static str, crate::instruction_metadata::InstructionArity>
{
    INSTRUCTION_ARITY.get_or_init(get_instruction_arity_map)
}

/// Check if an instruction has the correct number of parameters (linear format)
pub fn check_instruction_parameter_count(node: &Node, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.is_empty() {
        return diagnostics;
    }

    let first_child = &children[0];
    let instr_kind = first_child.kind();

    #[cfg(feature = "native")]
    let instr_kind_ref = instr_kind;
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let instr_kind_ref = &*instr_kind;

    match instr_kind_ref {
        "op_index" | "op_index_opt" | "op_gc" | "op_exception" => {
            let instr_name = source[first_child.byte_range()].trim();
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| {
                    let k = c.kind();
                    #[cfg(feature = "native")]
                    let k_ref = k;
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let k_ref = &*k;
                    k_ref == "index" || k_ref == "ref_type"
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, &mut diagnostics);
        }
        "op_const" => {
            let mut op_const_cursor = first_child.walk();
            let op_const_children: Vec<_> = first_child.children(&mut op_const_cursor).collect();

            if op_const_children.is_empty() {
                return diagnostics;
            }

            let instr_name = source[op_const_children[0].byte_range()].trim();
            let param_count = op_const_children
                .iter()
                .skip(1)
                .filter(|c| {
                    let k = c.kind();
                    #[cfg(feature = "native")]
                    let k_ref = k;
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let k_ref = &*k;
                    matches!(k_ref, "int" | "float")
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, &mut diagnostics);
        }
        "op_nullary" => {
            let instr_name = source[first_child.byte_range()].trim();
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| {
                    let k = c.kind();
                    #[cfg(feature = "native")]
                    let k_ref = k;
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let k_ref = &*k;
                    matches!(k_ref, "index" | "expr")
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, &mut diagnostics);
        }
        k if k.starts_with("op_") => {
            let instr_name = source[first_child.byte_range()].trim();
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| {
                    let k = c.kind();
                    #[cfg(feature = "native")]
                    let k_ref = k;
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let k_ref = &*k;
                    k_ref == "index"
                })
                .count();
            validate_instruction_arity(instr_name, param_count, node, &mut diagnostics);
        }
        _ => {}
    }

    diagnostics
}

fn validate_instruction_arity(
    instr_name: &str,
    param_count: usize,
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arity_map = get_arity_map();

    if let Some(arity) = arity_map.get(instr_name) {
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
