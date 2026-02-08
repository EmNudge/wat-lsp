use crate::features::definition_core::provide_definition_core;
use crate::symbols::SymbolTable;
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

#[cfg(test)]
mod tests;

/// Main entry point for providing go-to-definition (native LSP).
pub fn provide_definition(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
    uri: &str,
) -> Option<Location> {
    let lsp_uri = Url::parse(uri).ok()?;
    let core_range = provide_definition_core(document, symbols, tree, position.into())?;
    Some(Location {
        uri: lsp_uri,
        range: core_range.into(),
    })
}
