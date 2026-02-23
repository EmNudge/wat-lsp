//! Shared SIMD lane index validation for both native and WASM builds.
//!
//! Validates that SIMD lane indices are within the valid range for each instruction.

use crate::core::types::Diagnostic;
use crate::utils::node_to_range;

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check SIMD lane indices in an `instr_plain` node.
///
/// Returns diagnostics for any lane index that exceeds the valid range
/// for the given instruction.
pub(crate) fn check_simd_lane_index(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind_ref = child);

        if kind_ref == "op_simd_lane" {
            // op_simd_lane contains instr_name + int children
            // Extract the instruction name from the first child
            let instr_name = find_instr_name(&child, source);
            if let Some(max_lane) = max_lane_index(&instr_name) {
                // Collect all int children and validate
                let mut int_cursor = child.walk();
                for int_child in child.children(&mut int_cursor) {
                    node_kind!(int_kind_ref = int_child);

                    if int_kind_ref == "int" {
                        if let Some(diag) = validate_lane_value(&int_child, source, max_lane) {
                            diagnostics.push(diag);
                        }
                    }
                }
            }
        } else if kind_ref == "op_simd_lane_memarg" {
            // op_simd_lane_memarg is a token like "v128.load8_lane"
            // The int child is a sibling of op_simd_lane_memarg under instr_plain
            let memarg_text = &source[child.byte_range()];
            if let Some(max_lane) = max_lane_index(memarg_text) {
                // Find the int sibling (last int child of the instr_plain parent)
                let mut parent_cursor = node.walk();
                for sibling in node.children(&mut parent_cursor) {
                    node_kind!(sib_kind_ref = sibling);

                    if sib_kind_ref == "int" {
                        if let Some(diag) = validate_lane_value(&sibling, source, max_lane) {
                            diagnostics.push(diag);
                        }
                    }
                }
            }
        }
    }
}

/// Find the instruction name from an op_simd_lane node.
/// The grammar splits the instruction name into multiple `instr_name` children
/// (e.g., "i8x16", ".", "extract_lane", "_", "s"), so we concatenate them all.
fn find_instr_name(node: &Node, source: &str) -> String {
    let mut name = String::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind_ref = child);

        if kind_ref == "instr_name" {
            name.push_str(&source[child.byte_range()]);
        }
    }
    if name.is_empty() {
        // Fallback: use the text from the node's first token
        source[node.byte_range()]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        name
    }
}

/// Validate a single lane index value against the maximum.
fn validate_lane_value(int_node: &Node, source: &str, max_lane: u32) -> Option<Diagnostic> {
    let text = &source[int_node.byte_range()];
    // Lane indices must be non-negative integers
    // The `int` node may have a sign child; negative values are always invalid
    if text.starts_with('-') {
        return Some(Diagnostic::error(
            node_to_range(int_node),
            "invalid lane index",
        ));
    }
    if let Ok(value) = text.trim_start_matches('+').parse::<u32>() {
        if value > max_lane {
            return Some(Diagnostic::error(
                node_to_range(int_node),
                "invalid lane index",
            ));
        }
    }
    None
}

/// Get the maximum valid lane index for a SIMD instruction.
///
/// Returns `None` if the instruction is not a SIMD lane instruction.
fn max_lane_index(instr_name: &str) -> Option<u32> {
    // i8x16.shuffle: selects from two concatenated v128s (32 lanes total)
    if instr_name == "i8x16.shuffle" {
        return Some(31);
    }

    // Extract/replace lane instructions: max = (128 / lane_bits) - 1
    if instr_name.contains("extract_lane") || instr_name.contains("replace_lane") {
        return lane_count_from_prefix(instr_name).map(|count| count - 1);
    }

    // v128.load/store lane: max = (128 / lane_bits) - 1
    if instr_name.starts_with("v128.load") || instr_name.starts_with("v128.store") {
        if let Some(bits) = extract_lane_bits_from_memarg(instr_name) {
            return Some(128 / bits - 1);
        }
    }

    None
}

/// Get the number of lanes from an instruction prefix like "i8x16", "i32x4", etc.
fn lane_count_from_prefix(instr_name: &str) -> Option<u32> {
    let prefix = instr_name.split('.').next()?;
    match prefix {
        "i8x16" => Some(16),
        "i16x8" => Some(8),
        "i32x4" | "f32x4" => Some(4),
        "i64x2" | "f64x2" => Some(2),
        _ => None,
    }
}

