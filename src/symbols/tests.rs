use super::*;

#[test]
fn test_symbol_table_new() {
    let table = SymbolTable::new();
    assert_eq!(table.functions.len(), 0);
    assert_eq!(table.globals.len(), 0);
    assert_eq!(table.tables.len(), 0);
    assert_eq!(table.types.len(), 0);
}

#[test]
fn test_add_function() {
    let mut table = SymbolTable::new();

    let func = Function {
        name: Some("$test".to_string()),
        index: 0,
        parameters: vec![],
        results: vec![ValueType::I32],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 100,
    };

    table.add_function(func);

    assert_eq!(table.functions.len(), 1);
    assert!(table.function_map.contains_key("$test"));
    assert_eq!(table.get_function_by_name("$test").unwrap().name, Some("$test".to_string()));
}

#[test]
fn test_add_global() {
    let mut table = SymbolTable::new();

    let global = Global {
        name: Some("$counter".to_string()),
        index: 0,
        var_type: ValueType::I32,
        is_mutable: true,
        initial_value: Some("0".to_string()),
        line: 0,
    };

    table.add_global(global);

    assert_eq!(table.globals.len(), 1);
    assert!(table.global_map.contains_key("$counter"));

    let g = table.get_global_by_name("$counter").unwrap();
    assert!(g.is_mutable);
    assert_eq!(g.var_type, ValueType::I32);
}

#[test]
fn test_add_table() {
    let mut table = SymbolTable::new();

    let tbl = Table {
        name: Some("$funcs".to_string()),
        index: 0,
        ref_type: ValueType::Funcref,
        limits: (10, Some(100)),
        line: 0,
    };

    table.add_table(tbl);

    assert_eq!(table.tables.len(), 1);
    assert!(table.table_map.contains_key("$funcs"));

    let t = table.get_table_by_name("$funcs").unwrap();
    assert_eq!(t.ref_type, ValueType::Funcref);
    assert_eq!(t.limits.0, 10);
    assert_eq!(t.limits.1, Some(100));
}

#[test]
fn test_add_type() {
    let mut table = SymbolTable::new();

    let type_def = TypeDef {
        name: Some("$binop".to_string()),
        index: 0,
        parameters: vec![ValueType::I32, ValueType::I32],
        results: vec![ValueType::I32],
        line: 0,
    };

    table.add_type(type_def);

    assert_eq!(table.types.len(), 1);
    assert!(table.type_map.contains_key("$binop"));

    let t = table.get_type_by_name("$binop").unwrap();
    assert_eq!(t.parameters.len(), 2);
    assert_eq!(t.results.len(), 1);
}

#[test]
fn test_get_function_by_index() {
    let mut table = SymbolTable::new();

    for i in 0..3 {
        let func = Function {
            name: Some(format!("$func{}", i)),
            index: i,
            parameters: vec![],
            results: vec![],
            locals: vec![],
            blocks: vec![],
            line: i as u32,
            end_line: i as u32 + 5,
            start_byte: 0,
            end_byte: 100,
        };
        table.add_function(func);
    }

    assert!(table.get_function_by_index(0).is_some());
    assert!(table.get_function_by_index(2).is_some());
    assert!(table.get_function_by_index(999).is_none());

    let f1 = table.get_function_by_index(1).unwrap();
    assert_eq!(f1.name, Some("$func1".to_string()));
}

#[test]
fn test_get_global_by_index() {
    let mut table = SymbolTable::new();

    for i in 0..3 {
        let global = Global {
            name: Some(format!("$global{}", i)),
            index: i,
            var_type: ValueType::I32,
            is_mutable: false,
            initial_value: None,
            line: i as u32,
        };
        table.add_global(global);
    }

    assert!(table.get_global_by_index(0).is_some());
    assert!(table.get_global_by_index(999).is_none());
}

#[test]
fn test_unnamed_symbols() {
    let mut table = SymbolTable::new();

    // Add unnamed function
    let func = Function {
        name: None,
        index: 0,
        parameters: vec![],
        results: vec![],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 100,
    };
    table.add_function(func);

    // Should be accessible by index but not by name
    assert!(table.get_function_by_index(0).is_some());
    assert!(table.get_function_by_name("$unnamed").is_none());
}

#[test]
fn test_multiple_symbols_same_type() {
    let mut table = SymbolTable::new();

    // Add multiple functions
    for i in 0..5 {
        let func = Function {
            name: Some(format!("$func{}", i)),
            index: i,
            parameters: vec![],
            results: vec![],
            locals: vec![],
            blocks: vec![],
            line: 0,
            end_line: 5,
            start_byte: 0,
            end_byte: 100,
        };
        table.add_function(func);
    }

    assert_eq!(table.functions.len(), 5);
    assert_eq!(table.function_map.len(), 5);

    // Verify all are accessible
    for i in 0..5 {
        assert!(table.get_function_by_name(&format!("$func{}", i)).is_some());
        assert!(table.get_function_by_index(i).is_some());
    }
}

#[test]
fn test_parameter_creation() {
    let param = Parameter {
        name: Some("$x".to_string()),
        param_type: ValueType::I32,
        index: 0,
    };

    assert_eq!(param.name, Some("$x".to_string()));
    assert_eq!(param.param_type, ValueType::I32);
    assert_eq!(param.index, 0);
}

#[test]
fn test_variable_creation() {
    let var = Variable {
        name: Some("$temp".to_string()),
        var_type: ValueType::F64,
        is_mutable: true,
        initial_value: Some("3.14".to_string()),
        index: 0,
    };

    assert_eq!(var.name, Some("$temp".to_string()));
    assert_eq!(var.var_type, ValueType::F64);
    assert!(var.is_mutable);
}

#[test]
fn test_block_label_creation() {
    let block = BlockLabel {
        label: "$exit".to_string(),
        block_type: "block".to_string(),
        line: 42,
    };

    assert_eq!(block.label, "$exit");
    assert_eq!(block.block_type, "block");
    assert_eq!(block.line, 42);
}

#[test]
fn test_value_type_equality() {
    assert_eq!(ValueType::I32, ValueType::I32);
    assert_ne!(ValueType::I32, ValueType::I64);
    assert_eq!(ValueType::Funcref, ValueType::Funcref);
}

#[test]
fn test_complex_function() {
    let func = Function {
        name: Some("$complex".to_string()),
        index: 0,
        parameters: vec![
            Parameter {
                name: Some("$a".to_string()),
                param_type: ValueType::I32,
                index: 0,
            },
            Parameter {
                name: Some("$b".to_string()),
                param_type: ValueType::I64,
                index: 1,
            },
        ],
        results: vec![ValueType::I32, ValueType::I64],
        locals: vec![
            Variable {
                name: Some("$temp".to_string()),
                var_type: ValueType::F32,
                is_mutable: true,
                initial_value: None,
                index: 0,
            },
        ],
        blocks: vec![
            BlockLabel {
                label: "$exit".to_string(),
                block_type: "block".to_string(),
                line: 10,
            },
        ],
        line: 5,
        end_line: 20,
        start_byte: 0,
        end_byte: 500,
    };

    assert_eq!(func.parameters.len(), 2);
    assert_eq!(func.results.len(), 2);
    assert_eq!(func.locals.len(), 1);
    assert_eq!(func.blocks.len(), 1);
}
