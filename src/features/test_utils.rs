//! Shared test utilities for feature module tests.
//!
//! This module provides common helpers used across unit tests in the features modules.

#[cfg(all(test, feature = "native"))]
use crate::symbols::{
    BlockLabel, Function, Global, Parameter, SymbolTable, Table, TypeDef, TypeKind, ValueType,
    Variable,
};

#[cfg(all(test, feature = "native"))]
use crate::tree_sitter_bindings::create_parser;

#[cfg(all(test, feature = "native"))]
use tree_sitter::Tree;

/// Create a parsed tree from a document string for testing.
#[cfg(all(test, feature = "native"))]
pub fn create_test_tree(document: &str) -> Tree {
    let mut parser = create_parser();
    parser
        .parse(document, None)
        .expect("Failed to parse test document")
}

/// Create a minimal symbol table for basic tests.
/// Contains: $add function, $counter global, $funcs table, $binop type.
#[cfg(all(test, feature = "native"))]
pub fn create_test_symbols() -> SymbolTable {
    let mut table = SymbolTable::new();

    // Add a function with parameters, locals, and a block label
    let func = Function {
        name: Some("$add".to_string()),
        index: 0,
        parameters: vec![
            Parameter {
                name: Some("$a".to_string()),
                param_type: ValueType::I32,
                index: 0,
                range: None,
            },
            Parameter {
                name: Some("$b".to_string()),
                param_type: ValueType::I32,
                index: 1,
                range: None,
            },
        ],
        results: vec![ValueType::I32],
        locals: vec![Variable {
            name: Some("$temp".to_string()),
            var_type: ValueType::I32,
            is_mutable: true,
            initial_value: None,
            index: 0,
            range: None,
        }],
        blocks: vec![BlockLabel {
            label: "$exit".to_string(),
            block_type: "block".to_string(),
            line: 5,
            range: None,
        }],
        line: 0,
        end_line: 10,
        start_byte: 0,
        end_byte: 300,
        range: None,
        doc_comment: None,
    };
    table.add_function(func);

    // Add a global
    let global = Global {
        name: Some("$counter".to_string()),
        index: 0,
        var_type: ValueType::I32,
        is_mutable: true,
        initial_value: Some("0".to_string()),
        line: 0,
        range: None,
    };
    table.add_global(global);

    // Add a table
    let tbl = Table {
        name: Some("$funcs".to_string()),
        index: 0,
        ref_type: ValueType::Funcref,
        limits: (10, None),
        line: 0,
        range: None,
    };
    table.add_table(tbl);

    // Add a type
    let type_def = TypeDef {
        name: Some("$binop".to_string()),
        index: 0,
        kind: TypeKind::Func {
            params: vec![ValueType::I32, ValueType::I32],
            results: vec![ValueType::I32],
        },
        supertype: None,
        is_final: true,
        rec_group_id: None,
        line: 0,
        range: None,
    };
    table.add_type(type_def);

    table
}

/// Create a symbol table for signature tests with multiple functions and types.
#[cfg(all(test, feature = "native"))]
pub fn create_signature_test_symbols() -> SymbolTable {
    let mut table = create_test_symbols();

    // Add a multi-param function
    let multi_param_func = Function {
        name: Some("$process".to_string()),
        index: 1,
        parameters: vec![
            Parameter {
                name: Some("$x".to_string()),
                param_type: ValueType::I32,
                index: 0,
                range: None,
            },
            Parameter {
                name: Some("$y".to_string()),
                param_type: ValueType::I64,
                index: 1,
                range: None,
            },
            Parameter {
                name: Some("$z".to_string()),
                param_type: ValueType::F32,
                index: 2,
                range: None,
            },
        ],
        results: vec![ValueType::F64],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 8,
        start_byte: 0,
        end_byte: 300,
        range: None,
        doc_comment: None,
    };
    table.add_function(multi_param_func);

    // Add another function type with more parameters
    let complex_type = TypeDef {
        name: Some("$complex_fn".to_string()),
        index: 1,
        kind: TypeKind::Func {
            params: vec![ValueType::I32, ValueType::I64, ValueType::F32],
            results: vec![ValueType::F64],
        },
        supertype: None,
        is_final: true,
        rec_group_id: None,
        line: 0,
        range: None,
    };
    table.add_type(complex_type);

    table
}

/// Create a test file URI string.
#[cfg(all(test, feature = "native"))]
pub fn create_uri() -> String {
    "file:///test.wat".to_string()
}
