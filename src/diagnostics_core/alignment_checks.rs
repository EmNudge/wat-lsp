//! Shared alignment validation for load/store instructions.
//!
//! Validates that `align=N` values do not exceed the instruction's natural alignment
//! and that alignment values are powers of 2.

use crate::core::types::Diagnostic;
use crate::utils::node_to_range;

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check alignment constraints on a memory instruction (`instr_plain` node).
///
/// Returns diagnostics if:
/// - `align=N` where N is not a power of 2
/// - `align=N` where N exceeds the instruction's natural alignment
pub fn check_alignment(node: &Node, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut instr_name = None;
    let mut align_node = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        #[cfg(feature = "native")]
        let kind_ref = kind;
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind_ref = &*kind;

        match kind_ref {
            "op_index_opt_offset_opt_align_opt"
            | "op_simd_offset_opt_align_opt"
            | "op_simd_lane_memarg" => {
                instr_name = Some(source[child.byte_range()].to_string());
            }
            "align_value" => {
                align_node = Some(child);
            }
            _ => {}
        }
    }

    // Only check if we have both a memory instruction and an align_value
    let (Some(instr), Some(align_nd)) = (instr_name, align_node) else {
        return diagnostics;
    };

    // Extract the numeric value from align_offset_value child
    let align_val = parse_align_value(&align_nd, source);
    let Some(align) = align_val else {
        return diagnostics;
    };

    // Check power of 2
    if align == 0 || (align & (align - 1)) != 0 {
        diagnostics.push(Diagnostic::error(
            node_to_range(&align_nd),
            "alignment must be a power of 2".to_string(),
        ));
        return diagnostics;
    }

    // Check against natural alignment
    if let Some(natural) = natural_alignment(&instr) {
        if align > natural {
            diagnostics.push(Diagnostic::error(
                node_to_range(&align_nd),
                "alignment must not be larger than natural".to_string(),
            ));
        }
    }

    diagnostics
}

/// Parse the numeric alignment value from an `align_value` node.
/// The node structure is: `align_value` -> `align_offset_value` (child with the number).
fn parse_align_value(node: &Node, source: &str) -> Option<u32> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        #[cfg(feature = "native")]
        let kind_ref = kind;
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind_ref = &*kind;

        if kind_ref == "align_offset_value" {
            let text = &source[child.byte_range()];
            return parse_alignment_number(text);
        }
    }
    None
}

/// Parse an alignment number, handling both decimal and hex formats.
/// Underscores are allowed as digit separators.
fn parse_alignment_number(text: &str) -> Option<u32> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).ok()
    } else {
        cleaned.parse::<u32>().ok()
    }
}

/// Get the natural (maximum valid) alignment in bytes for a memory instruction.
///
/// Returns `None` for non-memory instructions or instructions without alignment constraints.
fn natural_alignment(instr_name: &str) -> Option<u32> {
    // v128.load/store lane variants
    if instr_name.contains("_lane") {
        return natural_alignment_lane(instr_name);
    }

    // v128.load splat/zero variants
    if instr_name.contains("_splat") || instr_name.contains("_zero") {
        return natural_alignment_splat_zero(instr_name);
    }

    // v128.loadNxM_s/u (widening loads — always load 8 bytes)
    if instr_name.contains("x8_") || instr_name.contains("x4_") || instr_name.contains("x2_") {
        // i16x8.load8x8_s, i32x4.load16x4_s, i64x2.load32x2_s etc.
        return Some(8);
    }

    // v128.load / v128.store (full 128-bit)
    if instr_name.starts_with("v128.load") || instr_name.starts_with("v128.store") {
        return Some(16);
    }

    // Standard load/store with explicit sub-width: load8, store16, load32, etc.
    if let Some(bits) = extract_sub_width(instr_name) {
        return Some(bits / 8);
    }

    // Base load/store: derive from type prefix
    natural_alignment_base(instr_name)
}

/// Extract sub-width from instructions like `i32.load8_s`, `i64.store32`.
fn extract_sub_width(instr_name: &str) -> Option<u32> {
    // Find the part after "load" or "store"
    let after = instr_name
        .find("load")
        .map(|i| &instr_name[i + 4..])
        .or_else(|| instr_name.find("store").map(|i| &instr_name[i + 5..]))?;

    // Extract leading digits
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u32>().ok()
}

