//! Shared tree-sitter syntax diagnostics for both native and WASM builds.
//!
//! Walks the parse tree and reports ERROR / MISSING nodes as diagnostics.

use crate::core::types::{Diagnostic, DiagnosticSeverity, Range};

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::{Node, Tree};

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::{Node, Tree};

/// Provide diagnostics for syntax errors in the document.
pub(crate) fn provide_tree_sitter_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    walk_tree_for_errors(tree.root_node(), source, &mut diagnostics);
    diagnostics
}

/// Recursively walk the tree and collect ERROR / MISSING nodes as diagnostics.
fn walk_tree_for_errors(node: Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "ERROR" {
        diagnostics.push(create_error_diagnostic(&node, source));
    }

    if node.is_missing() {
        diagnostics.push(create_missing_diagnostic(&node));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_errors(child, source, diagnostics);
    }
}

fn create_error_diagnostic(node: &Node, source: &str) -> Diagnostic {
    let range = node_to_core_range(node);
    let text = &source[node.byte_range()];

    let message = if text.trim().is_empty() {
        "Syntax error: unexpected token".to_string()
    } else {
        format!("Syntax error near: {}", text.lines().next().unwrap_or(text))
    };

    Diagnostic {
        range,
        severity: DiagnosticSeverity::Error,
        message,
        code: None,
        source: "wat-lsp",
    }
}

fn create_missing_diagnostic(node: &Node) -> Diagnostic {
    Diagnostic {
        range: node_to_core_range(node),
        severity: DiagnosticSeverity::Error,
        message: format!("Missing {}", node.kind()),
        code: None,
        source: "wat-lsp",
    }
}

fn node_to_core_range(node: &Node) -> Range {
    Range::from_coords(
        node.start_position().row as u32,
        node.start_position().column as u32,
        node.end_position().row as u32,
        node.end_position().column as u32,
    )
}
