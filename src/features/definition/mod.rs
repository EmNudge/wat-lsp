use crate::symbol_lookup::{
    find_block_label_in_function, find_local_or_param_in_function, find_symbol_definition_range,
    IndexContext,
};
use crate::symbols::*;
use crate::utils::{
    determine_context_with_fallback, find_containing_function, get_word_at_position,
    InstructionContext,
};
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

/// Helper to convert a core Range to an LSP Location
fn range_to_location(range: crate::core::types::Range, uri: &Url) -> Location {
    Location {
        uri: uri.clone(),
        range: range.into(),
    }
}

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
    let word = get_word_at_position(document, position.into())?;

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
    if let Some(func) = find_containing_function(symbols, position.into()) {
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

    let context = determine_context_with_fallback(tree, document, position.into());

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
            if let Some(func) = find_containing_function(symbols, position.into()) {
                if let Some(range) = find_local_or_param_in_function(word, func) {
                    return Some(range_to_location(range, &lsp_uri));
                }
            }
        }
        InstructionContext::Branch => {
            // Jump to block label definition
            if let Some(func) = find_containing_function(symbols, position.into()) {
                if let Some(range) = find_block_label_in_function(word, func) {
                    return Some(range_to_location(range, &lsp_uri));
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
            if let Some(func) = find_containing_function(symbols, position.into()) {
                if let Some(range) = find_block_label_in_function(word, func) {
                    return Some(range_to_location(range, &lsp_uri));
                }
            }
        }
        InstructionContext::General => {
            // Use shared symbol lookup for all symbol types
            if let Some(range) = find_symbol_definition_range(word, symbols, position.into()) {
                return Some(range_to_location(range, &lsp_uri));
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
    let lsp_uri = Url::parse(uri).ok()?;
    let instr_context = determine_context_with_fallback(tree, document, position.into());

    // Convert InstructionContext to IndexContext
    let index_context = IndexContext::from_instruction_context(instr_context)?;

    // Use shared lookup
    let range = crate::symbol_lookup::find_index_definition_range(
        index,
        symbols,
        index_context,
        position.into(),
    )?;

    Some(range_to_location(range, &lsp_uri))
}
