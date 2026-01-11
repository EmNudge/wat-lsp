use crate::core::types::Position;
use crate::symbols::{Function, SymbolTable};

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::{Node, Tree};

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::{Node, Tree};

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
    Data,     // data segment operations (memory.init, data.drop)
    Elem,     // elem segment operations (elem, table.init, elem.drop)
    General,  // fallback
}

/// Determine instruction context by walking up the AST tree.
/// Used by hover, definition, and completion modules.
pub fn determine_instruction_context(node: Node, document: &str) -> InstructionContext {
    // First check if we're in a catch_clause - this needs special handling
    // because catch clauses contain both tag and label references
    if let Some(context) = determine_catch_clause_context(&node, document) {
        return context;
    }

    let mut current = node;

    loop {
        let kind = current.kind();

        // Check for instruction contexts
        if kind == "instr_plain"
            || kind == "expr1_plain"
            || kind == "instr_call"
            || kind == "expr1_call"
        {
            let instr_text = &document[current.byte_range()];

            if instr_text.contains("call") {
                return InstructionContext::Call;
            } else if instr_text.contains("local.") {
                return InstructionContext::Local;
            } else if instr_text.contains("global.") {
                return InstructionContext::Global;
            } else if instr_text.starts_with("br") || instr_text.contains(" br") {
                return InstructionContext::Branch;
            // Check data/elem segment operations BEFORE general table/memory
            } else if instr_text.contains("memory.init") || instr_text.contains("data.drop") {
                return InstructionContext::Data;
            } else if instr_text.contains("table.init") || instr_text.contains("elem.drop") {
                return InstructionContext::Elem;
            } else if instr_text.contains("table.") {
                return InstructionContext::Table;
            } else if instr_text.contains("memory.")
                || instr_text.contains(".load")
                || instr_text.contains(".store")
            {
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

        // Check for block/loop contexts (both instr_* and block_* variants)
        if kind == "instr_block"
            || kind == "instr_loop"
            || kind == "block_block"
            || kind == "block_loop"
            || kind == "block_if"
            || kind == "block_try"
            || kind == "block_try_table"
            || kind == "expr1_block"
            || kind == "expr1_loop"
            || kind == "expr1_if"
            || kind == "expr1_try"
        {
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

        // Check for data segment definition
        if kind == "module_field_data" {
            return InstructionContext::Data;
        }

        // Check for elem segment definition
        if kind == "module_field_elem" {
            return InstructionContext::Elem;
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
        // Extract just the first token (instruction name) to avoid matching nested instructions
        let first_token = instr_text.split_whitespace().next().unwrap_or("");

        // Check GC/struct/array instructions first (they take type indices)
        // Also includes call_ref and return_call_ref which take type indices
        if first_token.starts_with("struct.")
            || first_token.starts_with("array.")
            || first_token == "ref.cast"
            || first_token == "ref.test"
            || first_token == "call_ref"
            || first_token == "return_call_ref"
        {
            return InstructionContext::Type;
        } else if first_token.starts_with("br") {
            return InstructionContext::Branch;
        } else if first_token.starts_with("call") && first_token != "call_indirect" {
            return InstructionContext::Call;
        } else if first_token.starts_with("local.") {
            return InstructionContext::Local;
        } else if first_token.starts_with("global.") {
            return InstructionContext::Global;
        } else if first_token.starts_with("table.") {
            return InstructionContext::Table;
        } else if first_token.starts_with("memory.") {
            return InstructionContext::Memory;
        } else if first_token == "throw" || first_token == "rethrow" {
            return InstructionContext::Tag;
        }
    }

    // Check for type use context
    if kind == "type_use" {
        return InstructionContext::Type;
    }

    // Check for ref type context (e.g., (ref $point) in result/param types)
    if kind == "ref_type_ref" || kind == "ref_type" {
        return InstructionContext::Type;
    }

    // Check for catch_clause context - need to determine if we're on tag or label
    // Walk up to find if we're inside a catch_clause
    if let Some(context) = determine_catch_clause_context(node, document) {
        return context;
    }

    InstructionContext::General
}

/// Determine the context for a node inside a catch_clause.
/// For (catch $tag $label): first index is Tag, second is Branch
/// For (catch_all $label): single index is Branch
#[cfg(feature = "native")]
fn determine_catch_clause_context(node: &Node, document: &str) -> Option<InstructionContext> {
    // Walk up to find the catch_clause and track our position
    let mut current = *node;
    let mut index_node: Option<Node> = None;

    loop {
        let kind = current.kind();

        // Track if we pass through an index node
        if kind == "index" {
            index_node = Some(current);
        }

        if kind == "catch_clause" {
            // Found the catch_clause, now determine position
            let text = &document[current.byte_range()];

            // Determine if this is catch/catch_ref (has tag) or catch_all/catch_all_ref (no tag)
            let has_tag = text.contains("catch_ref")
                || (text.contains("catch") && !text.contains("catch_all"));

            if let Some(idx_node) = index_node {
                // Find the position of this index among all index children
                let mut cursor = current.walk();
                let indices: Vec<_> = current
                    .children(&mut cursor)
                    .filter(|c| c.kind() == "index")
                    .collect();

                for (i, idx) in indices.iter().enumerate() {
                    if idx.byte_range() == idx_node.byte_range() {
                        // Found our index - first index in catch/catch_ref is tag, rest are labels
                        if has_tag && i == 0 {
                            return Some(InstructionContext::Tag);
                        } else {
                            return Some(InstructionContext::Branch);
                        }
                    }
                }
            }

            // Inside catch_clause but not in an index - shouldn't happen for identifiers
            return None;
        }

        // Walk up the tree
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    None
}

/// Determine the context for a node inside a catch_clause (WASM version)
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn determine_catch_clause_context(node: &Node, document: &str) -> Option<InstructionContext> {
    // Walk up to find the catch_clause and track our position
    let mut current = node.clone();
    let mut index_node: Option<Node> = None;

    loop {
        let kind = current.kind();

        // Track if we pass through an index node
        if kind == "index" {
            index_node = Some(current.clone());
        }

        if kind == "catch_clause" {
            // Found the catch_clause, now determine position
            let text = &document[current.byte_range()];

            // Determine if this is catch/catch_ref (has tag) or catch_all/catch_all_ref (no tag)
            let has_tag = text.contains("catch_ref")
                || (text.contains("catch") && !text.contains("catch_all"));

            if let Some(idx_node) = index_node {
                // Find the position of this index among all index children
                let mut cursor = current.walk();
                let indices: Vec<_> = current
                    .children(&mut cursor)
                    .filter(|c| c.kind() == "index")
                    .collect();

                for (i, idx) in indices.iter().enumerate() {
                    if idx.byte_range() == idx_node.byte_range() {
                        // Found our index - first index in catch/catch_ref is tag, rest are labels
                        if has_tag && i == 0 {
                            return Some(InstructionContext::Tag);
                        } else {
                            return Some(InstructionContext::Branch);
                        }
                    }
                }
            }

            // Inside catch_clause but not in an index - shouldn't happen for identifiers
            return None;
        }

        // Walk up the tree
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    None
}

/// Check if a line contains a keyword as an instruction or declaration (not as part of an identifier).
/// Keywords are matched when they appear as:
/// - An instruction (e.g., "call ", "local.get", "memory.grow")
/// - A declaration (e.g., "(func ", "(global ", "(memory ")
fn line_contains_keyword(line: &str, keyword: &str) -> bool {
    for (i, _) in line.match_indices(keyword) {
        // Check character before the match (if any)
        let char_before = if i > 0 { line.chars().nth(i - 1) } else { None };

        // Check character after the match (if any)
        let char_after = line.chars().nth(i + keyword.len());

        // The keyword should be preceded by non-identifier char or start of string
        let valid_before = match char_before {
            None => true,       // Start of line
            Some('(') => true,  // Declaration like (func
            Some(' ') => true,  // Instruction
            Some('\t') => true, // Tab
            Some(_) => false,   // Part of identifier like $my_array
        };

        // The keyword should be followed by non-identifier char
        let valid_after = match char_after {
            None => true,                          // End of line
            Some(' ') => true,                     // Keyword followed by space
            Some('.') => true,                     // Instruction like local.get
            Some('_') if keyword == "br" => true,  // br_if, br_table
            Some(')') => true,                     // End of s-expr
            Some('$') => true,                     // Keyword followed by identifier
            Some(c) if c.is_ascii_digit() => true, // Like br0, call0
            Some(_) => false,                      // Part of identifier
        };

        if valid_before && valid_after {
            return true;
        }
    }
    false
}

/// Determine context from line text (fallback for incomplete code).
/// Used when AST-based detection returns General.
pub fn determine_context_from_line(line: &str) -> InstructionContext {
    if line_contains_keyword(line, "call") {
        InstructionContext::Call
    } else if line_contains_keyword(line, "global") {
        InstructionContext::Global
    } else if line_contains_keyword(line, "local") {
        InstructionContext::Local
    } else if line_contains_keyword(line, "br") {
        InstructionContext::Branch
    } else if line_contains_keyword(line, "block") || line_contains_keyword(line, "loop") {
        InstructionContext::Block
    // Check data/elem before general table/memory
    } else if line.contains("memory.init")
        || line.contains("data.drop")
        || line_contains_keyword(line, "data")
    {
        InstructionContext::Data
    } else if line.contains("table.init")
        || line.contains("elem.drop")
        || line_contains_keyword(line, "elem")
    {
        InstructionContext::Elem
    } else if line_contains_keyword(line, "table") {
        InstructionContext::Table
    } else if line_contains_keyword(line, "memory") {
        InstructionContext::Memory
    } else if line_contains_keyword(line, "type")
        || line_contains_keyword(line, "struct")
        || line_contains_keyword(line, "array")
        || line.contains("ref.")
    {
        InstructionContext::Type
    } else if line_contains_keyword(line, "throw")
        || line_contains_keyword(line, "tag")
        || line_contains_keyword(line, "catch")
    {
        InstructionContext::Tag
    } else if line_contains_keyword(line, "func") {
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

/// Find the AST node at the given position (native version)
#[cfg(feature = "native")]
pub fn node_at_position<'a>(tree: &'a Tree, source: &str, position: Position) -> Option<Node<'a>> {
    let byte_offset = position_to_byte(source, position);
    let root = tree.root_node();

    find_deepest_node(root, byte_offset)
}

/// Find the AST node at the given position (WASM version)
#[cfg(all(feature = "wasm", not(feature = "native")))]
pub fn node_at_position(tree: &Tree, source: &str, position: Position) -> Option<Node> {
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

/// Recursively find the deepest (most specific) node containing the byte offset (native version)
#[cfg(feature = "native")]
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

/// Recursively find the deepest (most specific) node containing the byte offset (WASM version)
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_deepest_node(node: Node, byte_offset: usize) -> Option<Node> {
    let range = node.byte_range();
    if !(range.start <= byte_offset && byte_offset <= range.end) {
        return None;
    }

    let mut cursor = node.walk();
    let mut best_child: Option<Node> = None;
    for child in node.children(&mut cursor) {
        let child_range = child.byte_range();
        let contains_or_adjacent =
            child_range.start <= byte_offset && byte_offset <= child_range.end;
        if contains_or_adjacent {
            if let Some(found) = find_deepest_node(child, byte_offset) {
                if byte_offset < child_range.end {
                    // If offset is properly inside, return immediately
                    return Some(found);
                } else if best_child.is_none() {
                    best_child = Some(found);
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
#[cfg(feature = "native")]
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
