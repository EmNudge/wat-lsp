use crate::symbols::{Function, SymbolTable};
use tower_lsp::lsp_types::Position;
use tree_sitter::{Node, Tree};

/// Unified context for instruction type identification.
/// Used across hover, definition, references, completion, and diagnostics.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum InstructionContext {
    Call,     // call instruction
    Global,   // global.get/set
    Local,    // local.get/set/tee
    Branch,   // br/br_if/br_table
    Block,    // block/loop labels
    Table,    // table operations
    Memory,   // memory operations
    Type,     // type definitions/uses
    Tag,      // throw/try
    Function, // function definition
    General,  // fallback
}

/// Determine instruction context by walking up the AST tree.
/// Used by hover, definition, and completion modules.
pub fn determine_instruction_context(node: Node, document: &str) -> InstructionContext {
    let mut current = node;

    loop {
        let kind = current.kind();

        // Check for instruction contexts
        if kind == "instr_plain" || kind == "expr1_plain" {
            let instr_text = &document[current.byte_range()];

            if instr_text.contains("call") {
                return InstructionContext::Call;
            } else if instr_text.contains("local.") {
                return InstructionContext::Local;
            } else if instr_text.contains("global.") {
                return InstructionContext::Global;
            } else if instr_text.starts_with("br") || instr_text.contains(" br") {
                return InstructionContext::Branch;
            } else if instr_text.contains("table.") {
                return InstructionContext::Table;
            } else if instr_text.contains("memory.") {
                return InstructionContext::Memory;
            } else if instr_text.contains("struct.")
                || instr_text.contains("array.")
                || instr_text.contains("ref.cast")
                || instr_text.contains("ref.test")
            {
                return InstructionContext::Type;
            } else if instr_text.contains("throw") || instr_text.contains("rethrow") {
                return InstructionContext::Tag;
            }
        }

        // Check for block/loop contexts
        if kind == "instr_block" || kind == "instr_loop" {
            return InstructionContext::Block;
        }

        // Check for function definition
        if kind == "module_field_func" {
            return InstructionContext::Function;
        }

        // Check for type definition
        if kind == "module_field_type" || kind == "type_use" {
            return InstructionContext::Type;
        }

        // Check for tag definition
        if kind == "module_field_tag" {
            return InstructionContext::Tag;
        }

        // Walk up the tree
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    InstructionContext::General
}

/// Determine instruction context from a single node (no tree walking).
/// Used by references and semantic diagnostics where only the current node should be checked.
pub fn determine_instruction_context_at_node(node: &Node, document: &str) -> InstructionContext {
    let kind = node.kind();

    // Only check instr_plain nodes for instruction context
    if kind == "instr_plain" || kind == "expr1_plain" {
        let instr_text = &document[node.byte_range()];

        if instr_text.starts_with("br") || instr_text.contains(" br") {
            return InstructionContext::Branch;
        } else if instr_text.contains("call") && !instr_text.contains("call_indirect") {
            return InstructionContext::Call;
        } else if instr_text.contains("local.") {
            return InstructionContext::Local;
        } else if instr_text.contains("global.") {
            return InstructionContext::Global;
        } else if instr_text.contains("table.") {
            return InstructionContext::Table;
        } else if instr_text.contains("memory.") {
            return InstructionContext::Memory;
        } else if instr_text.contains("struct.")
            || instr_text.contains("array.")
            || instr_text.contains("ref.cast")
            || instr_text.contains("ref.test")
        {
            return InstructionContext::Type;
        } else if instr_text.contains("throw") || instr_text.contains("rethrow") {
            return InstructionContext::Tag;
        }
    }

    // Check for type use context
    if kind == "type_use" {
        return InstructionContext::Type;
    }

    InstructionContext::General
}

/// Determine context from line text (fallback for incomplete code).
/// Used when AST-based detection returns General.
pub fn determine_context_from_line(line: &str) -> InstructionContext {
    if line.contains("call") {
        InstructionContext::Call
    } else if line.contains("global") {
        InstructionContext::Global
    } else if line.contains("local") {
        InstructionContext::Local
    } else if line.contains("br") {
        InstructionContext::Branch
    } else if line.contains("block") || line.contains("loop") {
        InstructionContext::Block
    } else if line.contains("table") {
        InstructionContext::Table
    } else if line.contains("memory") {
        InstructionContext::Memory
    } else if line.contains("type")
        || line.contains("struct")
        || line.contains("array")
        || line.contains("ref.")
    {
        InstructionContext::Type
    } else if line.contains("throw") || line.contains("tag") {
        InstructionContext::Tag
    } else if line.contains("func") {
        InstructionContext::Function
    } else {
        InstructionContext::General
    }
}

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

/// Get the word at the specified position in a document.
/// A word includes alphanumerics, underscores, dollar signs, dots, and hyphens.
pub fn get_word_at_position(document: &str, position: Position) -> Option<String> {
    let line = get_line_at_position(document, position.line as usize)?;
    let col = position.character as usize;

    if col > line.len() {
        return None;
    }

    // Find word boundaries
    let mut start = col;
    let mut end = col;

    let chars: Vec<char> = line.chars().collect();

    // Move back to start of word
    while start > 0 && is_word_char(chars.get(start - 1).copied()?) {
        start -= 1;
    }

    // Move forward to end of word
    while end < chars.len() && is_word_char(chars.get(end).copied()?) {
        end += 1;
    }

    if start < end {
        Some(chars[start..end].iter().collect())
    } else {
        None
    }
}

/// Check if a character is part of a word in WAT (alphanumeric, _, $, ., -)
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || c == '.' || c == '-'
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

/// Check if the given position is inside a comment (block or line)
pub fn is_inside_comment(tree: &Tree, source: &str, position: Position) -> bool {
    let byte_offset = position_to_byte(source, position);
    let root = tree.root_node();

    is_inside_comment_node(root, byte_offset)
}

/// Recursively check if byte_offset is inside a comment node
fn is_inside_comment_node(node: Node, byte_offset: usize) -> bool {
    let range = node.byte_range();
    if !(range.start <= byte_offset && byte_offset < range.end) {
        return false;
    }

    let kind = node.kind();
    if kind == "comment_block"
        || kind == "comment_line"
        || kind == "comment_block_annot"
        || kind == "comment_line_annot"
    {
        return true;
    }

    // Check children (including extra nodes like comments)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_inside_comment_node(child, byte_offset) {
            return true;
        }
    }

    false
}

