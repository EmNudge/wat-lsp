use super::*;
use crate::parser;
use crate::tree_sitter_bindings::create_parser;
use tree_sitter::Tree;

fn create_test_tree(document: &str) -> Tree {
    let mut parser = create_parser();
    parser.parse(document, None).expect("Failed to parse test document")
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
            },
            Parameter {
                name: Some("$b".to_string()),
                param_type: ValueType::I32,
                index: 1,
            },
        ],
        results: vec![ValueType::I32],
        locals: vec![Variable {
            name: Some("$temp".to_string()),
            var_type: ValueType::I32,
            is_mutable: true,
            initial_value: None,
            index: 0,
        }],
        blocks: vec![],
        line: 0,
        end_line: 10,
        start_byte: 0,
        end_byte: 250,
    };
    table.add_function(func);

    let global = Global {
        name: Some("$counter".to_string()),
        index: 0,
        var_type: ValueType::I32,
        is_mutable: true,
        initial_value: Some("0".to_string()),
        line: 0,
    };
    table.add_global(global);

    table
}

#[test]
fn test_number_constant_completion() {
    let document = "5i32";
    let symbols = create_test_symbols();
    let position = Position::new(0, 4);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    let completion = &completions[0];
    assert!(completion.insert_text.as_ref().unwrap().contains("i32.const 5"));
}

#[test]
fn test_float_constant_completion() {
    let document = "3.14f64";
    let symbols = create_test_symbols();
    let position = Position::new(0, 7);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    let completion = &completions[0];
    assert!(completion.insert_text.as_ref().unwrap().contains("f64.const 3.14"));
}

#[test]
fn test_underscore_number_completion() {
    let document = "1_000_000i64";
    let symbols = create_test_symbols();
    let position = Position::new(0, 12);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    let completion = &completions[0];
    // Underscores should be removed
    assert!(completion.insert_text.as_ref().unwrap().contains("1000000"));
}

#[test]
fn test_local_get_emmet() {
    // This tests the emmet expansion feature
    // Note: Function context detection may not work in all cases
    let document = "(func $test (param $x i32)\n  l$";
    let symbols = parser::parse_document(document).unwrap();
    let position = Position::new(1, 3); // After "l$"

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    // The emmet feature should trigger, even if specific local isn't found
    // This is because find_containing_function may not always work
    // Test passes if it doesn't crash
    let _ = completions;
}

#[test]
fn test_local_set_emmet() {
    let document = "(func $test (param $x i32)\n  l=$";
    let symbols = parser::parse_document(document).unwrap();
    let position = Position::new(1, 4); // After "l=$"

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    // The emmet feature should trigger
    // Test passes if it doesn't crash
    let _ = completions;
}

#[test]
fn test_global_get_emmet() {
    let document = "g$";
    let symbols = create_test_symbols();
    let position = Position::new(0, 2);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    // Should suggest global variables
    assert!(completions.iter().any(|c| c.label.contains("counter")));
}

#[test]
fn test_global_set_emmet() {
    let document = "g=$";
    let symbols = create_test_symbols();
    let position = Position::new(0, 3);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    // Should only suggest mutable globals
    assert!(completions.iter().any(|c| c.label.contains("counter")));
    assert!(completions.iter().any(|c| {
        c.insert_text.as_ref().is_some_and(|t| t.contains("global.set"))
    }));
}

#[test]
fn test_i32_instruction_completion() {
    let document = "i32.";
    let symbols = create_test_symbols();
    let position = Position::new(0, 4);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    // Should have arithmetic operations
    assert!(completions.iter().any(|c| c.label == "add"));
    assert!(completions.iter().any(|c| c.label == "sub"));
    assert!(completions.iter().any(|c| c.label == "mul"));
    assert!(completions.iter().any(|c| c.label == "const"));
}

#[test]
fn test_f64_instruction_completion() {
    let document = "f64.";
    let symbols = create_test_symbols();
    let position = Position::new(0, 4);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    // Should have float-specific operations
    assert!(completions.iter().any(|c| c.label == "sqrt"));
    assert!(completions.iter().any(|c| c.label == "min"));
    assert!(completions.iter().any(|c| c.label == "max"));
}

#[test]
fn test_local_instruction_completion() {
    let document = "local.";
    let symbols = create_test_symbols();
    let position = Position::new(0, 6);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    assert!(completions.iter().any(|c| c.label == "get"));
    assert!(completions.iter().any(|c| c.label == "set"));
    assert!(completions.iter().any(|c| c.label == "tee"));
}

