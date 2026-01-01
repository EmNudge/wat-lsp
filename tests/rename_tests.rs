use tower_lsp::lsp_types::*;
use wat_lsp_rust::{
    parser,
    references::{self, ReferenceTarget},
    tree_sitter_bindings,
};

fn parse_and_get_context(source: &str) -> (wat_lsp_rust::symbols::SymbolTable, tree_sitter::Tree) {
    let symbols = parser::parse_document(source).unwrap();
    let mut parser = tree_sitter_bindings::create_parser();
    let tree = parser.parse(source, None).unwrap();
    (symbols, tree)
}

#[test]
fn test_rename_function_named() {
    let source = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $main
    call $add)
)"#;

    let (symbols, tree) = parse_and_get_context(source);
    let position = Position::new(7, 10); // "call $add"

    // 1. Identify target
    let target = references::identify_symbol_at_position(source, &symbols, &tree, position)
        .expect("Symbol not found");

    // 2. Verify it is a named function
    if let ReferenceTarget::Function { name, .. } = &target {
        assert_eq!(name.as_deref(), Some("$add"));
    } else {
        panic!("Expected named function target");
    }

    // 3. Find references (simulating rename)
    let refs =
        references::provide_references(source, &symbols, &tree, position, "file:///test.wat", true);

    // Should find definition (line 1) and call (line 6)
    assert_eq!(refs.len(), 2);
    assert!(refs.iter().any(|r| r.range.start.line == 1));
    assert!(refs.iter().any(|r| r.range.start.line == 7));
}

#[test]
fn test_rename_local_named() {
    let source = r#"(module
  (func $test (local $x i32)
    i32.const 10
    local.set $x
    local.get $x
    drop)
)"#;

    let (symbols, tree) = parse_and_get_context(source);
    let position = Position::new(3, 15); // "local.set $x"

    let target = references::identify_symbol_at_position(source, &symbols, &tree, position)
        .expect("Symbol not found");

    if let ReferenceTarget::Local { name, .. } = &target {
        assert_eq!(name.as_deref(), Some("$x"));
    } else {
        panic!("Expected named local target");
    }

    let refs =
        references::provide_references(source, &symbols, &tree, position, "file:///test.wat", true);

    // Definition (line 1), set (line 3), get (line 4)
    assert_eq!(refs.len(), 3);
}

#[test]
fn test_rename_indexed_usage_of_named_local() {
    // This is the critical case: renaming "$x" but user clicked on "local.get 0" (or we are checking if "local.get 0" is found when renaming $x)
    // Wait, if I click on "local.get 0", does it identify it as named local "$x"?
    // identify_symbol_at_position should resolve the name if available.

    let source = r#"(module
  (func $test (local $x i32)
    local.get 0)
)"#;

    let (symbols, tree) = parse_and_get_context(source);
    let position = Position::new(2, 14); // "local.get 0"

    let target = references::identify_symbol_at_position(source, &symbols, &tree, position)
        .expect("Symbol not found");

    // Even though used by index, it refers to a named local, so identify should return that name?
    // Let's check logic in confirm. references.rs: identify_indexed_symbol -> ReferenceTarget::Local { name: local.name.clone(), index, ... }
    // local.name comes from symbols. If defined as (local $x i32), local.name is Some("$x").

    if let ReferenceTarget::Local { name, .. } = &target {
        assert_eq!(name.as_deref(), Some("$x"));
    } else {
        panic!(
            "Expected named local target from index usage, got {:?}",
            target
        );
    }

    let refs =
        references::provide_references(source, &symbols, &tree, position, "file:///test.wat", true);

    // Definition (line 1), get 0 (line 2)
    assert_eq!(refs.len(), 2);
}

#[test]
fn test_rename_unnamed_fails_check() {
    let source = r#"(module
  (func $test (local i32)
    local.get 0)
)"#;
    let (symbols, tree) = parse_and_get_context(source);
    let position = Position::new(2, 14); // "local.get 0"

    let target = references::identify_symbol_at_position(source, &symbols, &tree, position)
        .expect("Symbol not found");

    if let ReferenceTarget::Local { name, .. } = &target {
        assert!(name.is_none());
    } else {
        panic!("Expected unnamed local target");
    }

    // Main.rs logic would reject this.
}
