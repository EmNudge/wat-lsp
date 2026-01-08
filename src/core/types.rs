//! Protocol-independent types for positions and ranges.
//!
//! These types have no dependencies on tower-lsp or wasm-bindgen,
//! allowing them to be used in both native and WASM builds.

use serde::{Deserialize, Serialize};

#[cfg(feature = "native")]
use tower_lsp::lsp_types as lsp;

/// A position in a text document (0-indexed line and character)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// A range in a text document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Create a range from line/column coordinates
    pub fn from_coords(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Self {
        Self {
            start: Position::new(start_line, start_char),
            end: Position::new(end_line, end_char),
        }
    }
}

/// Hover result containing markdown content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverResult {
    /// Markdown-formatted content to display
    pub contents: String,
    /// Optional range to highlight
    pub range: Option<Range>,
}

impl HoverResult {
    pub fn new(contents: String) -> Self {
        Self {
            contents,
            range: None,
        }
    }

    pub fn with_range(contents: String, range: Range) -> Self {
        Self {
            contents,
            range: Some(range),
        }
    }
}

/// Definition result - the range where a symbol is defined
pub type DefinitionResult = Option<Range>;

/// References result - list of ranges where a symbol is referenced
pub type ReferencesResult = Vec<Range>;

// Conversion implementations for native builds (tower-lsp types)
#[cfg(feature = "native")]
impl From<lsp::Position> for Position {
    fn from(p: lsp::Position) -> Self {
        Self {
            line: p.line,
            character: p.character,
        }
    }
}

#[cfg(feature = "native")]
impl From<Position> for lsp::Position {
    fn from(p: Position) -> Self {
        Self {
            line: p.line,
            character: p.character,
        }
    }
}

#[cfg(feature = "native")]
impl From<lsp::Range> for Range {
    fn from(r: lsp::Range) -> Self {
        Self {
            start: r.start.into(),
            end: r.end.into(),
        }
    }
}

#[cfg(feature = "native")]
impl From<Range> for lsp::Range {
    fn from(r: Range) -> Self {
        Self {
            start: r.start.into(),
            end: r.end.into(),
        }
    }
}
