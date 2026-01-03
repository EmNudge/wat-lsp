use super::*;
use crate::parser::parse_document;
use crate::tree_sitter_bindings;

#[test]
fn test_function_references_named() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    i32.const 5
    i32.const 3
    call $add
    drop

    i32.const 10
    i32.const 20
    call $add
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "call $add" at line 9
    let position = Position {
        line: 9,
        character: 10,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references (line 9 and line 14)
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 9);
    assert_eq!(refs[1].range.start.line, 14);
}

#[test]
fn test_function_references_indexed() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    i32.const 5
    i32.const 3
    call 0
    drop

    i32.const 10
    i32.const 20
    call 0
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "call 0" at line 9
    let position = Position {
        line: 9,
        character: 10,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 9);
    assert_eq!(refs[1].range.start.line, 14);
}

#[test]
fn test_function_references_mixed() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    i32.const 5
    i32.const 3
    call $add
    drop

    i32.const 10
    i32.const 20
    call 0
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "call $add" at line 9
    let position = Position {
        line: 9,
        character: 10,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find both named and indexed references
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 9);
    assert_eq!(refs[1].range.start.line, 14);
}

#[test]
fn test_global_references_named() {
    let source = r#"(module
  (global $counter (mut i32) (i32.const 0))

  (func $increment
    global.get $counter
    i32.const 1
    i32.add
    global.set $counter)

  (func $get_count (result i32)
    global.get $counter)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "global.get $counter" at line 4
    let position = Position {
        line: 4,
        character: 16,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 3 references (lines 4, 7, 10)
    assert_eq!(refs.len(), 3);
}

#[test]
fn test_local_references_scoped() {
    let source = r#"(module
  (func $test1 (local $x i32)
    i32.const 5
    local.set $x
    local.get $x
    drop)

  (func $test2 (local $x i32)
    i32.const 10
    local.set $x
    local.get $x
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "local.set $x" in first function (line 3)
    let position = Position {
        line: 3,
        character: 15,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find only 2 references in first function (lines 3 and 4), not the ones in second function
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 3);
    assert_eq!(refs[1].range.start.line, 4);
}

#[test]
fn test_parameter_references() {
    let source = r#"(module
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "local.get $a" at line 2
    let position = Position {
        line: 2,
        character: 15,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference to $a (not counting definition)
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 2);
}

#[test]
fn test_parameter_references_from_definition() {
    let source = r#"(module
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $a
    local.get $b
    i32.add)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "$a" in the parameter definition (line 1, in "param $a")
    let position = Position {
        line: 1,
        character: 20,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references to $a (lines 2 and 3)
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 2);
    assert_eq!(refs[1].range.start.line, 3);
}

#[test]
fn test_local_references_from_definition() {
    let source = r#"(module
  (func $test (local $result i32)
    i32.const 0
    local.set $result
    local.get $result
    local.get $result
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "$result" in the local definition (line 1, in "local $result")
    let position = Position {
        line: 1,
        character: 24,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 3 references to $result (lines 3, 4, 5)
    assert_eq!(refs.len(), 3);
    assert_eq!(refs[0].range.start.line, 3);
    assert_eq!(refs[1].range.start.line, 4);
    assert_eq!(refs[2].range.start.line, 5);
}

#[test]
fn test_block_label_references_from_definition() {
    let source = r#"(module
  (func $test
    (block $break
      i32.const 1
      br_if $break
      br $break))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "$break" in the block definition (line 2, in "block $break")
    let position = Position {
        line: 2,
        character: 12,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references to $break (lines 4 and 5)
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 4);
    assert_eq!(refs[1].range.start.line, 5);
}

#[test]
fn test_loop_label_references_from_definition() {
    let source = r#"(module
  (func $test
    (loop $continue
      i32.const 1
      br_if $continue
      br $continue))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "$continue" in the loop definition (line 2)
    let position = Position {
        line: 2,
        character: 11,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references to $continue
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 4);
    assert_eq!(refs[1].range.start.line, 5);
}

#[test]
fn test_parameter_references_by_index() {
    let source = r#"(module
  (func $add (param i32) (param i32) (result i32)
    local.get 0
    local.get 1
    i32.add)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "local.get 0" at line 2
    let position = Position {
        line: 2,
        character: 15,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference to parameter 0
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 2);
}

#[test]
fn test_block_label_references_named() {
    let source = r#"(module
  (func $test
    (block $exit
      (loop $continue
        i32.const 1
        br_if $exit
        br $continue)
      i32.const 0
      br_if $exit))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "br_if $exit" at line 5
    let position = Position {
        line: 5,
        character: 15,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references to $exit (lines 5 and 8)
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 5);
    assert_eq!(refs[1].range.start.line, 8);
}

#[test]
fn test_block_label_depth_0() {
    let source = r#"(module
  (func $test
    (block $outer
      (block $inner
        br 0)))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "br 0" at line 4 - should reference $inner (innermost block)
    let position = Position {
        line: 4,
        character: 12,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference (the br 0 itself)
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 4);
}

#[test]
fn test_block_label_depth_1() {
    let source = r#"(module
  (func $test
    (block $outer
      (block $inner
        br 1
        br 1)))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on first "br 1" at line 4 - should reference $outer
    let position = Position {
        line: 4,
        character: 12,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references (both br 1 instructions)
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].range.start.line, 4);
    assert_eq!(refs[1].range.start.line, 5);
}

#[test]
fn test_include_declaration_true() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    call $add
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "call $add" at line 7
    let position = Position {
        line: 7,
        character: 10,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", true);

    // Should find 2 locations: 1 definition + 1 reference
    assert_eq!(refs.len(), 2);
    // First should be the definition (line 1)
    assert_eq!(refs[0].range.start.line, 1);
    // Second should be the reference (line 7)
    assert_eq!(refs[1].range.start.line, 7);
}

#[test]
fn test_include_declaration_false() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    call $add
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "call $add" at line 7
    let position = Position {
        line: 7,
        character: 10,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find only 1 reference (not including definition)
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 7);
}

#[test]
fn test_cursor_on_definition() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    call $add
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on the function definition "$add" at line 1
    let position = Position {
        line: 1,
        character: 9,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should still find the reference at line 7
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 7);
}

