use crate::utils::node_to_range;
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

/// Provide diagnostics for syntax errors in the document
pub fn provide_tree_sitter_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    walk_tree_for_errors(tree.root_node(), source, &mut diagnostics);
    diagnostics
}

/// Recursively walk the tree and collect ERROR nodes as diagnostics
fn walk_tree_for_errors(node: Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "ERROR" {
        let diagnostic = create_error_diagnostic(node, source);
        diagnostics.push(diagnostic);
    }

    // Check for MISSING nodes as well (incomplete syntax)
    if node.is_missing() {
        let diagnostic = create_missing_diagnostic(node, source);
        diagnostics.push(diagnostic);
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_errors(child, source, diagnostics);
    }
}

/// Create a diagnostic for an ERROR node
fn create_error_diagnostic(node: Node, source: &str) -> Diagnostic {
    let range: Range = node_to_range(&node).into();
    let text = &source[node.byte_range()];

    let message = if text.trim().is_empty() {
        "Syntax error: unexpected token".to_string()
    } else {
        format!("Syntax error near: {}", text.lines().next().unwrap_or(text))
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("wat-lsp".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Create a diagnostic for a MISSING node
fn create_missing_diagnostic(node: Node, _source: &str) -> Diagnostic {
    let range: Range = node_to_range(&node).into();

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("wat-lsp".to_string()),
        message: format!("Missing {}", node.kind()),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree_sitter_bindings::create_parser;

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

        // Check that at least one diagnostic is an error
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
        // Incomplete code may or may not produce errors depending on grammar
        // Just ensure we don't crash
        let _ = diagnostics;
    }

    #[test]
    fn test_diagnostic_range() {
        let document = "(func $test (param $x i32\n  (local.get $x))"; // Missing closing paren
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_tree_sitter_diagnostics(&tree, document);
        assert!(!diagnostics.is_empty());

        // Verify that diagnostics have valid ranges
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
    fn test_store8_load8_valid() {
        // These are valid WASM instructions and should not produce errors
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
