use super::*;
use crate::features::test_utils::{create_test_symbols, create_test_tree};
use crate::utils::{get_line_at_position, is_inside_comment, is_word_char};

#[test]
fn test_hover_on_instruction() {
    let document = "i32.add";
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 3); // On "i32"

    let hover = provide_hover(document, &symbols, &tree, position.into());
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

    let hover = provide_hover(
        document,
        &symbols,
        &create_test_tree(document),
        position.into(),
    );
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

    let hover = provide_hover(
        document,
        &symbols,
        &create_test_tree(document),
        position.into(),
    );
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

    let hover = provide_hover(
        document,
        &symbols,
        &create_test_tree(document),
        position.into(),
    );
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

    let hover = provide_hover(
        document,
        &symbols,
        &create_test_tree(document),
        position.into(),
    );
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

    let hover = provide_hover(
        document,
        &symbols,
        &create_test_tree(document),
        position.into(),
    );
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

    let hover = provide_hover(
        document,
        &symbols,
        &create_test_tree(document),
        position.into(),
    );
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

    let hover = provide_hover(
        document,
        &symbols,
        &create_test_tree(document),
        position.into(),
    );
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
    assert!(crate::docs::get_instruction_doc("i32.add").is_some());
    assert!(crate::docs::get_instruction_doc("f32.mul").is_some()); // Changed from f64.mul
    assert!(crate::docs::get_instruction_doc("local.get").is_some());
    assert!(crate::docs::get_instruction_doc("block").is_some());
    assert!(crate::docs::get_instruction_doc("call").is_some());
}

#[test]
fn test_new_instruction_docs_available() {
    // Test that the new WASM 3.0 instruction docs are available
    // Typed function references
    assert!(
        crate::docs::get_instruction_doc("call_ref").is_some(),
        "call_ref should be documented"
    );
    assert!(
        crate::docs::get_instruction_doc("return_call_ref").is_some(),
        "return_call_ref should be documented"
    );

    // Null-checking branches
    assert!(
        crate::docs::get_instruction_doc("br_on_null").is_some(),
        "br_on_null should be documented"
    );
    assert!(
        crate::docs::get_instruction_doc("br_on_non_null").is_some(),
        "br_on_non_null should be documented"
    );

    // Reference equality
    assert!(
        crate::docs::get_instruction_doc("ref.eq").is_some(),
        "ref.eq should be documented"
    );

    // Reference conversions
    assert!(
        crate::docs::get_instruction_doc("any.convert_extern").is_some(),
        "any.convert_extern should be documented"
    );
    assert!(
        crate::docs::get_instruction_doc("extern.convert_any").is_some(),
        "extern.convert_any should be documented"
    );

    // Array initialization
    assert!(
        crate::docs::get_instruction_doc("array.init_data").is_some(),
        "array.init_data should be documented"
    );
    assert!(
        crate::docs::get_instruction_doc("array.init_elem").is_some(),
        "array.init_elem should be documented"
    );
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
        doc_comment: None,
    };

    let sig = format_function_signature(&func);
    assert!(sig.contains("$test"));
    assert!(sig.contains("$x"));
    assert!(sig.contains("i32"));
    assert!(sig.contains("i64"));
    assert!(sig.contains("f32"));
}

#[test]
fn test_no_hover_in_block_comment() {
    // Block comment containing text that looks like an instruction
    let document = "(; i32.add ;)";
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 5); // On "i32.add" inside comment

    // Verify we're inside a comment
    assert!(is_inside_comment(&tree, document, position));

    // Hover should return None for content inside comments
    let hover = provide_hover(document, &symbols, &tree, position.into());
    assert!(hover.is_none());
}

#[test]
fn test_no_hover_in_block_comment_with_symbol() {
    // Block comment containing a symbol reference
    let document = "(; $add ;)";
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 4); // On "$add" inside comment

    // Verify we're inside a comment
    assert!(is_inside_comment(&tree, document, position));

    // Hover should return None for content inside comments
    let hover = provide_hover(document, &symbols, &tree, position.into());
    assert!(hover.is_none());
}

#[test]
fn test_no_hover_in_line_comment() {
    // Line comment containing an instruction
    let document = ";; i32.add";
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 5); // On "i32.add" inside comment

    // Verify we're inside a comment
    assert!(is_inside_comment(&tree, document, position));

    // Hover should return None for content inside comments
    let hover = provide_hover(document, &symbols, &tree, position.into());
    assert!(hover.is_none());
}

#[test]
fn test_hover_outside_comment() {
    // Verify hover still works outside comments
    let document = "(; comment ;) i32.add";
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 17); // On "i32.add" outside comment

    // Verify we're NOT inside a comment
    assert!(!is_inside_comment(&tree, document, position));

    // Hover should work for instruction outside comment
    let hover = provide_hover(document, &symbols, &tree, position.into());
    assert!(hover.is_some());
}

#[test]
fn test_hover_includes_doc_comment() {
    use crate::parser;

    // Hover over "call $add" - we need a call site
    let doc_with_call = r#"(module
  ;; Adds two 32-bit integers
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
  (func $main
    (call $add (i32.const 1) (i32.const 2)))
)"#;

    let symbols_with_call = parser::parse_document(doc_with_call).unwrap();
    let tree_with_call = create_test_tree(doc_with_call);

    // Position on "$add" in the call instruction (line 5, around column 10)
    let position = Position::new(5, 10);

    let hover = provide_hover(
        doc_with_call,
        &symbols_with_call,
        &tree_with_call,
        position.into(),
    );
    assert!(hover.is_some(), "Expected hover for function call");

    let content = match hover.unwrap().contents {
        tower_lsp::lsp_types::HoverContents::Markup(m) => m.value,
        _ => panic!("Expected Markup content"),
    };

    // Verify the hover contains the doc comment
    assert!(
        content.contains("Adds two 32-bit integers"),
        "Hover should include doc comment. Got: {}",
        content
    );

    // Also verify the function signature is present
    assert!(
        content.contains("func $add"),
        "Hover should include function signature"
    );
}

#[test]
fn test_hover_on_annotation_name() {
    let document = r#"(module (@name "test_module"))"#;
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 11); // On "name"

    let hover = provide_hover(document, &symbols, &tree, position.into());
    assert!(hover.is_some(), "Expected hover on annotation name");

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(
                    content.value.contains("@name"),
                    "Hover should contain annotation name"
                );
                assert!(
                    content.value.contains("annotation"),
                    "Hover should indicate it's an annotation"
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_annotation_producers() {
    let document = r#"(module (@producers (language "Rust")))"#;
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 12); // On "producers"

    let hover = provide_hover(document, &symbols, &tree, position.into());
    assert!(hover.is_some(), "Expected hover on producers annotation");

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(
                    content.value.contains("@producers"),
                    "Hover should contain annotation name"
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }
}

#[test]
fn test_hover_on_custom_annotation() {
    // Test an unknown annotation that should show "custom annotation" fallback
    let document = r#"(module (@unknown_annotation "value"))"#;
    let symbols = create_test_symbols();
    let tree = create_test_tree(document);
    let position = Position::new(0, 15); // On "unknown_annotation"

    let hover = provide_hover(document, &symbols, &tree, position.into());
    assert!(hover.is_some(), "Expected hover on custom annotation");

    if let Some(h) = hover {
        match h.contents {
            HoverContents::Markup(content) => {
                assert!(
                    content.value.contains("custom annotation"),
                    "Hover should indicate it's a custom annotation"
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }
}
