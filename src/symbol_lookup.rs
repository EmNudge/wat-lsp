//! Shared symbol lookup utilities.
//!
//! This module provides centralized symbol lookup functions used by
//! hover, definition, references, and the WASM API.

use crate::core::types::{Position, Range};
use crate::symbols::{BlockLabel, Function, Parameter, SymbolTable, Variable};
use crate::utils::find_containing_function;

/// Find the definition range for a named symbol (starts with $).
/// Searches all global symbol types in a consistent order.
pub fn find_symbol_definition_range(
    word: &str,
    symbols: &SymbolTable,
    position: Position,
) -> Option<Range> {
    // Check functions
    if let Some(func) = symbols.get_function_by_name(word) {
        return func.range;
    }

    // Check globals
    if let Some(global) = symbols.get_global_by_name(word) {
        return global.range;
    }

    // Check locals/params within containing function
    if let Some(range) = find_local_or_param_range(word, symbols, position) {
        return Some(range);
    }

    // Check block labels within containing function
    if let Some(range) = find_block_label_range(word, symbols, position) {
        return Some(range);
    }

    // Check tables
    if let Some(table) = symbols.get_table_by_name(word) {
        return table.range;
    }

    // Check memories
    if let Some(memory) = symbols.get_memory_by_name(word) {
        return memory.range;
    }

    // Check types
    if let Some(type_def) = symbols.get_type_by_name(word) {
        return type_def.range;
    }

    // Check tags
    if let Some(tag) = symbols.get_tag_by_name(word) {
        return tag.range;
    }

    // Check data segments
    if let Some(data) = symbols.get_data_by_name(word) {
        return data.range;
    }

    // Check elem segments
    if let Some(elem) = symbols.get_elem_by_name(word) {
        return elem.range;
    }

    None
}

/// Find range for a local variable or parameter within the containing function.
pub fn find_local_or_param_range(
    word: &str,
    symbols: &SymbolTable,
    position: Position,
) -> Option<Range> {
    let func = find_containing_function(symbols, position)?;
    find_local_or_param_in_function(word, func)
}

/// Find range for a local variable or parameter within a specific function.
pub fn find_local_or_param_in_function(word: &str, func: &Function) -> Option<Range> {
    // Check parameters first
    if let Some(param) = find_param_in_function(word, func) {
        return param.range;
    }

    // Check locals
    if let Some(local) = find_local_in_function(word, func) {
        return local.range;
    }

    None
}

/// Find a parameter by name within a specific function.
/// Returns the Parameter reference for access to type info, etc.
pub fn find_param_in_function<'a>(word: &str, func: &'a Function) -> Option<&'a Parameter> {
    func.parameters
        .iter()
        .find(|param| param.name.as_deref() == Some(word))
}

/// Find a local variable by name within a specific function.
/// Returns the Variable reference for access to type info, etc.
pub fn find_local_in_function<'a>(word: &str, func: &'a Function) -> Option<&'a Variable> {
    func.locals
        .iter()
        .find(|local| local.name.as_deref() == Some(word))
}

/// Find range for a block label within the containing function.
pub fn find_block_label_range(
    word: &str,
    symbols: &SymbolTable,
    position: Position,
) -> Option<Range> {
    let func = find_containing_function(symbols, position)?;
    find_block_label_in_function(word, func)
}

/// Find range for a block label within a specific function.
pub fn find_block_label_in_function(word: &str, func: &Function) -> Option<Range> {
    find_block_in_function(word, func).and_then(|block| block.range)
}

/// Find a block label by name within a specific function.
/// Returns the BlockLabel reference for access to block type, line, etc.
pub fn find_block_in_function<'a>(word: &str, func: &'a Function) -> Option<&'a BlockLabel> {
    func.blocks.iter().find(|block| block.label == word)
}

