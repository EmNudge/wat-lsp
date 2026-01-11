use crate::symbols::*;
use crate::utils::{
    determine_context_from_line, determine_instruction_context, find_containing_function,
    get_line_at_position, get_word_at_position, node_at_position, InstructionContext,
};
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

#[cfg(test)]
mod tests;

/// Main entry point for providing go-to-definition
pub fn provide_definition(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
    uri: &str,
) -> Option<Location> {
    let word = get_word_at_position(document, position)?;

    // Check if it's a symbol reference (starts with $)
    if word.starts_with('$') {
        let result = provide_symbol_definition(&word, symbols, document, tree, position, uri);

        // If we didn't find a definition (might be because we're ON the definition),
        // try to return the definition location itself
        if result.is_none() {
            return provide_definition_at_cursor(&word, symbols, position, uri);
        }

        return result;
    }

    // Check for numeric indices
    if word.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(index) = word.parse::<usize>() {
            return provide_index_definition(index, symbols, document, tree, position, uri);
        }
    }

    None
}

/// Check if cursor is on a definition and return that location
fn provide_definition_at_cursor(
    word: &str,
    symbols: &SymbolTable,
    position: Position,
    uri: &str,
) -> Option<Location> {
    let lsp_uri = Url::parse(uri).ok()?;

    // Check if we're on a function definition
    if let Some(func) = symbols.get_function_by_name(word) {
        if let Some(range) = func.range.as_ref() {
            // Check if the cursor position is within the definition range
            if position.line == range.start.line {
                return Some(Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
    }

    // Check if we're on a global definition
    if let Some(global) = symbols.get_global_by_name(word) {
        if let Some(range) = global.range.as_ref() {
            if position.line == range.start.line {
                return Some(Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
    }

    // Check if we're on a local/parameter definition (omitted for brevity, assume existing logic covers locals)
    // Actually reusing existing function body finding logic
    if let Some(func) = find_containing_function(symbols, position) {
        // ... (locals check logic) ...
        // Re-implementing logic here for safety
        for param in &func.parameters {
            if param.name.as_ref() == Some(&word.to_string()) {
                if let Some(range) = param.range.as_ref() {
                    if position.line == range.start.line {
                        return Some(Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
            }
        }
        for local in &func.locals {
            if local.name.as_ref() == Some(&word.to_string()) {
                if let Some(range) = local.range.as_ref() {
                    if position.line == range.start.line {
                        return Some(Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
            }
        }
        for block in &func.blocks {
            if block.label == word {
                if let Some(range) = block.range.as_ref() {
                    if position.line == range.start.line {
                        return Some(Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
            }
        }
    }

    // Check tables
    if let Some(table) = symbols.get_table_by_name(word) {
        if let Some(range) = table.range.as_ref() {
            if position.line == range.start.line {
                return Some(Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
    }

    // Check memories
    if let Some(memory) = symbols.get_memory_by_name(word) {
        if let Some(range) = memory.range.as_ref() {
            if position.line == range.start.line {
                return Some(Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
    }

    // Check types
    if let Some(type_def) = symbols.get_type_by_name(word) {
        if let Some(range) = type_def.range.as_ref() {
            if position.line == range.start.line {
                return Some(Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
    }

    // Check tags
    if let Some(tag) = symbols.get_tag_by_name(word) {
        if let Some(range) = tag.range.as_ref() {
            if position.line == range.start.line {
                return Some(Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
    }

    None
}

/// Provide definition for named symbols (e.g., $funcName, $varName)
fn provide_symbol_definition(
    word: &str,
    symbols: &SymbolTable,
    document: &str,
    tree: &Tree,
    position: Position,
    uri: &str,
) -> Option<Location> {
    // Parse URI once at the beginning
    let lsp_uri = Url::parse(uri).ok()?;

    // Determine context using AST, with fallback to line matching
    let context = if let Some(node) = node_at_position(tree, document, position) {
        let ast_context = determine_instruction_context(node, document);
        if ast_context == InstructionContext::General {
            // Fallback to line-based detection for incomplete code
            if let Some(line) = get_line_at_position(document, position.line as usize) {
                determine_context_from_line(line)
            } else {
                InstructionContext::General
            }
        } else {
            ast_context
        }
    } else {
        // Fallback to line-based detection
        if let Some(line) = get_line_at_position(document, position.line as usize) {
            determine_context_from_line(line)
        } else {
            InstructionContext::General
        }
    };

    match context {
        InstructionContext::Call => {
            // Jump to function definition
            if let Some(func) = symbols.get_function_by_name(word) {
                return func.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Global => {
            // Jump to global definition
            if let Some(global) = symbols.get_global_by_name(word) {
                return global.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Local => {
            // Jump to local/parameter definition
            if let Some(func) = find_containing_function(symbols, position) {
                // Check parameters first
                for param in &func.parameters {
                    if param.name.as_ref() == Some(&word.to_string()) {
                        return param.range.as_ref().map(|range| Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
                // Then check locals
                for local in &func.locals {
                    if local.name.as_ref() == Some(&word.to_string()) {
                        return local.range.as_ref().map(|range| Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
            }
        }
        InstructionContext::Branch => {
            // Jump to block label definition
            if let Some(func) = find_containing_function(symbols, position) {
                for block in &func.blocks {
                    if block.label == word {
                        return block.range.as_ref().map(|range| Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
            }
        }
        InstructionContext::Table => {
            // Jump to table definition
            if let Some(table) = symbols.get_table_by_name(word) {
                return table.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Memory => {
            // Jump to memory definition
            if let Some(memory) = symbols.get_memory_by_name(word) {
                return memory.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Type => {
            // Jump to type definition
            if let Some(type_def) = symbols.get_type_by_name(word) {
                return type_def.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Tag => {
            // Jump to tag definition
            if let Some(tag) = symbols.get_tag_by_name(word) {
                return tag.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Function => {
            // Jump to function definition (when in function header itself)
            if let Some(func) = symbols.get_function_by_name(word) {
                return func.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Block => {
            // Block context - same as Branch for definition lookup
            if let Some(func) = find_containing_function(symbols, position) {
                for block in &func.blocks {
                    if block.label == word {
                        return block.range.as_ref().map(|range| Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
            }
        }
        InstructionContext::General => {
            // Try all symbol types
            // Try function
            if let Some(func) = symbols.get_function_by_name(word) {
                return func.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
            // Try global
            if let Some(global) = symbols.get_global_by_name(word) {
                return global.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
            // Try table
            if let Some(table) = symbols.get_table_by_name(word) {
                return table.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
            // Try memory
            if let Some(memory) = symbols.get_memory_by_name(word) {
                return memory.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
            // Try type
            if let Some(type_def) = symbols.get_type_by_name(word) {
                return type_def.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
            // Try tag
            if let Some(tag) = symbols.get_tag_by_name(word) {
                return tag.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
            // Try data segment
            if let Some(data) = symbols.get_data_by_name(word) {
                return data.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
            // Try elem segment
            if let Some(elem) = symbols.get_elem_by_name(word) {
                return elem.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Data => {
            // Jump to data segment definition
            if let Some(data) = symbols.get_data_by_name(word) {
                return data.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Elem => {
            // Jump to elem segment definition
            if let Some(elem) = symbols.get_elem_by_name(word) {
                return elem.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
    }

    None
}

/// Provide definition for numeric indices (e.g., call 0, local.get 1)
fn provide_index_definition(
    index: usize,
    symbols: &SymbolTable,
    document: &str,
    tree: &Tree,
    position: Position,
    uri: &str,
) -> Option<Location> {
    // Parse URI once at the beginning
    let lsp_uri = Url::parse(uri).ok()?;

    // Determine context using AST, with fallback to line matching
    let context = if let Some(node) = node_at_position(tree, document, position) {
        let ast_context = determine_instruction_context(node, document);
        if ast_context == InstructionContext::General {
            // Fallback to line-based detection for incomplete code
            if let Some(line) = get_line_at_position(document, position.line as usize) {
                determine_context_from_line(line)
            } else {
                InstructionContext::General
            }
        } else {
            ast_context
        }
    } else {
        // Fallback to line-based detection
        if let Some(line) = get_line_at_position(document, position.line as usize) {
            determine_context_from_line(line)
        } else {
            InstructionContext::General
        }
    };

    match context {
        InstructionContext::Call => {
            // Jump to function by index
            if let Some(func) = symbols.get_function_by_index(index) {
                return func.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Global => {
            // Jump to global by index
            if let Some(global) = symbols.get_global_by_index(index) {
                return global.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Type => {
            // Jump to type by index
            if let Some(type_def) = symbols.get_type_by_index(index) {
                return type_def.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Tag => {
            // Jump to tag by index
            if let Some(tag) = symbols.get_tag_by_index(index) {
                return tag.range.as_ref().map(|range| Location {
                    uri: lsp_uri.clone(),
                    range: (*range).into(),
                });
            }
        }
        InstructionContext::Local => {
            // Jump to local/parameter by index
            if let Some(func) = find_containing_function(symbols, position) {
                // Parameters come first, then locals
                let total_params = func.parameters.len();

                if index < total_params {
                    // It's a parameter
                    if let Some(param) = func.parameters.get(index) {
                        return param.range.as_ref().map(|range| Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                } else {
                    // It's a local
                    let local_index = index - total_params;
                    if let Some(local) = func.locals.get(local_index) {
                        return local.range.as_ref().map(|range| Location {
                            uri: lsp_uri.clone(),
                            range: (*range).into(),
                        });
                    }
                }
            }
        }
        _ => {}
    }

    None
}
