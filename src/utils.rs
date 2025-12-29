use crate::symbols::{Function, SymbolTable};
use tower_lsp::lsp_types::Position;
use tree_sitter::{Node, Tree};

/// Find the function that contains the given position.
/// Returns the last function whose start line is at or before the position.
pub fn find_containing_function(symbols: &SymbolTable, position: Position) -> Option<&Function> {
    // FIXED: Iterate in reverse to find the LAST (most recent) function
    // that starts at or before this position, not the first one
    symbols
        .functions
        .iter()
        .rev()
        .find(|func| func.line <= position.line)
}

/// Get the line at the specified position in a document.
/// Returns a borrowed string slice to avoid unnecessary allocations.
pub fn get_line_at_position(document: &str, line_num: usize) -> Option<&str> {
    document.lines().nth(line_num)
}

/// Convert an LSP Position to a byte offset in the source text
pub fn position_to_byte(source: &str, position: Position) -> usize {
    let mut byte_offset = 0;

    for (current_line, line) in source.lines().enumerate() {
        if current_line == position.line as usize {
            // Add character offset within this line
            let char_offset = position.character as usize;
            let line_bytes: Vec<_> = line.char_indices().collect();
            if char_offset < line_bytes.len() {
                return byte_offset + line_bytes[char_offset].0;
            } else {
                return byte_offset + line.len();
            }
        }
        byte_offset += line.len() + 1; // +1 for newline
    }

    byte_offset
}

/// Find the AST node at the given position
pub fn node_at_position<'a>(tree: &'a Tree, source: &str, position: Position) -> Option<Node<'a>> {
    let byte_offset = position_to_byte(source, position);
    let root = tree.root_node();

    find_deepest_node(root, byte_offset)
}

/// Recursively find the deepest (most specific) node containing the byte offset
fn find_deepest_node(node: Node, byte_offset: usize) -> Option<Node> {
    if !node.byte_range().contains(&byte_offset) {
        return None;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_deepest_node(child, byte_offset) {
            return Some(found);
        }
    }

    Some(node)
}

/// Apply a text edit to a string in-place
/// Returns the new end position after the edit
pub fn apply_text_edit(
    text: &mut String,
    start: Position,
    end: Position,
    new_text: &str,
) -> Position {
    let start_byte = position_to_byte(text, start);
    let end_byte = position_to_byte(text, end);

    // Remove the old text and insert the new text
    text.replace_range(start_byte..end_byte, new_text);

    // Calculate the new end position
    calculate_position_after_edit(text, start, new_text)
}

/// Calculate the position after inserting text at a given position
fn calculate_position_after_edit(_text: &str, start: Position, inserted_text: &str) -> Position {
    if inserted_text.is_empty() {
        return start;
    }

    let newline_count = inserted_text.matches('\n').count();

    if newline_count == 0 {
        // Single line insert
        Position {
            line: start.line,
            character: start.character + inserted_text.len() as u32,
        }
    } else {
        // Multi-line insert
        let last_line = inserted_text.lines().last().unwrap_or("");
        Position {
            line: start.line + newline_count as u32,
            character: last_line.len() as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_text_edit_insert() {
        let mut text = String::from("hello world");
        let start = Position::new(0, 6);
        let end = Position::new(0, 6);

        apply_text_edit(&mut text, start, end, "beautiful ");
        assert_eq!(text, "hello beautiful world");
    }

    #[test]
    fn test_apply_text_edit_replace() {
        let mut text = String::from("hello world");
        let start = Position::new(0, 6);
        let end = Position::new(0, 11);

        apply_text_edit(&mut text, start, end, "Rust");
        assert_eq!(text, "hello Rust");
    }

    #[test]
    fn test_apply_text_edit_delete() {
        let mut text = String::from("hello world");
        let start = Position::new(0, 5);
        let end = Position::new(0, 11);

        apply_text_edit(&mut text, start, end, "");
        assert_eq!(text, "hello");
    }

    #[test]
    fn test_apply_text_edit_multiline() {
        let mut text = String::from("hello world");
        let start = Position::new(0, 6);
        let end = Position::new(0, 11);

        let new_end = apply_text_edit(&mut text, start, end, "beautiful\nRust");
        assert_eq!(text, "hello beautiful\nRust");
        assert_eq!(new_end.line, 1);
        assert_eq!(new_end.character, 4);
    }
}