#[test]
fn test_no_references() {
    let source = r#"(module
  (func $unused (result i32)
    i32.const 42)

  (func $main
    i32.const 0
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "$unused" at line 1
    let position = Position {
        line: 1,
        character: 9,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find no references
    assert_eq!(refs.len(), 0);
}

#[test]
fn test_multiple_references_same_line() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    call $add
    call $add
    i32.add
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on first "call $add" at line 7
    let position = Position {
        line: 7,
        character: 10,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references on line 7 and line 8
    assert_eq!(refs.len(), 2);
}

#[test]
fn test_type_references() {
    let source = r#"(module
  (type $add_type (func (param i32 i32) (result i32)))

  (func $add (type $add_type)
    local.get 0
    local.get 1
    i32.add)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "(type $add_type)" at line 3
    let position = Position {
        line: 3,
        character: 20,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 3);
}

#[test]
fn test_nested_blocks_depth() {
    let source = r#"(module
  (func $test
    (block $level0
      (block $level1
        (block $level2
          br 0
          br 1
          br 2))))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "br 2" - should reference $level0 (outermost)
    let position = Position {
        line: 7,
        character: 13,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference (the br 2 itself)
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 7);
}

#[test]
fn test_local_vs_parameter_indexing() {
    let source = r#"(module
  (func $test (param i32) (param i32) (local i32)
    local.get 0
    local.get 1
    local.get 2
    drop
    drop
    drop)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "local.get 2" - this is the local, not a parameter
    let position = Position {
        line: 4,
        character: 15,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference (only the local at index 2)
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 4);
}

#[test]
fn test_memory_extraction() {
    let source = r#"(module
  (memory $mem 1)
)"#;

    let symbols = parse_document(source).unwrap();

    // Check that memory was extracted
    assert_eq!(symbols.memories.len(), 1);
    assert_eq!(symbols.memories[0].name, Some("$mem".to_string()));
    assert_eq!(symbols.memories[0].index, 0);
    assert_eq!(symbols.memories[0].limits.0, 1);
}

#[test]
fn test_memory_go_to_definition() {
    let source = r#"(module
  (memory $mem 1)
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // First check memory was extracted
    assert_eq!(symbols.memories.len(), 1, "Memory should be extracted");
    assert_eq!(symbols.memories[0].name, Some("$mem".to_string()));

    // Position on "$mem" in definition (line 1)
    let position = Position {
        line: 1,
        character: 11,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", true);

    // With include_declaration=true, should find the definition
    assert!(!refs.is_empty(), "Should find at least the definition");
    assert!(
        refs.iter().any(|r| r.range.start.line == 1),
        "Should find definition on line 1"
    );
}

#[test]
fn test_export_references() {
    let source = r#"(module
  (func $log (param i32))

  (func $main
    i32.const 42
    call $log)

  (export "log" (func $log))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "$log" in the function definition (line 1)
    let position = Position {
        line: 1,
        character: 9,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references: call $log (line 5) and export (line 7)
    assert_eq!(refs.len(), 2, "Expected 2 references (call and export)");
    assert!(
        refs.iter().any(|r| r.range.start.line == 5),
        "Should find call reference on line 5"
    );
    assert!(
        refs.iter().any(|r| r.range.start.line == 7),
        "Should find export reference on line 7"
    );
}

#[test]
fn test_export_references_indexed() {
    let source = r#"(module
  (func $log (param i32))

  (func $main
    i32.const 42
    call 0)

  (export "log" (func 0))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "0" in call 0 (line 5)
    let position = Position {
        line: 5,
        character: 10,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references: call 0 (line 5) and export func 0 (line 7)
    assert_eq!(refs.len(), 2, "Expected 2 references (call and export)");
    assert!(
        refs.iter().any(|r| r.range.start.line == 5),
        "Should find call reference on line 5"
    );
    assert!(
        refs.iter().any(|r| r.range.start.line == 7),
        "Should find export reference on line 7"
    );
}

#[test]
fn test_constants_not_function_references() {
    let source = r#"(module
  (func $log (param i32))

  (func $main
    (call $log (i32.const 1))
    (call $log (i32.const 42)))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "$log" in the function definition (line 1)
    let position = Position {
        line: 1,
        character: 9,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find exactly 2 references (the two calls to $log)
    // Should NOT find the "1" or "42" constants as references
    assert_eq!(
        refs.len(),
        2,
        "Expected exactly 2 references (calls only, not constants)"
    );
    assert!(
        refs.iter().any(|r| r.range.start.line == 4),
        "Should find call reference on line 4"
    );
    assert!(
        refs.iter().any(|r| r.range.start.line == 5),
        "Should find call reference on line 5"
    );
}

#[test]
fn test_constants_not_indexed_function_references() {
    let source = r#"(module
  (func $first (param i32))
  (func $second (param i32))

  (func $main
    (call 1 (i32.const 1))
    (call 1 (i32.const 0)))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Position on "1" in call 1 (line 5) - should reference $second (index 1)
    let position = Position {
        line: 5,
        character: 11,
    };

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find exactly 2 references (the two "call 1" instructions)
    // Should NOT find the "i32.const 1" or "i32.const 0" as references
    assert_eq!(
        refs.len(),
        2,
        "Expected exactly 2 references (calls only, not constants)"
    );

    // Check that we're finding the right positions (the "1" in "call 1")
    for r in &refs {
        assert!(
            r.range.start.line == 5 || r.range.start.line == 6,
            "References should be on lines 5 and 6"
        );
    }
}

#[test]
fn test_export_only_function_reference() {
    // Simplified test - function with just one local and export
    let source = r#"(module
  (func $edit_memory
    (local $index i32)
  )
  (export "editMemory" (func $edit_memory))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // First verify the function is in the symbol table
    assert!(
        symbols.get_function_by_name("$edit_memory").is_some(),
        "Function $edit_memory should be in symbol table"
    );

    // Position on "$edit_memory" in the function definition (line 1)
    let position = Position {
        line: 1,
        character: 8, // The $ in $edit_memory
    };

    // First check that identify_symbol_at_position works
    let target = identify_symbol_at_position(source, &symbols, &tree, position);
    assert!(
        target.is_some(),
        "Should identify symbol at position, got None"
    );

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference: export (line 4)
    assert_eq!(
        refs.len(),
        1,
        "Expected 1 reference (export), got {:?}",
        refs
    );
}

#[test]
fn test_011_host_edit_memory() {
    // Exact content from exercises/011_host.wat
    let source = r#"(;
  This exercise just exists to get better at interop between the host environment and wasm.

  There is nothing you must edit here, but feel free to play around!

  You will find more detailed comments in the associated JS file.
;)

(module
  (import "env" "memory" (memory 1))
  (import "env" "log" (func $log (param i32)))

  (func $square_num (param i32) (result i32)
    (i32.mul (local.get 0) (local.get 0))
  )

  (func $log_some_numbers
    (call $log (i32.const 1))
    (call $log (i32.const 42))
    (call $log (i32.const 88))
  )

  (func $edit_memory
    (local $index i32)
    (local $end i32)

    (local.set $index (i32.const 0))
    (local.set $end (i32.const 500))

    (loop $loop_name
      (i32.store8
        (local.get $index)
        (i32.mul
          (i32.load8_u (local.get $index))
          (i32.const 2)
        )
      )

      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (i32.lt_u (local.get $index) (local.get $end))
      (br_if $loop_name)
    )
  )

  (export "squareNum" (func $square_num))
  (export "logSomeNumbers" (func $log_some_numbers))
  (export "editMemory" (func $edit_memory))
)"#;

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Check all functions are in symbol table
    assert!(
        symbols.get_function_by_name("$log").is_some(),
        "$log should be in symbol table"
    );
    assert!(
        symbols.get_function_by_name("$square_num").is_some(),
        "$square_num should be in symbol table"
    );
    assert!(
        symbols.get_function_by_name("$log_some_numbers").is_some(),
        "$log_some_numbers should be in symbol table"
    );
    assert!(
        symbols.get_function_by_name("$edit_memory").is_some(),
        "$edit_memory should be in symbol table"
    );

    // Test $edit_memory (line 22 in 0-indexed, character 8 for the $)
    let position = Position {
        line: 22,
        character: 8,
    };

    // Check identify_symbol_at_position
    let target = identify_symbol_at_position(source, &symbols, &tree, position);
    assert!(
        target.is_some(),
        "Should identify $edit_memory at position, got None"
    );

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 1 reference: export on line 46
    assert_eq!(
        refs.len(),
        1,
        "Expected 1 reference (export) for $edit_memory, got {:?}",
        refs
    );
}

#[test]
fn test_tag_references_in_catch_clause() {
    let source = r#"(module
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

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Verify tag is in symbol table
    assert!(
        symbols.get_tag_by_name("$div_error").is_some(),
        "Tag $div_error should be in symbol table"
    );

    // Find position of $div_error in catch clause (line 5)
    let line5 = source.lines().nth(5).unwrap();
    let col = line5
        .find("$div_error")
        .expect("Should find $div_error on line 5");
    let position = Position {
        line: 5,
        character: col as u32,
    };

    // Check identify_symbol_at_position works
    let target = identify_symbol_at_position(source, &symbols, &tree, position);
    assert!(
        target.is_some(),
        "Should identify tag symbol at position ({}, {}), line: '{}'",
        position.line,
        position.character,
        line5
    );

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references: catch clause (line 5) and throw (line 8)
    assert_eq!(
        refs.len(),
        2,
        "Expected 2 references for $div_error (catch and throw), got {:?}",
        refs.iter().map(|r| r.range.start.line).collect::<Vec<_>>()
    );
}

#[test]
fn test_tag_references_from_throw() {
    let source = r#"(module
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

    let symbols = parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();

    // Find position of $div_error in throw (line 8)
    let line8 = source.lines().nth(8).unwrap();
    let col = line8
        .find("$div_error")
        .expect("Should find $div_error on line 8");
    let position = Position {
        line: 8,
        character: col as u32,
    };

    // Check identify_symbol_at_position works
    let target = identify_symbol_at_position(source, &symbols, &tree, position);
    assert!(
        target.is_some(),
        "Should identify tag symbol at position ({}, {}), line: '{}'",
        position.line,
        position.character,
        line8
    );

    let refs = provide_references(source, &symbols, &tree, position, "file:///test.wat", false);

    // Should find 2 references: catch clause (line 5) and throw (line 8)
    assert_eq!(
        refs.len(),
        2,
        "Expected 2 references for $div_error (catch and throw), got {:?}",
        refs.iter().map(|r| r.range.start.line).collect::<Vec<_>>()
    );
}