/// Extract lane bit width from a v128.load/store lane instruction name.
/// E.g., "v128.load8_lane" -> 8, "v128.store64_lane" -> 64
fn extract_lane_bits_from_memarg(instr_name: &str) -> Option<u32> {
    // Pattern: v128.{load|store}{8|16|32|64}_lane
    let after_op = instr_name
        .strip_prefix("v128.load")
        .or_else(|| instr_name.strip_prefix("v128.store"))?;

    let bits_str = after_op.split('_').next()?;
    bits_str.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_lane_index() {
        // Extract lane instructions
        assert_eq!(max_lane_index("i8x16.extract_lane_s"), Some(15));
        assert_eq!(max_lane_index("i8x16.extract_lane_u"), Some(15));
        assert_eq!(max_lane_index("i16x8.extract_lane_s"), Some(7));
        assert_eq!(max_lane_index("i16x8.extract_lane_u"), Some(7));
        assert_eq!(max_lane_index("i32x4.extract_lane"), Some(3));
        assert_eq!(max_lane_index("i64x2.extract_lane"), Some(1));
        assert_eq!(max_lane_index("f32x4.extract_lane"), Some(3));
        assert_eq!(max_lane_index("f64x2.extract_lane"), Some(1));

        // Replace lane instructions
        assert_eq!(max_lane_index("i8x16.replace_lane"), Some(15));
        assert_eq!(max_lane_index("i16x8.replace_lane"), Some(7));
        assert_eq!(max_lane_index("i32x4.replace_lane"), Some(3));
        assert_eq!(max_lane_index("i64x2.replace_lane"), Some(1));
        assert_eq!(max_lane_index("f32x4.replace_lane"), Some(3));
        assert_eq!(max_lane_index("f64x2.replace_lane"), Some(1));

        // Shuffle
        assert_eq!(max_lane_index("i8x16.shuffle"), Some(31));

        // Memory lane operations
        assert_eq!(max_lane_index("v128.load8_lane"), Some(15));
        assert_eq!(max_lane_index("v128.load16_lane"), Some(7));
        assert_eq!(max_lane_index("v128.load32_lane"), Some(3));
        assert_eq!(max_lane_index("v128.load64_lane"), Some(1));
        assert_eq!(max_lane_index("v128.store8_lane"), Some(15));
        assert_eq!(max_lane_index("v128.store16_lane"), Some(7));
        assert_eq!(max_lane_index("v128.store32_lane"), Some(3));
        assert_eq!(max_lane_index("v128.store64_lane"), Some(1));

        // Non-lane instructions
        assert_eq!(max_lane_index("i32.add"), None);
        assert_eq!(max_lane_index("v128.load"), None);
        assert_eq!(max_lane_index("i8x16.add"), None);
    }

    #[cfg(feature = "native")]
    mod integration {
        use crate::parser::parse_document;
        use crate::tree_sitter_bindings::create_parser;

        fn get_diagnostics(source: &str) -> Vec<String> {
            let mut parser = create_parser();
            let tree = parser.parse(source, None).unwrap();
            let symbols = parse_document(source).expect("Failed to extract symbols");
            let diags = crate::diagnostics::provide_semantic_diagnostics(&tree, source, &symbols);
            diags
                .iter()
                .filter(|d| d.message == "invalid lane index")
                .map(|d| d.message.clone())
                .collect()
        }

        #[test]
        fn test_valid_lane_indices() {
            // These should produce no diagnostics
            let source = r#"(module
                (func
                    i32.const 0
                    i8x16.extract_lane_s 0
                    drop
                    i32.const 0
                    i8x16.extract_lane_s 15
                    drop
                    i32.const 0
                    i32x4.extract_lane 3
                    drop
                    i32.const 0
                    i64x2.extract_lane 1
                    drop
                )
            )"#;
            assert!(get_diagnostics(source).is_empty());
        }

        #[test]
        fn test_invalid_lane_index_i8x16() {
            let source = r#"(module
                (func
                    i32.const 0
                    i8x16.extract_lane_s 16
                    drop
                )
            )"#;
            assert_eq!(get_diagnostics(source).len(), 1);
        }

        #[test]
        fn test_invalid_lane_index_i32x4() {
            let source = r#"(module
                (func
                    i32.const 0
                    i32x4.extract_lane 4
                    drop
                )
            )"#;
            assert_eq!(get_diagnostics(source).len(), 1);
        }

        #[test]
        fn test_invalid_lane_index_i64x2() {
            let source = r#"(module
                (func
                    i64.const 0
                    i64x2.extract_lane 2
                    drop
                )
            )"#;
            assert_eq!(get_diagnostics(source).len(), 1);
        }

        #[test]
        fn test_invalid_shuffle_index() {
            let source = r#"(module
                (func
                    v128.const i32x4 0 0 0 0
                    v128.const i32x4 0 0 0 0
                    i8x16.shuffle 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 32
                    drop
                )
            )"#;
            assert_eq!(get_diagnostics(source).len(), 1);
        }

        #[test]
        fn test_valid_shuffle_max() {
            let source = r#"(module
                (func
                    v128.const i32x4 0 0 0 0
                    v128.const i32x4 0 0 0 0
                    i8x16.shuffle 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 31
                    drop
                )
            )"#;
            assert!(get_diagnostics(source).is_empty());
        }

        #[test]
        fn test_replace_lane_invalid() {
            let source = r#"(module
                (func
                    v128.const i32x4 0 0 0 0
                    f32.const 0
                    f32x4.replace_lane 4
                    drop
                )
            )"#;
            assert_eq!(get_diagnostics(source).len(), 1);
        }
    }
}