/// Recursively find the deepest (most specific) node containing the byte offset
fn find_deepest_node(node: Node, byte_offset: usize) -> Option<Node> {
    let range = node.byte_range();
    // Check if byte_offset is within the node (inclusive of start, exclusive of end)
    // OR if it's at the end of the node (to handle cursor at end of word)
    if !(range.start <= byte_offset && byte_offset <= range.end) {
        return None;
    }

    let mut cursor = node.walk();
    let mut best_child = None;
    for child in node.children(&mut cursor) {
        let child_range = child.byte_range();
        let contains_or_adjacent =
            child_range.start <= byte_offset && byte_offset <= child_range.end;
        if contains_or_adjacent {
            if let Some(found) = find_deepest_node(child, byte_offset) {
                // Prefer the child that properly contains the offset over one where offset is at the end
                if byte_offset < child_range.end || best_child.is_none() {
                    best_child = Some(found);
                    if byte_offset < child_range.end {
                        // If offset is properly inside, return immediately
                        return Some(found);
                    }
                }
            }
        }
    }

    if let Some(child) = best_child {
        return Some(child);
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

    #[test]
    fn test_node_at_position_identifier() {
        // We need tree-sitter-wasm to test this
        // Note: verify if tree-sitter-wasm is available in test context.
        // It is a dependency.

        // Setup parser
        use crate::tree_sitter_bindings;
        let mut parser = tree_sitter_bindings::create_parser();

        let source = "(func $test)";
        let tree = parser.parse(source, None).unwrap();

        // Position at '$' of $test (line 0, char 7)
        let position = Position::new(0, 7);
        let node = node_at_position(&tree, source, position).unwrap();

        assert_eq!(node.kind(), "identifier");
        assert_eq!(&source[node.byte_range()], "$test");

        // Position at 't' of $test (line 0, char 8)
        let position = Position::new(0, 8);
        let node = node_at_position(&tree, source, position).unwrap();

        assert_eq!(node.kind(), "identifier");
        assert_eq!(&source[node.byte_range()], "$test");
    }
}
