use super::*;
use crate::tree_sitter_bindings::create_parser;
use crate::utils::{get_line_at_position, is_word_char};
use tree_sitter::Tree;

fn create_test_tree(document: &str) -> Tree {
    let mut parser = create_parser();
    parser
        .parse(document, None)
        .expect("Failed to parse test document")
}

fn create_test_symbols() -> SymbolTable {
    let mut table = SymbolTable::new();

    // Add a function
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
        line: 0,
        range: None,
    };
    table.add_type(type_def);

    table
}

#[test]
fn test_hover_on_instruction() {
    let document = "i32.add";
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 3); // On "i32"

    let hover = provide_hover(document, &symbols, &tree, position);
    assert!(hover.is_some());

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(content.value.contains("Add"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_function() {
    let document = "call $add";
    let symbols = create_test_symbols();
    let position = Position::new(0, 6); // On "$add"

    let hover = provide_hover(document, &symbols, &create_test_tree(document), position);
    assert!(hover.is_some());

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(content.value.contains("func"));
                assert!(content.value.contains("$add"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_global() {
    let document = "global.get $counter";
    let symbols = create_test_symbols();
    let position = Position::new(0, 12); // On "$counter"

    let hover = provide_hover(document, &symbols, &create_test_tree(document), position);
    assert!(hover.is_some());

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(content.value.contains("global"));
                assert!(content.value.contains("$counter"));
                assert!(content.value.contains("mut"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_local_parameter() {
    let document = "(func $test (param $a i32)\n  (local.get $a))";
    let symbols = create_test_symbols();
    let position = Position::new(1, 14); // On "$a" in local.get

    let hover = provide_hover(document, &symbols, &create_test_tree(document), position);
    assert!(hover.is_some());

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(content.value.contains("param"));
                assert!(content.value.contains("i32"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_block_label() {
    let document = "(block $exit\n  (br $exit))";
    let symbols = create_test_symbols();
    let position = Position::new(1, 7); // On "$exit" in br

    let hover = provide_hover(document, &symbols, &create_test_tree(document), position);
    assert!(hover.is_some());

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(content.value.contains("block") || content.value.contains("$exit"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_table() {
    let document = "table.get $funcs";
    let symbols = create_test_symbols();
    let position = Position::new(0, 11); // On "$funcs"

    let hover = provide_hover(document, &symbols, &create_test_tree(document), position);
    assert!(hover.is_some());

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(content.value.contains("table"));
                assert!(content.value.contains("$funcs"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_type() {
    let document = "(type $binop";
    let symbols = create_test_symbols();
    let position = Position::new(0, 7); // On "$binop"

    let hover = provide_hover(document, &symbols, &create_test_tree(document), position);
    assert!(hover.is_some());

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(content.value.contains("type"));
                assert!(content.value.contains("$binop"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_nonexistent_symbol() {
    let document = "call $nonexistent";
    let symbols = create_test_symbols();
    let position = Position::new(0, 6);

    let hover = provide_hover(document, &symbols, &create_test_tree(document), position);
    // Should return None for nonexistent symbols
    assert!(hover.is_none());
}

#[test]
fn test_get_word_at_position() {
    let document = "i32.add $var hello";

    assert_eq!(
        get_word_at_position(document, Position::new(0, 0)),
        Some("i32.add".to_string())
    );
    assert_eq!(
        get_word_at_position(document, Position::new(0, 8)),
        Some("$var".to_string())
    );
    assert_eq!(
        get_word_at_position(document, Position::new(0, 13)),
        Some("hello".to_string())
    );
}

#[test]
fn test_get_line_at_position() {
    let document = "line 0\nline 1\nline 2";

    assert_eq!(get_line_at_position(document, 0), Some("line 0"));
    assert_eq!(get_line_at_position(document, 1), Some("line 1"));
    assert_eq!(get_line_at_position(document, 2), Some("line 2"));
    assert_eq!(get_line_at_position(document, 999), None);
}

#[test]
fn test_is_word_char() {
    assert!(is_word_char('a'));
    assert!(is_word_char('Z'));
    assert!(is_word_char('0'));
    assert!(is_word_char('_'));
    assert!(is_word_char('$'));
    assert!(is_word_char('.'));
    assert!(is_word_char('-'));
    assert!(!is_word_char(' '));
    assert!(!is_word_char('('));
    assert!(!is_word_char(')'));
}

#[test]
fn test_instruction_docs_available() {
    // Test that some basic instruction docs are available
    assert!(INSTRUCTION_DOCS.contains_key("i32.add"));
    assert!(INSTRUCTION_DOCS.contains_key("f32.mul")); // Changed from f64.mul
    assert!(INSTRUCTION_DOCS.contains_key("local.get"));
    assert!(INSTRUCTION_DOCS.contains_key("block"));
    assert!(INSTRUCTION_DOCS.contains_key("call"));
}

#[test]
fn test_format_function_signature() {
    let func = Function {
        name: Some("$test".to_string()),
        index: 0,
        parameters: vec![
            Parameter {
                name: Some("$x".to_string()),
                param_type: ValueType::I32,
                index: 0,
                range: None,
            },
            Parameter {
                name: None,
                param_type: ValueType::I64,
                index: 1,
                range: None,
            },
        ],
        results: vec![ValueType::F32],
        locals: vec![],
        blocks: vec![],
        line: 0,
        end_line: 5,
        start_byte: 0,
        end_byte: 150,
        range: None,
    };

    let sig = format_function_signature(&func);
    assert!(sig.contains("$test"));
    assert!(sig.contains("$x"));
    assert!(sig.contains("i32"));
    assert!(sig.contains("i64"));
    assert!(sig.contains("f32"));
}
