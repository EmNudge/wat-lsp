use super::*;
use crate::parser::parse_document;
use crate::tree_sitter_bindings::create_parser;
use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Tree;

fn create_test_tree(document: &str) -> Tree {
    let mut parser = create_parser();
    parser
        .parse(document, None)
        .expect("Failed to parse test document")
}

fn create_uri() -> String {
    "file:///test.wat".to_string()
}

#[test]
fn test_goto_function_definition_by_name() {
    let document = r#"(module
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add
  )
  (func $main
    i32.const 1
    i32.const 2
    call $add
    drop
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "call $add" - on the "$add" part
    let position = Position::new(9, 10);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 1 where $add is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_global_definition() {
    let document = r#"(module
  (global $counter (mut i32) (i32.const 0))
  (func $increment
    global.get $counter
    i32.const 1
    i32.add
    global.set $counter
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "global.get $counter" - on the "$counter" part
    let position = Position::new(3, 16);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 1 where $counter is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_local_definition() {
    let document = r#"(module
  (func $test (param $x i32)
    (local $temp i32)
    local.get $temp
    local.get $x
    i32.add
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "local.get $temp" - on the "$temp" part
    let position = Position::new(3, 15);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 2 where $temp is defined
    assert_eq!(location.range.start.line, 2);
}

#[test]
fn test_goto_parameter_definition() {
    let document = r#"(module
  (func $test (param $x i32) (param $y i32)
    local.get $x
    local.get $y
    i32.add
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "local.get $x" - on the "$x" part
    let position = Position::new(2, 15);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 1 where $x is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_block_label_definition() {
    let document = r#"(module
  (func $test
    (block $exit
      i32.const 1
      br $exit
    )
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "br $exit" - on the "$exit" part
    let position = Position::new(4, 10);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 2 where $exit is defined
    assert_eq!(location.range.start.line, 2);
}

#[test]
fn test_goto_type_definition() {
    let document = r#"(module
  (type $binop (func (param i32 i32) (result i32)))
  (func $add (type $binop)
    local.get 0
    local.get 1
    i32.add
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "(type $binop)" in the function - on the "$binop" part
    let position = Position::new(2, 21);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 1 where $binop is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_table_definition() {
    let document = r#"(module
  (table $funcs 10 funcref)
  (func $test
    i32.const 0
    table.get $funcs
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "table.get $funcs" - on the "$funcs" part
    let position = Position::new(4, 15);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 1 where $funcs is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_function_definition_by_index() {
    let document = r#"(module
  (func $first (param i32) (result i32)
    local.get 0
    i32.const 1
    i32.add
  )
  (func $second
    i32.const 5
    call 0
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "call 0" - on the "0" part
    // Using numeric index to reference a named function
    let position = Position::new(8, 9);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 1 where the first function ($first) is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_local_definition_by_index() {
    let document = r#"(module
  (func $test (param $x i32) (param $y i32)
    (local $temp i32)
    local.get 0
    local.get 1
    i32.add
    local.set 2
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "local.get 0" - on the "0" part (first parameter $x)
    // Using numeric index to reference a named parameter
    let position = Position::new(3, 14);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 1 where the first param ($x) is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_local_variable_by_index() {
    let document = r#"(module
  (func $test (param $x i32) (param $y i32)
    (local $result i32)
    local.get 2
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "local.get 2" - on the "2" part (first local $result after 2 params)
    // Using numeric index to reference a named local
    let position = Position::new(3, 14);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.uri, Url::parse(&uri).unwrap());
    // The definition should point to line 2 where the local ($result) is defined
    assert_eq!(location.range.start.line, 2);
}

#[test]
fn test_goto_definition_not_found() {
    let document = r#"(module
  (func $test
    i32.const 42
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "i32.const" - not a symbol reference
    let position = Position::new(2, 5);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    // Should return None for instructions
    assert!(location.is_none());
}

#[test]
fn test_goto_definition_unnamed_symbol() {
    let document = r#"(module
  (func
    i32.const 42
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Try to get definition for an unnamed function
    // This should return None since unnamed symbols don't have $ references
    let position = Position::new(1, 3);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    // Unnamed symbols shouldn't have definitions accessible by name
    assert!(location.is_none());
}

#[test]
fn test_goto_definition_invalid_uri() {
    let document = r#"(module
  (func $test
    i32.const 42
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let invalid_uri = "not a valid uri";

    let position = Position::new(1, 8);

    let location = provide_definition(document, &symbols, &tree, position, invalid_uri);

    // Should return None for invalid URI
    assert!(location.is_none());
}

#[test]
fn test_goto_definition_nested_blocks() {
    let document = r#"(module
  (func $test
    (block $outer
      (block $inner
        br $inner
        br $outer
      )
    )
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "br $inner" - on the "$inner" part
    let position = Position::new(4, 12);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    // The definition should point to line 3 where $inner is defined
    assert_eq!(location.range.start.line, 3);

    // Position at "br $outer" - on the "$outer" part
    let position = Position::new(5, 12);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(location.is_some());
    let location = location.unwrap();
    // The definition should point to line 2 where $outer is defined
    assert_eq!(location.range.start.line, 2);
}

#[test]
fn test_goto_definition_on_parameter_definition() {
    let document = r#"(module
  (func $test (param $n i32)
    local.get $n
    drop)
)"#;

    let symbols = parse_document(document).unwrap();
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position on "$n" in the parameter definition (line 1)
    let position = Position::new(1, 21);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    // Should return the definition location (itself)
    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_definition_on_local_definition() {
    let document = r#"(module
  (func $test (local $result i32)
    i32.const 42
    local.set $result
    local.get $result)
)"#;

    let symbols = parse_document(document).unwrap();
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position on "$result" in the local definition (line 1)
    let position = Position::new(1, 24);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    // Should return the definition location (itself)
    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_definition_on_block_label_definition() {
    let document = r#"(module
  (func $test
    (block $break
      br $break))
)"#;

    let symbols = parse_document(document).unwrap();
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position on "$break" in the block definition (line 2)
    let position = Position::new(2, 12);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    // Should return the definition location (itself)
    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.range.start.line, 2);
}

#[test]
fn test_goto_definition_on_loop_label_definition() {
    let document = r#"(module
  (func $test
    (loop $continue
      br $continue))
)"#;

    let symbols = parse_document(document).unwrap();
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position on "$continue" in the loop definition (line 2)
    let position = Position::new(2, 11);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    // Should return the definition location (itself)
    assert!(location.is_some());
    let location = location.unwrap();
    assert_eq!(location.range.start.line, 2);
}

#[test]
fn test_goto_definition_catch_clause_tag() {
    let document = r#"(module
  (tag $div_error (param i32))

  (func $safe_div (param $a i32) (param $b i32) (result i32)
    (block $caught (result i32)
      (try_table (result i32) (catch $div_error $caught)
        (i32.div_s (local.get $a) (local.get $b))
      )
    )
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Find the exact position of $div_error in the catch clause
    let line5 = document.lines().nth(5).unwrap();
    let col = line5
        .find("$div_error")
        .expect("Should find $div_error on line 5");
    let position = Position::new(5, col as u32);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(
        location.is_some(),
        "Should find definition for tag in catch clause at position ({}, {}), line: '{}'",
        position.line,
        position.character,
        line5
    );
    let location = location.unwrap();
    // The definition should point to line 1 where $div_error tag is defined
    assert_eq!(location.range.start.line, 1);
}

#[test]
fn test_goto_definition_catch_clause_tag_full_example() {
    // Exact document from user's issue
    let document = r#"(module
  (tag $div_error (param i32))

  (func $safe_div (param $a i32) (param $b i32) (result i32)
    (block $caught (result i32)
      (try_table (result i32) (catch $div_error $caught)
        ;; throw if b is zero
        (if (i32.eqz (local.get $b))
          (then (throw $div_error (i32.const 400)))
        )
        ;; otherwise return a / b
        (i32.div_s (local.get $a) (local.get $b))
      )
    )
  )

  (export "safeDiv" (func $safe_div))
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Verify tag was parsed
    assert!(
        symbols.get_tag_by_name("$div_error").is_some(),
        "Tag $div_error should be in symbol table"
    );

    // Test go-to-definition on $div_error in catch clause (line 5)
    let line5 = document.lines().nth(5).unwrap();
    let col = line5
        .find("$div_error")
        .expect("Should find $div_error on line 5");
    let position = Position::new(5, col as u32);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(
        location.is_some(),
        "Should find definition for tag in catch clause. Position: ({}, {}), Line: '{}'",
        position.line,
        position.character,
        line5
    );
    let location = location.unwrap();
    assert_eq!(
        location.range.start.line, 1,
        "Definition should be on line 1"
    );

    // Test go-to-definition on $div_error in throw (line 8)
    let line8 = document.lines().nth(8).unwrap();
    let col = line8
        .find("$div_error")
        .expect("Should find $div_error on line 8");
    let position = Position::new(8, col as u32);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(
        location.is_some(),
        "Should find definition for tag in throw. Position: ({}, {}), Line: '{}'",
        position.line,
        position.character,
        line8
    );
    let location = location.unwrap();
    assert_eq!(
        location.range.start.line, 1,
        "Definition should be on line 1"
    );
}

#[test]
fn test_goto_definition_catch_clause_label() {
    let document = r#"(module
  (tag $div_error (param i32))

  (func $safe_div (param $a i32) (param $b i32) (result i32)
    (block $caught (result i32)
      (try_table (result i32) (catch $div_error $caught)
        (i32.div_s (local.get $a) (local.get $b))
      )
    )
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "$caught" in the catch clause (line 5, col ~51)
    let position = Position::new(5, 51);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(
        location.is_some(),
        "Should find definition for label in catch clause"
    );
    let location = location.unwrap();
    // The definition should point to line 4 where $caught block is defined
    assert_eq!(location.range.start.line, 4);
}

#[test]
fn test_goto_definition_catch_all_label() {
    let document = r#"(module
  (func $test (result i32)
    (block $caught (result i32)
      (try_table (result i32) (catch_all $caught)
        (i32.const 42)
      )
    )
  )
)"#;

    let symbols = parse_document(document).expect("Failed to parse document");
    let tree = create_test_tree(document);
    let uri = create_uri();

    // Position at "$caught" in the catch_all clause (line 3, col ~42)
    let position = Position::new(3, 42);

    let location = provide_definition(document, &symbols, &tree, position, &uri);

    assert!(
        location.is_some(),
        "Should find definition for label in catch_all clause"
    );
    let location = location.unwrap();
    // The definition should point to line 2 where $caught block is defined
    assert_eq!(location.range.start.line, 2);
}
