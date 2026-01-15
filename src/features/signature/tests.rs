use super::*;
use crate::tree_sitter_bindings::create_parser;
use tree_sitter::Tree;

fn create_test_tree(document: &str) -> Tree {
    let mut parser = create_parser();
    parser
        .parse(document, None)
        .expect("Failed to parse test document")
}

fn create_test_symbols() -> SymbolTable {
    let mut table = SymbolTable::new();

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
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 200,
        range: None,
    };
    table.add_function(func);

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
    };
    table.add_function(multi_param_func);

    // Add a function type for call_ref testing
    let func_type = TypeDef {
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
    table.add_type(func_type);

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

#[test]
fn test_signature_help_simple_call() {
    let document = "call $add(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 10); // After opening paren

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.active_signature, Some(0));
    assert_eq!(help.active_parameter, Some(0));

    let signature = &help.signatures[0];
    assert!(signature.label.contains("$add"));
    assert!(signature.parameters.is_some());
    assert_eq!(signature.parameters.as_ref().unwrap().len(), 2);
}

#[test]
fn test_signature_help_with_first_arg() {
    let document = "call $add(5,";
    let symbols = create_test_symbols();
    let position = Position::new(0, 12); // After first comma

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    // Should be on second parameter
    assert_eq!(help.active_parameter, Some(1));
}

#[test]
fn test_signature_help_multi_param() {
    let document = "call $process(1,";
    let symbols = create_test_symbols();
    let position = Position::new(0, 16);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    assert_eq!(help.active_parameter, Some(1));

    let signature = &help.signatures[0];
    assert!(signature.label.contains("$process"));
    assert_eq!(signature.parameters.as_ref().unwrap().len(), 3);
}

#[test]
fn test_signature_help_third_param() {
    let document = "call $process(1, 2,";
    let symbols = create_test_symbols();
    let position = Position::new(0, 19);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    assert_eq!(help.active_parameter, Some(2));
}

#[test]
fn test_signature_help_nonexistent_function() {
    let document = "call $nonexistent(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 18);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    // Should return None for nonexistent function
    assert!(sig_help.is_none());
}

#[test]
fn test_signature_help_by_index() {
    let document = "call 0(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 7);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    assert_eq!(help.signatures.len(), 1);
    assert!(help.signatures[0].label.contains("$add"));
}

#[test]
fn test_find_function_call_simple() {
    let line = "call $add(";
    let info = find_function_call(line);
    assert!(info.is_some());

    let call = info.unwrap();
    assert_eq!(call.name, "$add");
    assert_eq!(call.arg_text, "");
}

#[test]
fn test_find_function_call_with_args() {
    let line = "call $add(5, 10";
    let info = find_function_call(line);
    assert!(info.is_some());

    let call = info.unwrap();
    assert_eq!(call.name, "$add");
    assert_eq!(call.arg_text, "5, 10");
}

#[test]
fn test_find_function_call_nested() {
    let line = "call $outer(call $inner(1), 2";
    let info = find_function_call(line);
    assert!(info.is_some());

    // Should find the outermost call
    let call = info.unwrap();
    assert_eq!(call.name, "$outer");
}

#[test]
fn test_find_function_call_no_call() {
    let line = "i32.add";
    let info = find_function_call(line);
    assert!(info.is_none());
}

#[test]
fn test_find_function_call_by_index() {
    let line = "call 42(";
    let info = find_function_call(line);
    assert!(info.is_some());

    let call = info.unwrap();
    assert_eq!(call.name, "42");
}

#[test]
fn test_format_function_signature_simple() {
    let func = Function {
        name: Some("$test".to_string()),
        index: 0,
        parameters: vec![],
        results: vec![],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 100,
        range: None,
    };

    let sig = format_function_signature(&func);
    assert_eq!(sig, "(func $test)");
}

#[test]
fn test_format_function_signature_with_params() {
    let func = Function {
        name: Some("$test".to_string()),
        index: 0,
        parameters: vec![Parameter {
            name: Some("$x".to_string()),
            param_type: ValueType::I32,
            index: 0,
            range: None,
        }],
        results: vec![],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 100,
        range: None,
    };

    let sig = format_function_signature(&func);
    assert!(sig.contains("$test"));
    assert!(sig.contains("$x"));
    assert!(sig.contains("i32"));
}

#[test]
fn test_format_function_signature_with_results() {
    let func = Function {
        name: Some("$test".to_string()),
        index: 0,
        parameters: vec![],
        results: vec![ValueType::I32, ValueType::I64],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 100,
        range: None,
    };

    let sig = format_function_signature(&func);
    assert!(sig.contains("result"));
    assert!(sig.contains("i32"));
    assert!(sig.contains("i64"));
}

