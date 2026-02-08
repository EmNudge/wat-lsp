use crate::features::document_symbols_core::provide_document_symbols_core;
use crate::symbols::*;
use tower_lsp::lsp_types::DocumentSymbol;

#[cfg(test)]
mod tests;

/// Provide document symbols for the outline view.
/// Returns a hierarchical list of symbols where functions contain their locals/params as children.
pub fn provide_document_symbols(symbols: &SymbolTable) -> Vec<DocumentSymbol> {
    provide_document_symbols_core(symbols)
        .into_iter()
        .map(Into::into)
        .collect()
}
