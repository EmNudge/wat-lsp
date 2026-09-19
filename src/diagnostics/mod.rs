mod semantic_diagnostics;
mod tree_sitter_diagnostics;
mod wast_validator;

pub use semantic_diagnostics::provide_semantic_diagnostics;
pub use semantic_diagnostics::provide_semantic_diagnostics_multi;
pub use tree_sitter_diagnostics::provide_tree_sitter_diagnostics;
pub use wast_validator::validate_wat;

use tower_lsp::lsp_types::Diagnostic;

/// Merge diagnostics from tree-sitter, semantic, and wast, sorted by position.
///
/// Exact-duplicate diagnostics are removed. The independent syntax, semantic, and
/// wast passes can each surface the same problem at the same location (e.g. an
/// undefined reference flagged by both the semantic checker and the wast
/// validator), which would otherwise show up as redundant squiggles in the
/// editor. Two diagnostics are considered duplicates when their range, severity,
/// code, and message all match.
pub fn merge_all_diagnostics(
    tree_sitter: Vec<Diagnostic>,
    semantic: Vec<Diagnostic>,
    wast: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let mut all = tree_sitter;
    all.extend(semantic);
    all.extend(wast);
    all.sort_by(|a, b| {
        a.range
            .start
            .line
            .cmp(&b.range.start.line)
            .then(a.range.start.character.cmp(&b.range.start.character))
    });
    dedup_exact(&mut all);
    all
}

/// Remove exact-duplicate diagnostics in place, preserving order.
///
/// `all` is expected to be sorted by start position so that duplicates from
/// different passes end up adjacent; any remaining out-of-order duplicates are
/// still caught by the retained set.
fn dedup_exact(all: &mut Vec<Diagnostic>) {
    let mut seen: std::collections::HashSet<DiagnosticKey> = std::collections::HashSet::new();
    all.retain(|d| seen.insert(diagnostic_key(d)));
}

/// A comparable, hashable identity for a diagnostic: range, severity, code, and
/// message. Two diagnostics with the same key are treated as exact duplicates.
type DiagnosticKey = ((u32, u32, u32, u32), Option<i32>, Option<String>, String);

fn diagnostic_key(d: &Diagnostic) -> DiagnosticKey {
    let range = (
        d.range.start.line,
        d.range.start.character,
        d.range.end.line,
        d.range.end.character,
    );
    let severity = d.severity.map(|s| match s {
        tower_lsp::lsp_types::DiagnosticSeverity::ERROR => 1,
        tower_lsp::lsp_types::DiagnosticSeverity::WARNING => 2,
        tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION => 3,
        tower_lsp::lsp_types::DiagnosticSeverity::HINT => 4,
        _ => 0,
    });
    let code = d.code.as_ref().map(|c| match c {
        tower_lsp::lsp_types::NumberOrString::Number(n) => n.to_string(),
        tower_lsp::lsp_types::NumberOrString::String(s) => s.clone(),
    });
    (range, severity, code, d.message.clone())
}

#[cfg(test)]
mod tests {
    use super::merge_all_diagnostics;
    use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};

    fn diag(line: u32, msg: &str, code: &str) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position { line, character: 0 },
                end: Position { line, character: 1 },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(code.to_string())),
            message: msg.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn merge_drops_exact_duplicates_across_passes() {
        // The same undefined-reference error surfaced by two passes.
        let semantic = vec![diag(4, "undefined function '$foo'", "undefined-ref")];
        let wast = vec![diag(4, "undefined function '$foo'", "undefined-ref")];
        let merged = merge_all_diagnostics(vec![], semantic, wast);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_keeps_distinct_diagnostics_and_sorts() {
        let syntax = vec![diag(5, "syntax error", "syntax")];
        let semantic = vec![
            diag(2, "type mismatch", "type-mismatch"),
            // Distinct message at the same position must be preserved.
            diag(2, "arity error", "arity"),
        ];
        let merged = merge_all_diagnostics(syntax, semantic, vec![]);
        assert_eq!(merged.len(), 3);
        // Sorted by start line.
        assert_eq!(merged[0].range.start.line, 2);
        assert_eq!(merged[2].range.start.line, 5);
    }
}
