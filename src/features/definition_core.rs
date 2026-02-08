//! Shared definition lookup core — protocol-independent logic for both native and WASM.
//!
//! Returns `Option<Range>` in core types; callers convert to their output format.

use crate::core::types::{Position, Range};
use crate::symbol_lookup::{
    find_block_label_in_function, find_local_or_param_in_function, find_symbol_definition_range,
    IndexContext,
};
use crate::symbols::SymbolTable;
use crate::ts_facade::Tree;
use crate::utils::{
    determine_context_with_fallback, find_containing_function, get_word_at_position,
    InstructionContext,
};

/// Core go-to-definition returning a protocol-independent Range.
pub fn provide_definition_core(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
) -> Option<Range> {
    let word = get_word_at_position(document, position)?;

    if word.starts_with('$') {
        let result = provide_symbol_definition(&word, symbols, document, tree, position);
        if result.is_none() {
            return provide_definition_at_cursor(&word, symbols, position);
        }
        return result;
    }

    if word.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(index) = word.parse::<usize>() {
            return provide_index_definition(index, symbols, document, tree, position);
        }
    }

    None
}

/// Check if cursor is on a definition and return that location
fn provide_definition_at_cursor(
    word: &str,
    symbols: &SymbolTable,
    position: Position,
) -> Option<Range> {
    // Check top-level symbols
    if let Some(func) = symbols.get_function_by_name(word) {
        if let Some(range) = func.range.as_ref() {
            if position.line == range.start.line {
                return Some(*range);
            }
        }
    }

    if let Some(global) = symbols.get_global_by_name(word) {
        if let Some(range) = global.range.as_ref() {
            if position.line == range.start.line {
                return Some(*range);
            }
        }
    }

    // Check locals/params/blocks in containing function
    if let Some(func) = find_containing_function(symbols, position) {
        for param in &func.parameters {
            if param.name.as_ref() == Some(&word.to_string()) {
                if let Some(range) = param.range.as_ref() {
                    if position.line == range.start.line {
                        return Some(*range);
                    }
                }
            }
        }
        for local in &func.locals {
            if local.name.as_ref() == Some(&word.to_string()) {
                if let Some(range) = local.range.as_ref() {
                    if position.line == range.start.line {
                        return Some(*range);
                    }
                }
            }
        }
        for block in &func.blocks {
            if block.label == word {
                if let Some(range) = block.range.as_ref() {
                    if position.line == range.start.line {
                        return Some(*range);
                    }
                }
            }
        }
    }

    // Check tables, memories, types, tags
    let candidates: [Option<Range>; 4] = [
        symbols.get_table_by_name(word).and_then(|t| t.range),
        symbols.get_memory_by_name(word).and_then(|m| m.range),
        symbols.get_type_by_name(word).and_then(|t| t.range),
        symbols.get_tag_by_name(word).and_then(|t| t.range),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|range| position.line == range.start.line)
}

fn provide_symbol_definition(
    word: &str,
    symbols: &SymbolTable,
    document: &str,
    tree: &Tree,
    position: Position,
) -> Option<Range> {
    let context = determine_context_with_fallback(tree, document, position);

    match context {
        InstructionContext::Call | InstructionContext::Function => {
            symbols.get_function_by_name(word).and_then(|f| f.range)
        }
        InstructionContext::Global => symbols.get_global_by_name(word).and_then(|g| g.range),
        InstructionContext::Local => {
            let func = find_containing_function(symbols, position)?;
            find_local_or_param_in_function(word, func)
        }
        InstructionContext::Branch | InstructionContext::Block => {
            let func = find_containing_function(symbols, position)?;
            find_block_label_in_function(word, func)
        }
        InstructionContext::Table => symbols.get_table_by_name(word).and_then(|t| t.range),
        InstructionContext::Memory => symbols.get_memory_by_name(word).and_then(|m| m.range),
        InstructionContext::Type => symbols.get_type_by_name(word).and_then(|t| t.range),
        InstructionContext::Tag => symbols.get_tag_by_name(word).and_then(|t| t.range),
        InstructionContext::Data => symbols.get_data_by_name(word).and_then(|d| d.range),
        InstructionContext::Elem => symbols.get_elem_by_name(word).and_then(|e| e.range),
        InstructionContext::General => find_symbol_definition_range(word, symbols, position),
    }
}

fn provide_index_definition(
    index: usize,
    symbols: &SymbolTable,
    document: &str,
    tree: &Tree,
    position: Position,
) -> Option<Range> {
    let instr_context = determine_context_with_fallback(tree, document, position);
    let index_context = IndexContext::from_instruction_context(instr_context)?;
    crate::symbol_lookup::find_index_definition_range(index, symbols, index_context, position)
}