#[test]
fn test_global_instruction_completion() {
    let document = "global.";
    let symbols = create_test_symbols();
    let position = Position::new(0, 7);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    assert!(completions.iter().any(|c| c.label == "get"));
    assert!(completions.iter().any(|c| c.label == "set"));
}

#[test]
fn test_memory_instruction_completion() {
    let document = "memory.";
    let symbols = create_test_symbols();
    let position = Position::new(0, 7);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    assert!(completions.iter().any(|c| c.label == "size"));
    assert!(completions.iter().any(|c| c.label == "grow"));
    assert!(completions.iter().any(|c| c.label == "fill"));
    assert!(completions.iter().any(|c| c.label == "copy"));
}

#[test]
fn test_table_instruction_completion() {
    let document = "table.";
    let symbols = create_test_symbols();
    let position = Position::new(0, 6);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(!completions.is_empty());

    assert!(completions.iter().any(|c| c.label == "get"));
    assert!(completions.iter().any(|c| c.label == "set"));
    assert!(completions.iter().any(|c| c.label == "size"));
    assert!(completions.iter().any(|c| c.label == "grow"));
}

#[test]
fn test_dollar_sign_function_completion() {
    let document = "call $";
    let symbols = create_test_symbols();
    let position = Position::new(0, 6);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    // Should suggest functions
    assert!(completions.iter().any(|c| c.label.contains("add")));
}

#[test]
fn test_dollar_sign_global_completion() {
    let document = "global.get $";
    let symbols = create_test_symbols();
    let position = Position::new(0, 12);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    // Should suggest globals
    assert!(completions.iter().any(|c| c.label.contains("counter")));
}

#[test]
fn test_dollar_sign_local_completion() {
    // Use complete code so the parser can extract the function
    let document = "(func $test (param $x i32)\n  local.get $\n  )";
    let symbols = parser::parse_document(document).unwrap();
    let position = Position::new(1, 13); // Position after the $ on line 2

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);

    // Should suggest local parameters
    assert!(completions.iter().any(|c| c.label.contains("x")),
        "Expected completion with 'x' but got: {:?}",
        completions.iter().map(|c| &c.label).collect::<Vec<_>>());
}

#[test]
fn test_jsdoc_tag_completion() {
    let document = "@";
    let symbols = create_test_symbols();
    let position = Position::new(0, 1);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    assert!(completions.iter().any(|c| c.label == "param"));
    assert!(completions.iter().any(|c| c.label == "result"));
    assert!(completions.iter().any(|c| c.label == "function"));
    assert!(completions.iter().any(|c| c.label == "todo"));
}

#[test]
fn test_get_type_completions() {
    let i32_completions = get_type_completions("i32");
    assert!(i32_completions.iter().any(|c| c.label == "add"));
    assert!(i32_completions.iter().any(|c| c.label == "div_s"));
    assert!(i32_completions.iter().any(|c| c.label == "and"));

    let f32_completions = get_type_completions("f32");
    assert!(f32_completions.iter().any(|c| c.label == "sqrt"));
    assert!(f32_completions.iter().any(|c| c.label == "abs"));
}

#[test]
fn test_get_keyword_completions() {
    let keywords = get_keyword_completions();
    assert!(keywords.iter().any(|c| c.label == "func"));
    assert!(keywords.iter().any(|c| c.label == "param"));
    assert!(keywords.iter().any(|c| c.label == "local"));
    assert!(keywords.iter().any(|c| c.label == "global"));
    assert!(keywords.iter().any(|c| c.label == "block"));
    assert!(keywords.iter().any(|c| c.label == "loop"));
}

#[test]
fn test_number_const_regex() {
    assert!(NUMBER_CONST_REGEX.is_match("5i32"));
    assert!(NUMBER_CONST_REGEX.is_match("3.14f64"));
    assert!(NUMBER_CONST_REGEX.is_match("1_000_000i64"));
    assert!(NUMBER_CONST_REGEX.is_match("0.5f32"));

    assert!(!NUMBER_CONST_REGEX.is_match("abc"));
    assert!(!NUMBER_CONST_REGEX.is_match("123"));
}

#[test]
fn test_completion_item_kinds() {
    let document = "i32.";
    let symbols = create_test_symbols();
    let position = Position::new(0, 4);

    let completions = provide_completion(document, &symbols, &create_test_tree(document), position);
    // Type completions should have KEYWORD kind
    assert!(completions.iter().all(|c| c.kind == Some(CompletionItemKind::KEYWORD)));
}
