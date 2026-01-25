//! Integration tests for the WAT grammar
//!
//! These tests verify that the tree-sitter grammar correctly parses all standard
//! WebAssembly instructions without syntax errors. This catches grammar regressions
//! when new WASM instructions are added to the specification.

#[cfg(feature = "native")]
mod grammar_tests {
    use std::fs;
    use std::path::Path;
    use tower_lsp::lsp_types::DiagnosticSeverity;
    use wat_lsp_rust::diagnostics::provide_tree_sitter_diagnostics;
    use wat_lsp_rust::tree_sitter_bindings::create_parser;

    /// Check a WAT file for syntax errors only (tree-sitter level)
    fn check_syntax(source: &str) -> Vec<String> {
        let mut parser = create_parser();
        let tree = parser.parse(source, None).expect("Failed to parse");
        let diagnostics = provide_tree_sitter_diagnostics(&tree, source);

        diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .map(|d| format!("Line {}: {}", d.range.start.line + 1, d.message))
            .collect()
    }

    /// Load and check a fixture file
    fn check_fixture(filename: &str) -> Vec<String> {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(filename);

        let source = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", fixture_path.display(), e));

        check_syntax(&source)
    }

    #[test]
    fn test_simd_comprehensive_parses_without_errors() {
        let errors = check_fixture("simd_comprehensive.wat");
        assert!(
            errors.is_empty(),
            "SIMD comprehensive test file had syntax errors:\n{}",
            errors.join("\n")
        );
    }

    // ========================================================================
    // Individual instruction tests - useful for debugging specific failures
    // ========================================================================

    #[test]
    fn test_v128_load64_zero() {
        let source = r#"
            (module
                (memory 1)
                (func (result v128)
                    (v128.load64_zero (i32.const 0))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.load64_zero failed: {:?}", errors);
    }

    #[test]
    fn test_v128_load32_zero() {
        let source = r#"
            (module
                (memory 1)
                (func (result v128)
                    (v128.load32_zero (i32.const 0))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.load32_zero failed: {:?}", errors);
    }

    #[test]
    fn test_v128_load64_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128) (result v128)
                    (v128.load64_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.load64_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_load32_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128) (result v128)
                    (v128.load32_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.load32_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_load16_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128) (result v128)
                    (v128.load16_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.load16_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_load8_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128) (result v128)
                    (v128.load8_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.load8_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_store64_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128)
                    (v128.store64_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.store64_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_store32_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128)
                    (v128.store32_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.store32_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_store16_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128)
                    (v128.store16_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.store16_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_store8_lane() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128)
                    (v128.store8_lane 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.store8_lane failed: {:?}", errors);
    }

    #[test]
    fn test_v128_lane_with_offset() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128) (result v128)
                    (v128.load64_lane offset=8 1 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(
            errors.is_empty(),
            "v128.load64_lane with offset failed: {:?}",
            errors
        );
    }

    #[test]
    fn test_v128_lane_with_align() {
        let source = r#"
            (module
                (memory 1)
                (func (param $v v128)
                    (v128.store64_lane align=8 0 (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(
            errors.is_empty(),
            "v128.store64_lane with align failed: {:?}",
            errors
        );
    }

    // ========================================================================
    // Tests for other SIMD instructions that should continue to work
    // ========================================================================

    #[test]
    fn test_v128_basic_load_store() {
        let source = r#"
            (module
                (memory 1)
                (func (local $v v128)
                    (local.set $v (v128.load (i32.const 0)))
                    (v128.store (i32.const 0) (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(
            errors.is_empty(),
            "v128 basic load/store failed: {:?}",
            errors
        );
    }

    #[test]
    fn test_v128_const() {
        let source = r#"
            (module
                (func (result v128)
                    (v128.const i32x4 1 2 3 4)
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "v128.const failed: {:?}", errors);
    }

    #[test]
    fn test_i8x16_shuffle() {
        let source = r#"
            (module
                (func (param $a v128) (param $b v128) (result v128)
                    (i8x16.shuffle 0 1 2 3 4 5 6 7 16 17 18 19 20 21 22 23
                                   (local.get $a) (local.get $b))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "i8x16.shuffle failed: {:?}", errors);
    }

    #[test]
    fn test_extract_lane() {
        let source = r#"
            (module
                (func (param $v v128) (result i32)
                    (i32x4.extract_lane 0 (local.get $v))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "extract_lane failed: {:?}", errors);
    }

    #[test]
    fn test_replace_lane() {
        let source = r#"
            (module
                (func (param $v v128) (param $val i32) (result v128)
                    (i32x4.replace_lane 0 (local.get $v) (local.get $val))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "replace_lane failed: {:?}", errors);
    }

    #[test]
    fn test_splat_loads() {
        let source = r#"
            (module
                (memory 1)
                (func (result v128)
                    (v128.load64_splat (i32.const 0))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "splat loads failed: {:?}", errors);
    }

    #[test]
    fn test_extending_loads() {
        let source = r#"
            (module
                (memory 1)
                (func (result v128)
                    (v128.load8x8_s (i32.const 0))
                )
            )
        "#;
        let errors = check_syntax(source);
        assert!(errors.is_empty(), "extending loads failed: {:?}", errors);
    }
}