/// Natural alignment for base load/store (no sub-width suffix).
fn natural_alignment_base(instr_name: &str) -> Option<u32> {
    if instr_name.starts_with("i32.load") || instr_name.starts_with("i32.store") {
        Some(4)
    } else if instr_name.starts_with("i64.load") || instr_name.starts_with("i64.store") {
        Some(8)
    } else if instr_name.starts_with("f32.load") || instr_name.starts_with("f32.store") {
        Some(4)
    } else if instr_name.starts_with("f64.load") || instr_name.starts_with("f64.store") {
        Some(8)
    } else {
        None
    }
}

/// Natural alignment for v128.load/store lane instructions.
fn natural_alignment_lane(instr_name: &str) -> Option<u32> {
    // v128.load8_lane -> 1, v128.load16_lane -> 2, etc.
    let after = instr_name
        .find("load")
        .map(|i| &instr_name[i + 4..])
        .or_else(|| instr_name.find("store").map(|i| &instr_name[i + 5..]))?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    let bits: u32 = digits.parse().ok()?;
    Some(bits / 8)
}

/// Natural alignment for splat and zero load instructions.
fn natural_alignment_splat_zero(instr_name: &str) -> Option<u32> {
    // v128.load8_splat -> 1, v128.load16_splat -> 2, v128.load32_splat -> 4, etc.
    let after = instr_name.find("load").map(|i| &instr_name[i + 4..])?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    let bits: u32 = digits.parse().ok()?;
    Some(bits / 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_alignment_basic_loads() {
        assert_eq!(natural_alignment("i32.load"), Some(4));
        assert_eq!(natural_alignment("i64.load"), Some(8));
        assert_eq!(natural_alignment("f32.load"), Some(4));
        assert_eq!(natural_alignment("f64.load"), Some(8));
    }

    #[test]
    fn test_natural_alignment_basic_stores() {
        assert_eq!(natural_alignment("i32.store"), Some(4));
        assert_eq!(natural_alignment("i64.store"), Some(8));
        assert_eq!(natural_alignment("f32.store"), Some(4));
        assert_eq!(natural_alignment("f64.store"), Some(8));
    }

    #[test]
    fn test_natural_alignment_sub_width_loads() {
        assert_eq!(natural_alignment("i32.load8_s"), Some(1));
        assert_eq!(natural_alignment("i32.load8_u"), Some(1));
        assert_eq!(natural_alignment("i32.load16_s"), Some(2));
        assert_eq!(natural_alignment("i32.load16_u"), Some(2));
        assert_eq!(natural_alignment("i64.load8_s"), Some(1));
        assert_eq!(natural_alignment("i64.load8_u"), Some(1));
        assert_eq!(natural_alignment("i64.load16_s"), Some(2));
        assert_eq!(natural_alignment("i64.load16_u"), Some(2));
        assert_eq!(natural_alignment("i64.load32_s"), Some(4));
        assert_eq!(natural_alignment("i64.load32_u"), Some(4));
    }

    #[test]
    fn test_natural_alignment_sub_width_stores() {
        assert_eq!(natural_alignment("i32.store8"), Some(1));
        assert_eq!(natural_alignment("i32.store16"), Some(2));
        assert_eq!(natural_alignment("i64.store8"), Some(1));
        assert_eq!(natural_alignment("i64.store16"), Some(2));
        assert_eq!(natural_alignment("i64.store32"), Some(4));
    }

    #[test]
    fn test_natural_alignment_v128() {
        assert_eq!(natural_alignment("v128.load"), Some(16));
        assert_eq!(natural_alignment("v128.store"), Some(16));
    }

    #[test]
    fn test_natural_alignment_v128_splat() {
        assert_eq!(natural_alignment("v128.load8_splat"), Some(1));
        assert_eq!(natural_alignment("v128.load16_splat"), Some(2));
        assert_eq!(natural_alignment("v128.load32_splat"), Some(4));
        assert_eq!(natural_alignment("v128.load64_splat"), Some(8));
    }

    #[test]
    fn test_natural_alignment_v128_zero() {
        assert_eq!(natural_alignment("v128.load32_zero"), Some(4));
        assert_eq!(natural_alignment("v128.load64_zero"), Some(8));
    }

    #[test]
    fn test_natural_alignment_v128_lane() {
        assert_eq!(natural_alignment("v128.load8_lane"), Some(1));
        assert_eq!(natural_alignment("v128.load16_lane"), Some(2));
        assert_eq!(natural_alignment("v128.load32_lane"), Some(4));
        assert_eq!(natural_alignment("v128.load64_lane"), Some(8));
        assert_eq!(natural_alignment("v128.store8_lane"), Some(1));
        assert_eq!(natural_alignment("v128.store16_lane"), Some(2));
        assert_eq!(natural_alignment("v128.store32_lane"), Some(4));
        assert_eq!(natural_alignment("v128.store64_lane"), Some(8));
    }

    #[test]
    fn test_natural_alignment_widening_loads() {
        // These all load 8 bytes (64 bits)
        assert_eq!(natural_alignment("i16x8.load8x8_s"), Some(8));
        assert_eq!(natural_alignment("i16x8.load8x8_u"), Some(8));
        assert_eq!(natural_alignment("i32x4.load16x4_s"), Some(8));
        assert_eq!(natural_alignment("i32x4.load16x4_u"), Some(8));
        assert_eq!(natural_alignment("i64x2.load32x2_s"), Some(8));
        assert_eq!(natural_alignment("i64x2.load32x2_u"), Some(8));
    }

    #[test]
    fn test_natural_alignment_non_memory() {
        assert_eq!(natural_alignment("i32.add"), None);
        assert_eq!(natural_alignment("i32.const"), None);
        assert_eq!(natural_alignment("drop"), None);
        assert_eq!(natural_alignment("nop"), None);
    }

    #[test]
    fn test_parse_alignment_number() {
        assert_eq!(parse_alignment_number("1"), Some(1));
        assert_eq!(parse_alignment_number("4"), Some(4));
        assert_eq!(parse_alignment_number("16"), Some(16));
        assert_eq!(parse_alignment_number("0x10"), Some(16));
        assert_eq!(parse_alignment_number("0x1"), Some(1));
        assert_eq!(parse_alignment_number("1_000"), Some(1000));
    }

    #[cfg(feature = "native")]
    mod integration {
        use crate::parser::parse_document;
        use crate::tree_sitter_bindings::create_parser;

        fn get_alignment_diagnostics(source: &str) -> Vec<String> {
            let mut parser = create_parser();
            let tree = parser.parse(source, None).unwrap();
            let symbols = parse_document(source).expect("Failed to extract symbols");
            let diags = crate::diagnostics::provide_semantic_diagnostics(&tree, source, &symbols);
            diags
                .iter()
                .filter(|d| {
                    d.message.contains("alignment must not be larger")
                        || d.message.contains("alignment must be a power of 2")
                })
                .map(|d| d.message.clone())
                .collect()
        }

        #[test]
        fn test_valid_alignments() {
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    i32.load align=4
                    drop
                    i32.const 0
                    i32.load align=1
                    drop
                    i32.const 0
                    i32.load align=2
                    drop
                    i32.const 0
                    f64.store align=8
                    i32.const 0
                    i64.load8_s align=1
                    drop
                )
            )"#;
            assert!(get_alignment_diagnostics(source).is_empty());
        }

        #[test]
        fn test_alignment_too_large_i32_load() {
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    i32.load align=8
                    drop
                )
            )"#;
            let diags = get_alignment_diagnostics(source);
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0], "alignment must not be larger than natural");
        }

        #[test]
        fn test_alignment_too_large_i32_load8() {
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    i32.load8_s align=2
                    drop
                )
            )"#;
            let diags = get_alignment_diagnostics(source);
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0], "alignment must not be larger than natural");
        }

        #[test]
        fn test_alignment_too_large_v128_load() {
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    v128.load align=32
                    drop
                )
            )"#;
            let diags = get_alignment_diagnostics(source);
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0], "alignment must not be larger than natural");
        }

        #[test]
        fn test_alignment_folded_form() {
            let source = r#"(module
                (memory 1)
                (func
                    (drop (i32.load align=8 (i32.const 0)))
                )
            )"#;
            let diags = get_alignment_diagnostics(source);
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0], "alignment must not be larger than natural");
        }

        #[test]
        fn test_alignment_not_power_of_2() {
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    i32.load align=3
                    drop
                )
            )"#;
            let diags = get_alignment_diagnostics(source);
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0], "alignment must be a power of 2");
        }

        #[test]
        fn test_alignment_valid_at_boundary() {
            // align=4 is exactly the natural alignment for i32.load
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    i32.load align=4
                    drop
                )
            )"#;
            assert!(get_alignment_diagnostics(source).is_empty());
        }

        #[test]
        fn test_alignment_v128_lane() {
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    v128.const i32x4 0 0 0 0
                    v128.load8_lane align=2 0
                    drop
                )
            )"#;
            let diags = get_alignment_diagnostics(source);
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0], "alignment must not be larger than natural");
        }

        #[test]
        fn test_multiple_alignment_errors() {
            let source = r#"(module
                (memory 1)
                (func
                    i32.const 0
                    i32.load align=8
                    drop
                    i32.const 0
                    i64.load align=16
                    drop
                )
            )"#;
            let diags = get_alignment_diagnostics(source);
            assert_eq!(diags.len(), 2);
        }
    }
}
