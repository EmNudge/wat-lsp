//! Convert byte-column feature results to the server's declared UTF-16 encoding.
//! Call only at publication/response boundaries, using the result's source snapshot.

use crate::core::text::TextIndex;
use tower_lsp::lsp_types::*;

pub(super) fn range(index: &TextIndex<'_>, range: Range) -> Range {
    index.range_to_utf16(range.into()).into()
}

pub(super) fn locations(source: &str, uri: &str, locations: &mut [Location]) {
    let index = TextIndex::new(source);
    for location in locations {
        if location.uri.as_str() == uri {
            location.range = range(&index, location.range);
        }
    }
}

pub(super) fn diagnostics(source: &str, uri: &Url, diagnostics: &mut [Diagnostic]) {
    let index = TextIndex::new(source);
    for diagnostic in diagnostics {
        diagnostic.range = range(&index, diagnostic.range);
        if let Some(related) = &mut diagnostic.related_information {
            for info in related {
                if &info.location.uri == uri {
                    info.location.range = range(&index, info.location.range);
                }
            }
        }
    }
}

pub(super) fn document_symbols(source: &str, symbols: &mut [DocumentSymbol]) {
    fn convert(index: &TextIndex<'_>, symbols: &mut [DocumentSymbol]) {
        for symbol in symbols {
            symbol.range = range(index, symbol.range);
            symbol.selection_range = range(index, symbol.selection_range);
            if let Some(children) = &mut symbol.children {
                convert(index, children);
            }
        }
    }
    convert(&TextIndex::new(source), symbols);
}