#[test]
fn test_format_function_signature_unnamed_params() {
    let func = Function {
        name: Some("$test".to_string()),
        index: 0,
        parameters: vec![Parameter {
            name: None,
            param_type: ValueType::I32,
            index: 0,
            range: None,
        }],
        results: vec![],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 100,
        range: None,
    };

    let sig = format_function_signature(&func);
    assert!(sig.contains("param"));
    assert!(sig.contains("i32"));
    // Note: function name contains $, so we can't test for no $ at all
    // Just verify structure is correct
    assert!(sig.contains("func"));
}

#[test]
fn test_signature_help_parameter_info() {
    let document = "call $add(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 10);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    let help = sig_help.unwrap();
    let params = help.signatures[0].parameters.as_ref().unwrap();

    assert_eq!(params.len(), 2);
    // Check parameter labels
    match &params[0].label {
        ParameterLabel::Simple(label) => {
            assert!(label.contains("$a"));
            assert!(label.contains("i32"));
        }
        _ => panic!("Expected simple label"),
    }
}

// Tests for call_ref and return_call_ref

#[test]
fn test_find_function_call_call_ref() {
    let line = "call_ref $binop(";
    let info = find_function_call(line);
    assert!(info.is_some());

    let call = info.unwrap();
    assert_eq!(call.name, "$binop");
    assert_eq!(call.call_type, CallType::CallRef);
}

#[test]
fn test_find_function_call_return_call_ref() {
    let line = "return_call_ref $binop(";
    let info = find_function_call(line);
    assert!(info.is_some());

    let call = info.unwrap();
    assert_eq!(call.name, "$binop");
    assert_eq!(call.call_type, CallType::ReturnCallRef);
}

#[test]
fn test_find_function_call_call_ref_by_index() {
    let line = "call_ref 0(";
    let info = find_function_call(line);
    assert!(info.is_some());

    let call = info.unwrap();
    assert_eq!(call.name, "0");
    assert_eq!(call.call_type, CallType::CallRef);
}

#[test]
fn test_signature_help_call_ref() {
    let document = "call_ref $binop(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 16);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    assert_eq!(help.signatures.len(), 1);

    let signature = &help.signatures[0];
    assert!(signature.label.contains("call_ref"));
    assert!(signature.label.contains("$binop"));

    // Should have 2 params + 1 funcref = 3 parameters
    let params = signature.parameters.as_ref().unwrap();
    assert_eq!(params.len(), 3);

    // Last parameter should be funcref
    match &params[2].label {
        ParameterLabel::Simple(label) => {
            assert!(label.contains("funcref"));
        }
        _ => panic!("Expected simple label"),
    }
}

#[test]
fn test_signature_help_return_call_ref() {
    let document = "return_call_ref $complex_fn(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 28);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    let signature = &help.signatures[0];
    assert!(signature.label.contains("return_call_ref"));
    assert!(signature.label.contains("$complex_fn"));

    // Should have 3 params + 1 funcref = 4 parameters
    let params = signature.parameters.as_ref().unwrap();
    assert_eq!(params.len(), 4);
}

#[test]
fn test_signature_help_call_ref_nonexistent_type() {
    let document = "call_ref $nonexistent(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 22);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    // Should return None for nonexistent type
    assert!(sig_help.is_none());
}

#[test]
fn test_signature_help_call_ref_by_index() {
    let document = "call_ref 0(";
    let symbols = create_test_symbols();
    let position = Position::new(0, 11);

    let sig_help =
        provide_signature_help(document, &symbols, &create_test_tree(document), position);
    assert!(sig_help.is_some());

    let help = sig_help.unwrap();
    // Should find the $binop type (index 0)
    assert!(help.signatures[0].label.contains("call_ref"));
}

#[test]
fn test_call_type_enum() {
    // Verify the CallType enum variants work correctly
    let call_info_direct = CallInfo {
        name: "$add".to_string(),
        arg_text: String::new(),
        call_type: CallType::Direct,
    };
    assert_eq!(call_info_direct.call_type, CallType::Direct);

    let call_info_ref = CallInfo {
        name: "$binop".to_string(),
        arg_text: String::new(),
        call_type: CallType::CallRef,
    };
    assert_eq!(call_info_ref.call_type, CallType::CallRef);

    let call_info_return_ref = CallInfo {
        name: "$binop".to_string(),
        arg_text: String::new(),
        call_type: CallType::ReturnCallRef,
    };
    assert_eq!(call_info_return_ref.call_type, CallType::ReturnCallRef);
}
