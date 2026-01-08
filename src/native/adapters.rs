//! Adapters for converting between core types and tower-lsp types.
//!
//! The From implementations are defined in core::types, but this module
//! provides additional helper functions for common conversions.

use crate::core::types as core;
use tower_lsp::lsp_types as lsp;

/// Convert a core HoverResult to an LSP Hover
pub fn hover_result_to_lsp(result: core::HoverResult) -> lsp::Hover {
    lsp::Hover {
        contents: lsp::HoverContents::Markup(lsp::MarkupContent {
            kind: lsp::MarkupKind::Markdown,
            value: result.contents,
        }),
        range: result.range.map(|r| r.into()),
    }
}

/// Convert a core Range to an LSP Location with the given URI
pub fn range_to_location(range: core::Range, uri: lsp::Url) -> lsp::Location {
    lsp::Location {
        uri,
        range: range.into(),
    }
}

/// Convert a list of core Ranges to LSP Locations
pub fn ranges_to_locations(ranges: Vec<core::Range>, uri: lsp::Url) -> Vec<lsp::Location> {
    ranges
        .into_iter()
        .map(|r| lsp::Location {
            uri: uri.clone(),
            range: r.into(),
        })
        .collect()
}
