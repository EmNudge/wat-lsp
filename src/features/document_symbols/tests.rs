use super::*;
use crate::parser::parse_document;
use tower_lsp::lsp_types::SymbolKind;

#[test]
fn test_empty_document() {
    let symbols = SymbolTable::new();
    let result = provide_document_symbols(&symbols);
    assert!(result.is_empty());
}

#[test]
fn test_function_symbols() {
    let source = r#"
(module
  (func $add (param $a i32) (param $b i32) (result i32)
    (local $tmp i32)
    (i32.add (local.get $a) (local.get $b))
  )
)
"#;

    let symbols = parse_document(source).unwrap();
    let result = provide_document_symbols(&symbols);

    assert_eq!(result.len(), 1);
    let func_sym = &result[0];
    assert_eq!(func_sym.name, "$add");
    assert_eq!(func_sym.kind, SymbolKind::FUNCTION);

    // Check children (params and locals)
    let children = func_sym.children.as_ref().unwrap();
    assert_eq!(children.len(), 3); // 2 params + 1 local

    assert_eq!(children[0].name, "$a");
    assert_eq!(children[0].kind, SymbolKind::VARIABLE);
    assert_eq!(children[0].detail.as_deref(), Some("i32"));

    assert_eq!(children[1].name, "$b");
    assert_eq!(children[2].name, "$tmp");
}

#[test]
fn test_global_symbols() {
    let source = r#"
(module
  (global $counter (mut i32) (i32.const 0))
  (global $max i32 (i32.const 100))
)
"#;

    let symbols = parse_document(source).unwrap();
    let result = provide_document_symbols(&symbols);

    assert_eq!(result.len(), 2);

    let counter = &result[0];
    assert_eq!(counter.name, "$counter");
    assert_eq!(counter.kind, SymbolKind::CONSTANT);
    assert_eq!(counter.detail.as_deref(), Some("mut i32"));

    let max = &result[1];
    assert_eq!(max.name, "$max");
    assert_eq!(max.detail.as_deref(), Some("i32"));
}

#[test]
fn test_type_symbols() {
    let source = r#"
(module
  (type $sig (func (param i32) (result i32)))
)
"#;

    let symbols = parse_document(source).unwrap();
    let result = provide_document_symbols(&symbols);

    assert_eq!(result.len(), 1);
    let type_sym = &result[0];
    assert_eq!(type_sym.name, "$sig");
    assert_eq!(type_sym.kind, SymbolKind::INTERFACE);
}

#[test]
fn test_memory_and_table_symbols() {
    let source = r#"
(module
  (memory $mem 1 10)
  (table $tbl 1 funcref)
)
"#;

    let symbols = parse_document(source).unwrap();
    let result = provide_document_symbols(&symbols);

    assert_eq!(result.len(), 2);

    // Table comes before memory in the output order (tables, then memories)
    let table = result.iter().find(|s| s.name == "$tbl").unwrap();
    assert_eq!(table.kind, SymbolKind::ARRAY);

    let memory = result.iter().find(|s| s.name == "$mem").unwrap();
    assert_eq!(memory.kind, SymbolKind::OBJECT);
}

#[test]
fn test_block_labels() {
    let source = r#"
(module
  (func $test
    (block $outer
      (loop $inner
        (br $inner)
      )
    )
  )
)
"#;

    let symbols = parse_document(source).unwrap();
    let result = provide_document_symbols(&symbols);

    assert_eq!(result.len(), 1);
    let func = &result[0];
    let children = func.children.as_ref().unwrap();

    // Should have 2 block labels
    assert_eq!(children.len(), 2);

    let outer = children.iter().find(|c| c.name == "$outer").unwrap();
    assert_eq!(outer.kind, SymbolKind::KEY);
    assert_eq!(outer.detail.as_deref(), Some("block"));

    let inner = children.iter().find(|c| c.name == "$inner").unwrap();
    assert_eq!(inner.detail.as_deref(), Some("loop"));
}

#[test]
fn test_unnamed_symbols_use_index() {
    let source = r#"
(module
  (func (param i32) (result i32)
    (local.get 0)
  )
)
"#;

    let symbols = parse_document(source).unwrap();
    let result = provide_document_symbols(&symbols);

    assert_eq!(result.len(), 1);
    let func = &result[0];
    // Unnamed function should show index
    assert_eq!(func.name, "(func 0)");

    // Check that unnamed param also uses index
    let children = func.children.as_ref().unwrap();
    assert_eq!(children[0].name, "(param 0)");
}