/// Find the definition range for a numeric index based on context.
/// Returns the range of the symbol at the given index for the specified symbol type.
pub fn find_index_definition_range(
    index: usize,
    symbols: &SymbolTable,
    context: IndexContext,
    position: Position,
) -> Option<Range> {
    match context {
        IndexContext::Function => symbols.get_function_by_index(index).and_then(|f| f.range),
        IndexContext::Global => symbols.get_global_by_index(index).and_then(|g| g.range),
        IndexContext::Type => symbols.get_type_by_index(index).and_then(|t| t.range),
        IndexContext::Tag => symbols.get_tag_by_index(index).and_then(|t| t.range),
        IndexContext::Table => symbols.get_table_by_index(index).and_then(|t| t.range),
        IndexContext::Memory => symbols.get_memory_by_index(index).and_then(|m| m.range),
        IndexContext::Data => symbols.get_data_by_index(index).and_then(|d| d.range),
        IndexContext::Elem => symbols.get_elem_by_index(index).and_then(|e| e.range),
        IndexContext::Local => {
            if let Some(func) = find_containing_function(symbols, position) {
                let total_params = func.parameters.len();
                if index < total_params {
                    func.parameters.get(index).and_then(|p| p.range)
                } else {
                    func.locals.get(index - total_params).and_then(|l| l.range)
                }
            } else {
                None
            }
        }
    }
}

/// Context for numeric index lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexContext {
    Function,
    Global,
    Type,
    Tag,
    Table,
    Memory,
    Data,
    Elem,
    Local,
}

impl IndexContext {
    /// Convert from InstructionContext to IndexContext.
    /// Returns None for contexts that don't have numeric indices.
    pub fn from_instruction_context(ctx: crate::utils::InstructionContext) -> Option<IndexContext> {
        use crate::utils::InstructionContext;
        match ctx {
            InstructionContext::Call | InstructionContext::Function => Some(IndexContext::Function),
            InstructionContext::Global => Some(IndexContext::Global),
            InstructionContext::Local => Some(IndexContext::Local),
            InstructionContext::Type => Some(IndexContext::Type),
            InstructionContext::Tag => Some(IndexContext::Tag),
            InstructionContext::Table => Some(IndexContext::Table),
            InstructionContext::Memory => Some(IndexContext::Memory),
            InstructionContext::Data => Some(IndexContext::Data),
            InstructionContext::Elem => Some(IndexContext::Elem),
            InstructionContext::Branch
            | InstructionContext::Block
            | InstructionContext::General => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::*;

    fn make_test_symbols() -> SymbolTable {
        let mut symbols = SymbolTable::new();
        symbols.add_function(Function {
            name: Some("$add".to_string()),
            index: 0,
            parameters: vec![
                Parameter {
                    name: Some("$a".to_string()),
                    param_type: ValueType::I32,
                    index: 0,
                    range: Some(Range::from_coords(1, 10, 1, 12)),
                },
                Parameter {
                    name: Some("$b".to_string()),
                    param_type: ValueType::I32,
                    index: 1,
                    range: Some(Range::from_coords(1, 20, 1, 22)),
                },
            ],
            results: vec![ValueType::I32],
            locals: vec![],
            blocks: vec![],
            line: 1,
            end_line: 5,
            start_byte: 0,
            end_byte: 100,
            range: Some(Range::from_coords(1, 6, 1, 10)),
        });
        symbols.add_global(Global {
            name: Some("$counter".to_string()),
            index: 0,
            var_type: ValueType::I32,
            is_mutable: true,
            initial_value: None,
            line: 0,
            range: Some(Range::from_coords(0, 8, 0, 16)),
        });
        symbols
    }

    #[test]
    fn test_find_function_definition() {
        let symbols = make_test_symbols();
        let range = find_symbol_definition_range("$add", &symbols, Position::new(2, 0));
        assert!(range.is_some());
        assert_eq!(range.unwrap().start.line, 1);
    }

    #[test]
    fn test_find_global_definition() {
        let symbols = make_test_symbols();
        let range = find_symbol_definition_range("$counter", &symbols, Position::new(2, 0));
        assert!(range.is_some());
        assert_eq!(range.unwrap().start.line, 0);
    }

    #[test]
    fn test_find_param_definition() {
        let symbols = make_test_symbols();
        // Position inside the function
        let range = find_symbol_definition_range("$a", &symbols, Position::new(2, 0));
        assert!(range.is_some());
        assert_eq!(range.unwrap().start.character, 10);
    }
}
