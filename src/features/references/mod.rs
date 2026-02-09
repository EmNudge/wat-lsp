use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

// Re-export shared types from the core module
pub use crate::features::references_core::ReferenceTarget;

#[cfg(test)]
mod tests;

/// Identify what symbol the cursor is positioned on (native wrapper).
///
/// Accepts LSP `Position` and delegates to the shared core.
pub fn identify_symbol_at_position(
    document: &str,
    symbols: &crate::symbols::SymbolTable,
    tree: &Tree,
    position: Position,
) -> Option<ReferenceTarget> {
    crate::features::references_core::identify_symbol_at_position(
        document,
        symbols,
        tree,
        position.into(),
    )
}

/// Main entry point for providing find-references (native LSP wrapper).
///
/// Delegates to the shared core and converts `Vec<Range>` to `Vec<Location>`.
pub fn provide_references(
    document: &str,
    symbols: &crate::symbols::SymbolTable,
    tree: &Tree,
    position: Position,
    uri: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let ranges = crate::features::references_core::provide_references_core(
        document,
        symbols,
        tree,
        position.into(),
        include_declaration,
    );

    let lsp_uri = match Url::parse(uri) {
        Ok(u) => u,
        Err(_) => return vec![],
    };

    ranges
        .into_iter()
        .map(|r| Location {
            uri: lsp_uri.clone(),
            range: r.into(),
        })
        .collect()
}
