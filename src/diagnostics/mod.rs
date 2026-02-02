mod semantic_diagnostics;
mod tree_sitter_diagnostics;
mod wast_validator;

pub use semantic_diagnostics::provide_semantic_diagnostics;
pub use tree_sitter_diagnostics::provide_tree_sitter_diagnostics;
pub use wast_validator::validate_wat;

use tower_lsp::lsp_types::Diagnostic;

/// Merge diagnostics from tree-sitter, semantic, and wast, sorted by position
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
    all
}
