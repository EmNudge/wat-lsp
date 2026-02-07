use crate::symbols::SymbolTable;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::{Node, Tree, TreeCursor};

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::{Node, Tree, TreeCursor};

/// Result type for folding ranges (protocol-independent)
#[derive(Debug, Clone)]
pub struct FoldingRangeResult {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: FoldingRangeKind,
}

#[derive(Debug, Clone, Copy)]
pub enum FoldingRangeKind {
    Region,
    Comment,
}

/// Provide folding ranges for a WAT document.
/// Uses both the symbol table (for functions) and tree-sitter (for nested blocks).
pub fn provide_folding_ranges(
    _source: &str,
    symbols: &SymbolTable,
    tree: &Tree,
) -> Vec<FoldingRangeResult> {
    let mut ranges = Vec::new();

    // Add function folding ranges from symbol table
    for func in &symbols.functions {
        if func.end_line > func.line {
            ranges.push(FoldingRangeResult {
                start_line: func.line,
                end_line: func.end_line,
                kind: FoldingRangeKind::Region,
            });
        }
    }

    // Walk tree-sitter tree to find other foldable regions
    let root = tree.root_node();
    let mut cursor = root.walk();
    collect_foldable_nodes(root, &mut cursor, &mut ranges);

    // Deduplicate ranges (in case tree-sitter and symbol table overlap)
    ranges.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.end_line.cmp(&b.end_line))
    });
    ranges.dedup_by(|a, b| a.start_line == b.start_line && a.end_line == b.end_line);

    ranges
}

/// Check if a node kind is foldable
fn is_foldable_kind(kind: &str) -> bool {
    matches!(
        kind,
        // Module-level definitions
        "module_field_type"
            | "module_field_global"
            | "module_field_table"
            | "module_field_memory"
            | "module_field_import"
            | "module_field_export"
            | "module_field_data"
            | "module_field_elem"
            | "module_field_tag"
            | "module_field_start"
            // Control flow blocks (both block and expr forms)
            | "block_block"
            | "block_loop"
            | "block_if"
            | "block_try"
            | "block_try_table"
            | "expr1_block"
            | "expr1_loop"
            | "expr1_if"
            | "expr1_try"
            | "expr1_try_table"
            // Type definitions
            | "rec_type"
            | "typedef"
            | "struct_type"
            | "array_type"
            | "func_type"
    )
}

/// Check if a node is a comment
fn is_comment_kind(kind: &str) -> bool {
    kind == "comment_block" || kind == "comment_line"
}

/// Recursively collect foldable nodes from the tree-sitter AST.
#[cfg(feature = "native")]
fn collect_foldable_nodes<'a>(
    node: Node<'a>,
    cursor: &mut TreeCursor<'a>,
    ranges: &mut Vec<FoldingRangeResult>,
) {
    let kind = node.kind();

    let is_foldable = is_foldable_kind(kind);
    let is_comment = is_comment_kind(kind);

    if is_foldable || is_comment {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;

        // Only fold if it spans multiple lines
        if end_line > start_line {
            ranges.push(FoldingRangeResult {
                start_line,
                end_line,
                kind: if is_comment {
                    FoldingRangeKind::Comment
                } else {
                    FoldingRangeKind::Region
                },
            });
        }
    }

    // Recurse into children
    for child in node.children(cursor) {
        let mut child_cursor = child.walk();
        collect_foldable_nodes(child, &mut child_cursor, ranges);
    }
}

/// Recursively collect foldable nodes from the tree-sitter AST (WASM version).
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn collect_foldable_nodes(
    node: Node,
    cursor: &mut TreeCursor,
    ranges: &mut Vec<FoldingRangeResult>,
) {
    let kind = node.kind();

    let is_foldable = is_foldable_kind(&kind);
    let is_comment = is_comment_kind(&kind);

    if is_foldable || is_comment {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;

        // Only fold if it spans multiple lines
        if end_line > start_line {
            ranges.push(FoldingRangeResult {
                start_line,
                end_line,
                kind: if is_comment {
                    FoldingRangeKind::Comment
                } else {
                    FoldingRangeKind::Region
                },
            });
        }
    }

    // Recurse into children
    for child in node.children(cursor) {
        let mut child_cursor = child.walk();
        collect_foldable_nodes(child, &mut child_cursor, ranges);
    }
}

#[cfg(feature = "native")]
use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind as LspFoldingRangeKind};

/// Convert to LSP types for the native server
#[cfg(feature = "native")]
pub fn provide_folding_ranges_lsp(
    source: &str,
    symbols: &SymbolTable,
    tree: &Tree,
) -> Vec<FoldingRange> {
    provide_folding_ranges(source, symbols, tree)
        .into_iter()
        .map(|r| FoldingRange {
            start_line: r.start_line,
            start_character: None,
            end_line: r.end_line,
            end_character: None,
            kind: Some(match r.kind {
                FoldingRangeKind::Region => LspFoldingRangeKind::Region,
                FoldingRangeKind::Comment => LspFoldingRangeKind::Comment,
            }),
            collapsed_text: None,
        })
        .collect()
}
