use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

/// Provide diagnostics for syntax errors in the document
pub fn provide_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
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
    let range = node_to_range(node);
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
    let range = node_to_range(node);

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

/// Convert a tree-sitter node to an LSP range
fn node_to_range(node: Node) -> Range {
    let start_point = node.start_position();
    let end_point = node.end_position();

    Range {
        start: Position {
            line: start_point.row as u32,
            character: start_point.column as u32,
        },
        end: Position {
            line: end_point.row as u32,
            character: end_point.column as u32,
        },
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

        let diagnostics = provide_diagnostics(&tree, document);
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

        let diagnostics = provide_diagnostics(&tree, document);
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

        let diagnostics = provide_diagnostics(&tree, document);
        // Incomplete code may or may not produce errors depending on grammar
        // Just ensure we don't crash
        let _ = diagnostics;
    }

    #[test]
    fn test_diagnostic_range() {
        let document = "(func $test (param $x i32\n  (local.get $x))"; // Missing closing paren
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();

        let diagnostics = provide_diagnostics(&tree, document);
        assert!(!diagnostics.is_empty());

        // Verify that diagnostics have valid ranges
        for diagnostic in &diagnostics {
            assert!(diagnostic.range.start.line <= diagnostic.range.end.line);
            if diagnostic.range.start.line == diagnostic.range.end.line {
                assert!(diagnostic.range.start.character <= diagnostic.range.end.character);
            }
        }
    }
}
