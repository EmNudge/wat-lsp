use tower_lsp::lsp_types::Diagnostic;
use tree_sitter::Tree;

/// Provide diagnostics for syntax errors in the document.
///
/// Delegates to the shared `diagnostics_core::tree_sitter` implementation and
/// converts core diagnostics to tower-lsp types.
pub fn provide_tree_sitter_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    crate::diagnostics_core::tree_sitter::provide_tree_sitter_diagnostics(tree, source)
        .into_iter()
        .map(Diagnostic::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree_sitter_bindings::create_parser;
    use tower_lsp::lsp_types::DiagnosticSeverity;

    #[test]
    fn test_no_errors_in_valid_code() {
        let document = "(func $test (param $x i32) (result i32)\n  (local.get $x))";
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid code should have no diagnostics"
        );
    }

    #[test]
    fn test_syntax_error_detected() {
        let document = "(func $test (param $x i32\n  (local.get $x))"; // Missing closing paren
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            !diagnostics.is_empty(),
            "Invalid code should have diagnostics"
        );

        assert!(
            diagnostics
                .iter()
                .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
            "Should have at least one error diagnostic"
        );
    }

    #[test]
    fn test_incomplete_expression() {
        let document = "(func $test\n  local.get"; // Incomplete
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        let _ = diagnostics;
    }

    #[test]
    fn test_diagnostic_range() {
        let document = "(func $test (param $x i32\n  (local.get $x))"; // Missing closing paren
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(!diagnostics.is_empty());

        for diagnostic in &diagnostics {
            assert!(diagnostic.range.start.line <= diagnostic.range.end.line);
            if diagnostic.range.start.line == diagnostic.range.end.line {
                assert!(diagnostic.range.start.character <= diagnostic.range.end.character);
            }
        }
    }

    #[test]
    fn test_ref_null_func_and_ref_is_null() {
        let document = r#"(module
  (func $test (result i32)
    ref.null func
    ref.is_null
  )
)"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            diagnostics.is_empty(),
            "ref.null func + ref.is_null should not produce syntax errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_ref_null_funcref_lenient() {
        let document = r#"(module
  (func $test (result i32)
    ref.null funcref
    ref.is_null
  )
)"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            diagnostics.is_empty(),
            "ref.null funcref should be accepted leniently, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_ref_func() {
        let document = r#"(module
  (func $target)
  (func $test (result funcref)
    ref.func $target
  )
)"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            diagnostics.is_empty(),
            "ref.func $name should not produce syntax errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_ref_is_null_standalone() {
        let document = r#"(module
  (func $test (param funcref) (result i32)
    local.get 0
    ref.is_null
  )
)"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            diagnostics.is_empty(),
            "ref.is_null should not produce syntax errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_elem_with_ref_func_and_ref_null() {
        let document = r#"(module
  (func $f1)
  (table 2 funcref)
  (elem (i32.const 0) funcref (ref.func $f1) (ref.null func))
)"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            diagnostics.is_empty(),
            "elem with ref.func and ref.null func should not produce syntax errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_error_does_not_cascade_into_children() {
        // A single malformed region should not emit a flood of nested
        // ERROR/MISSING diagnostics from tree-sitter's recovery. We report one
        // diagnostic per top-level ERROR node and stop descending.
        let document = "(func $f (@#$%^ garbage )))) more (((( junk";
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            !diagnostics.is_empty(),
            "malformed input should report at least one diagnostic"
        );
        // Without cascade suppression this kind of input produces many derived
        // diagnostics; assert we stay small.
        assert!(
            diagnostics.len() <= 3,
            "expected a small number of diagnostics (no cascade), got {}: {:?}",
            diagnostics.len(),
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_single_error_still_reported() {
        // Cascade suppression must not silence the root error itself.
        let document = "(func $test (param $x i32\n  (local.get $x))"; // missing paren
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
            "root syntax error must still be reported"
        );
    }

    #[test]
    fn test_store8_load8_valid() {
        let document = r#"(module
  (memory 1)
  (func $test
    (i32.store8 (i32.const 0) (i32.const 42))
    (drop (i32.load8_u (i32.const 0)))
  )
)"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        for diag in &diagnostics {
            eprintln!("Diagnostic: {}", diag.message);
        }
        assert!(
            diagnostics.is_empty(),
            "i32.store8 and i32.load8_u should not produce syntax errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
}
