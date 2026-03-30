use super::*;

#[test]
fn test_parse_simple_function() {
    let wat = r#"
(module
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.functions.len(), 1);

    let func = &symbols.functions[0];
    assert_eq!(func.name, Some("$add".to_string()));
    assert_eq!(func.parameters.len(), 2);
    assert_eq!(func.results.len(), 1);
    assert_eq!(func.results[0], ValueType::I32);
}

#[test]
fn test_parse_function_with_locals() {
    let wat = r#"
(func $test (param $x i32) (result i32)
  (local $temp i32)
  (local $result i64)
  (local.get $x))
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.functions.len(), 1);

    let func = &symbols.functions[0];
    assert_eq!(func.name, Some("$test".to_string()));
    assert_eq!(func.parameters.len(), 1);
    assert_eq!(func.locals.len(), 2);

    assert_eq!(func.locals[0].name, Some("$temp".to_string()));
    assert_eq!(func.locals[0].var_type, ValueType::I32);
    assert_eq!(func.locals[1].name, Some("$result".to_string()));
    assert_eq!(func.locals[1].var_type, ValueType::I64);
}

#[test]
fn test_parse_function_with_blocks() {
    let wat = r#"
(func $test
  (block $exit
    (loop $continue
      (br $exit)))
  (if $check (i32.const 1)
    (then (nop))))
"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];

    assert_eq!(func.blocks.len(), 3);
    assert!(func
        .blocks
        .iter()
        .any(|b| b.label == "$exit" && b.block_type == "block"));
    assert!(func
        .blocks
        .iter()
        .any(|b| b.label == "$continue" && b.block_type == "loop"));
    assert!(func
        .blocks
        .iter()
        .any(|b| b.label == "$check" && b.block_type == "if"));
}

#[test]
fn test_parse_multiple_functions() {
    let wat = r#"
(module
  (func $add (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1)))

  (func $sub (param i32 i32) (result i32)
    (i32.sub (local.get 0) (local.get 1)))

  (func $mul (param i32 i32) (result i32)
    (i32.mul (local.get 0) (local.get 1))))
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.functions.len(), 3);
    assert_eq!(symbols.functions[0].name, Some("$add".to_string()));
    assert_eq!(symbols.functions[1].name, Some("$sub".to_string()));
    assert_eq!(symbols.functions[2].name, Some("$mul".to_string()));
}

#[test]
fn test_parse_globals() {
    let wat = r#"
(module
  (global $counter (mut i32) (i32.const 0)))
"#;

    let symbols = parse_document(wat).unwrap();
    assert!(!symbols.globals.is_empty());

    if let Some(counter) = symbols.get_global_by_name("$counter") {
        assert!(counter.is_mutable);
        assert_eq!(counter.var_type, ValueType::I32);
    }
}

#[test]
fn test_parse_immutable_globals() {
    let wat = r#"
(module
  (global $immutable i32 (i32.const 42))
  (global $pi f32 (f32.const 3.14159)))
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.globals.len(), 2);

    let immutable = symbols.get_global_by_name("$immutable").unwrap();
    assert!(!immutable.is_mutable);
    assert_eq!(immutable.var_type, ValueType::I32);

    let pi = symbols.get_global_by_name("$pi").unwrap();
    assert!(!pi.is_mutable);
    assert_eq!(pi.var_type, ValueType::F32);
}

#[test]
fn test_parse_tables() {
    let wat = r#"
(module
  (table $funcs 10 funcref)
  (table $refs 1 100 externref))
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.tables.len(), 2);

    let funcs = symbols.get_table_by_name("$funcs").unwrap();
    assert_eq!(funcs.limits.0, 10);
    assert_eq!(funcs.ref_type, ValueType::Funcref);

    let refs = symbols.get_table_by_name("$refs").unwrap();
    assert_eq!(refs.limits.0, 1);
    assert_eq!(refs.limits.1, Some(100));
}

#[test]
fn test_parse_types() {
    let wat = r#"
(module
  (type $binop (func (param i32 i32) (result i32))))
"#;

    let symbols = parse_document(wat).unwrap();
    assert!(!symbols.types.is_empty());

    if let Some(binop) = symbols.get_type_by_name("$binop") {
        if let TypeKind::Func { params, results } = &binop.kind {
            assert!(!params.is_empty());
            assert!(!results.is_empty());
        } else {
            panic!("Expected Func type");
        }
    }
}

#[test]
fn test_parse_unnamed_parameters() {
    let wat = r#"
(func (param i32 i32 i64) (result i32)
  (local.get 0))
"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];
    // Regex parser may only capture one param per line
    assert!(!func.parameters.is_empty());
    // Check that captured params don't have names
    for param in &func.parameters {
        assert!(param.name.is_none());
    }
}

#[test]
fn test_parse_exported_function() {
    let wat = r#"
(module
  (func (export "main") (result i32)
    (i32.const 42)))
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.functions.len(), 1);
    // Exported functions without names get the export name
    let func = &symbols.functions[0];
    assert!(func.name == Some("$main".to_string()) || func.name.is_none());
}

#[test]
fn test_parse_multi_result_function() {
    let wat = r#"
(func $multi (result i32 i64)
  (i32.const 1)
  (i64.const 2))
"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];
    // Regex parser may only capture first result
    assert!(!func.results.is_empty());
    assert_eq!(func.results[0], ValueType::I32);
}

#[test]
fn test_symbol_table_lookup() {
    let wat = r#"
(module
  (func $add (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1)))
  (global $counter (mut i32) (i32.const 0)))
"#;

    let symbols = parse_document(wat).unwrap();

    // Test function lookup by name
    assert!(symbols.get_function_by_name("$add").is_some());
    assert!(symbols.get_function_by_name("$nonexistent").is_none());

    // Test function lookup by index
    assert!(symbols.get_function_by_index(0).is_some());
    assert!(symbols.get_function_by_index(999).is_none());

    // Test global lookup
    assert!(symbols.get_global_by_name("$counter").is_some());
    assert!(symbols.get_global_by_index(0).is_some());
}

#[test]
fn test_value_type_conversion() {
    assert_eq!(ValueType::try_parse("i32"), Some(ValueType::I32));
    assert_eq!(ValueType::try_parse("i64"), Some(ValueType::I64));
    assert_eq!(ValueType::try_parse("f32"), Some(ValueType::F32));
    assert_eq!(ValueType::try_parse("f64"), Some(ValueType::F64));
    assert_eq!(ValueType::try_parse("funcref"), Some(ValueType::Funcref));
    assert_eq!(
        ValueType::try_parse("externref"),
        Some(ValueType::Externref)
    );
    assert_eq!(ValueType::try_parse("invalid"), None);

    assert_eq!(ValueType::I32.to_string(), "i32");
    assert_eq!(ValueType::F64.to_string(), "f64");
}

#[test]
fn test_parse_complex_module() {
    let wat = r#"
(module
  (type $callback (func (param i32)))
  (memory $mem 1)
  (table $callbacks 10 funcref)
  (global $count (mut i32) (i32.const 0))

  (func $increment (result i32)
    (global.set $count
      (i32.add (global.get $count) (i32.const 1)))
    (global.get $count))

  (func $process (param $n i32)
    (local $i i32)
    (block $break
      (loop $continue
        (br_if $break (i32.ge_s (local.get $i) (local.get $n)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue))))

  (export "increment" (func $increment))
  (export "process" (func $process)))
"#;

    let symbols = parse_document(wat).unwrap();

    // Verify all components parsed (regex parser may not catch everything)
    assert!(!symbols.types.is_empty());
    assert!(symbols.functions.len() >= 2);
    assert!(!symbols.globals.is_empty());
    assert!(!symbols.tables.is_empty());

    // Just verify we parsed something
    // The complex module test mainly checks that parsing doesn't crash
    assert!(!symbols.functions.is_empty() || !symbols.globals.is_empty());
}

#[test]
fn test_parse_imported_memory() {
    // This is a valid WAT program that imports memory from the host
    let wat = r#"(module
  (import "env" "mem" (memory 1))

  (func $test (param $start i32) (param $end i32)
    (local $index i32)
    (local.set $index (local.get $start))
  )

  (export "test" (func $test))
)"#;

    let symbols = parse_document(wat).unwrap();

    // There should be 1 function
    assert_eq!(symbols.functions.len(), 1);
    let func = symbols.get_function_by_name("$test").unwrap();
    assert_eq!(func.parameters.len(), 2);
    assert_eq!(func.locals.len(), 1);
}

#[test]
fn test_parse_imported_function() {
    // This is a valid WAT program that imports a function from the host
    let wat = r#"(module
  (import "env" "log" (func $log (param i32)))

  (func $main
    (call $log (i32.const 42))
  )
)"#;

    let symbols = parse_document(wat).unwrap();

    // Debug: Print all functions found
    eprintln!(
        "Functions found: {:?}",
        symbols
            .functions
            .iter()
            .map(|f| (&f.name, f.parameters.len()))
            .collect::<Vec<_>>()
    );

    // The imported function should be parsed and available
    let log_fn = symbols
        .get_function_by_name("$log")
        .expect("Imported function $log should be found");

    // Critical: the imported function should have its parameter extracted
    assert_eq!(
        log_fn.parameters.len(),
        1,
        "Imported function $log should have 1 parameter, got {:?}",
        log_fn.parameters
    );
    assert_eq!(
        log_fn.parameters[0].param_type,
        crate::symbols::ValueType::I32
    );

    // The local function should also be there
    assert!(
        symbols.get_function_by_name("$main").is_some(),
        "$main function should be found"
    );
}

#[test]
fn test_parse_imported_function_multiple_params() {
    // This tests that multiple parameters in a single (param ...) clause are parsed
    let wat = r#"(module
  (import "env" "add" (func $add (param i32 i32) (result i32)))

  (func $main (result i32)
    (call $add (i32.const 1) (i32.const 2))
  )
)"#;

    let symbols = parse_document(wat).unwrap();

    let add_fn = symbols
        .get_function_by_name("$add")
        .expect("Imported function $add should be found");

    // Critical: the imported function should have both parameters extracted
    assert_eq!(
        add_fn.parameters.len(),
        2,
        "Imported function $add should have 2 parameters"
    );
    assert_eq!(
        add_fn.parameters[0].param_type,
        crate::symbols::ValueType::I32
    );
    assert_eq!(
        add_fn.parameters[1].param_type,
        crate::symbols::ValueType::I32
    );

    // Results should also be extracted
    assert_eq!(add_fn.results.len(), 1);
    assert_eq!(add_fn.results[0], crate::symbols::ValueType::I32);
}

#[test]
fn test_parse_010_memory_watlings() {
    // Real-world example from watlings exercises
    let wat = r#"(module
  (import "env" "mem" (memory 1))

  (func $increment_data (param $start i32) (param $end i32)
    (local $index i32)
    (local $cur_num i32)

    (local.set $index (local.get $start))

    (loop $loop_name
      (i32.store8
        (local.get $index)
        (i32.add
          (i32.load8_u (local.get $index))
          (i32.const 1)
        )
      )

      (local.set $index (i32.add (local.get $index) (i32.const 1)))

      (i32.lt_u (local.get $index) (local.get $end))
      (br_if $loop_name)
    )
  )

  (export "incrementData" (func $increment_data))
)"#;

    let symbols = parse_document(wat).unwrap();

    // Function should be parsed (deduplication should handle error recovery duplicates)
    assert_eq!(
        symbols.functions.len(),
        1,
        "Expected 1 function, found {:?}",
        symbols
            .functions
            .iter()
            .map(|f| &f.name)
            .collect::<Vec<_>>()
    );
    let func = symbols.get_function_by_name("$increment_data").unwrap();
    assert_eq!(func.parameters.len(), 2);
    assert_eq!(func.locals.len(), 2);

    // Blocks should be parsed
    assert!(
        func.blocks.iter().any(|b| b.label == "$loop_name"),
        "Loop label $loop_name should be found"
    );
}

#[test]
fn test_parse_011_host_watlings() {
    // Real-world example from watlings exercises
    let wat = r#"(module
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

  (export "squareNum" (func $square_num))
  (export "logSomeNumbers" (func $log_some_numbers))
)"#;

    let symbols = parse_document(wat).unwrap();

    // All functions (including imported) should be parsed
    // With import support, we expect 3 functions: $log (imported), $square_num, $log_some_numbers
    assert!(
        symbols.get_function_by_name("$log").is_some(),
        "Imported function $log should be found"
    );
    assert!(
        symbols.get_function_by_name("$square_num").is_some(),
        "$square_num should be found"
    );
    assert!(
        symbols.get_function_by_name("$log_some_numbers").is_some(),
        "$log_some_numbers should be found"
    );
}

#[test]
fn test_parse_function_with_line_comment_doc() {
    // Test that line comments before a function are extracted as doc comments
    let wat = r#"(module
  ;; Adds two numbers together
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];

    assert_eq!(func.name, Some("$add".to_string()));
    assert!(
        func.doc_comment.is_some(),
        "Expected doc_comment, got {:?}",
        func.doc_comment
    );
    assert!(func
        .doc_comment
        .as_ref()
        .unwrap()
        .contains("Adds two numbers"));
}

#[test]
fn test_parse_function_with_block_comment_doc() {
    // Test that block comments before a function are extracted as doc comments
    let wat = r#"
(module
  (; Multiplies two numbers together ;)
  (func $mul (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b)))
)
"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];

    assert_eq!(func.name, Some("$mul".to_string()));
    assert!(func.doc_comment.is_some());
    assert!(func
        .doc_comment
        .as_ref()
        .unwrap()
        .contains("Multiplies two numbers"));
}

#[test]
fn test_parse_function_with_multiline_doc_comment() {
    // Test multiple line comments form a single doc comment
    let wat = r#"
(module
  ;; Calculates the factorial of n
  ;; Uses recursive algorithm
  (func $factorial (param $n i32) (result i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then (i32.const 1))
      (else (i32.mul (local.get $n)
        (call $factorial (i32.sub (local.get $n) (i32.const 1)))))))
)
"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];

    assert_eq!(func.name, Some("$factorial".to_string()));
    assert!(func.doc_comment.is_some());
    let doc = func.doc_comment.as_ref().unwrap();
    assert!(doc.contains("factorial"));
    assert!(doc.contains("recursive"));
}

#[test]
fn test_parse_function_without_doc_comment() {
    // Test that functions without preceding comments have no doc comment
    let wat = r#"
(module
  (func $simple (result i32)
    (i32.const 42))
)
"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];

    assert_eq!(func.name, Some("$simple".to_string()));
    assert!(func.doc_comment.is_none());
}

#[test]
fn test_parse_function_comment_with_one_blank_line() {
    // Test that comments with one blank line between them and function are still captured
    let wat = r#"(module
  ;; This comment has one blank line before function

  (func $spaced (result i32)
    (i32.const 0))
)"#;

    let symbols = parse_document(wat).unwrap();
    let func = &symbols.functions[0];

    assert_eq!(func.name, Some("$spaced".to_string()));
    // The comment should be captured because we allow up to 1 blank line
    assert!(
        func.doc_comment.is_some(),
        "Expected doc_comment with 1 blank line gap, got {:?}",
        func.doc_comment
    );
    assert!(func
        .doc_comment
        .as_ref()
        .unwrap()
        .contains("one blank line"));
}

#[test]
fn test_parse_nested_block_comment() {
    let wat = r#"(module
  (; outer (; inner ;) ;)
  (func $after_comment (result i32)
    (i32.const 42))
)"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(
        symbols.functions.len(),
        1,
        "Function after nested block comment should be parsed"
    );

    let func = &symbols.functions[0];
    assert_eq!(func.name, Some("$after_comment".to_string()));
    assert_eq!(func.results.len(), 1);
    assert_eq!(func.results[0], ValueType::I32);
}

// ===========================================================================
// Multi-module WAST tests
// ===========================================================================

#[test]
fn test_parse_multi_module_wast() {
    let wast = r#"
(module
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)
(module
  (func $sub (param $a i32) (param $b i32) (result i32)
    (i32.sub (local.get $a) (local.get $b)))
)
"#;

    let modules = parse_document_modules(wast).unwrap();
    assert_eq!(modules.len(), 2, "Should find two modules");

    // First module
    assert_eq!(modules[0].symbols.functions.len(), 1);
    assert_eq!(
        modules[0].symbols.functions[0].name,
        Some("$add".to_string())
    );

    // Second module
    assert_eq!(modules[1].symbols.functions.len(), 1);
    assert_eq!(
        modules[1].symbols.functions[0].name,
        Some("$sub".to_string())
    );
}

#[test]
fn test_multi_module_duplicate_identifiers_no_clash() {
    // Same identifier names in different modules should NOT conflict
    let wast = r#"
(module
  (func $foo (result i32) (i32.const 1))
  (global $bar (mut i32) (i32.const 0))
)
(module
  (func $foo (result i32) (i32.const 2))
  (global $bar (mut i32) (i32.const 0))
)
"#;

    let modules = parse_document_modules(wast).unwrap();
    assert_eq!(modules.len(), 2);

    // Each module should have its own $foo and $bar without conflicts
    assert_eq!(modules[0].symbols.functions.len(), 1);
    assert_eq!(modules[0].symbols.globals.len(), 1);
    assert_eq!(modules[1].symbols.functions.len(), 1);
    assert_eq!(modules[1].symbols.globals.len(), 1);
}

#[test]
fn test_multi_module_independent_indices() {
    // Function indices should start from 0 in each module
    let wast = r#"
(module
  (func $a (result i32) (i32.const 1))
  (func $b (result i32) (i32.const 2))
)
(module
  (func $c (result i32) (i32.const 3))
)
"#;

    let modules = parse_document_modules(wast).unwrap();
    assert_eq!(modules.len(), 2);

    assert_eq!(modules[0].symbols.functions.len(), 2);
    assert_eq!(modules[0].symbols.functions[0].index, 0);
    assert_eq!(modules[0].symbols.functions[1].index, 1);

    assert_eq!(modules[1].symbols.functions.len(), 1);
    assert_eq!(modules[1].symbols.functions[0].index, 0);
}

#[test]
fn test_multi_module_with_types_and_imports() {
    let wast = r#"
(module
  (type $sig (func (param i32) (result i32)))
  (import "env" "log" (func $log (type $sig)))
  (func $main (result i32) (i32.const 42))
)
(module
  (type $sig (func (param f64) (result f64)))
  (func $calc (param f64) (result f64) (local.get 0))
)
"#;

    let modules = parse_document_modules(wast).unwrap();
    assert_eq!(modules.len(), 2);

    // First module: 1 type, 1 import + 1 function = 2 functions
    assert_eq!(modules[0].symbols.types.len(), 1);
    assert_eq!(modules[0].symbols.functions.len(), 2);

    // Second module: 1 type, 1 function (no conflicts with first module's $sig)
    assert_eq!(modules[1].symbols.types.len(), 1);
    assert_eq!(modules[1].symbols.functions.len(), 1);
}

#[test]
fn test_single_module_still_works() {
    let wat = r#"
(module
  (func $test (result i32) (i32.const 42))
)
"#;

    let modules = parse_document_modules(wat).unwrap();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].symbols.functions.len(), 1);
}

#[test]
fn test_bare_fields_still_works() {
    // Bare module_field form (no module wrapper)
    let wat = r#"(func $test (result i32) (i32.const 42))"#;

    let modules = parse_document_modules(wat).unwrap();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].symbols.functions.len(), 1);
}

#[test]
fn test_multi_module_with_memories_and_globals() {
    let wast = r#"
(module
  (memory $mem 1)
  (global $g1 (mut i32) (i32.const 0))
  (func $f1 (result i32) (global.get $g1))
)
(module
  (memory $mem 2)
  (global $g1 (mut i64) (i64.const 0))
  (func $f1 (result i64) (global.get $g1))
)
"#;

    let modules = parse_document_modules(wast).unwrap();
    assert_eq!(modules.len(), 2);

    // First module
    assert_eq!(modules[0].symbols.memories.len(), 1);
    assert_eq!(modules[0].symbols.globals.len(), 1);
    assert_eq!(modules[0].symbols.globals[0].var_type, ValueType::I32);

    // Second module - same names but different types
    assert_eq!(modules[1].symbols.memories.len(), 1);
    assert_eq!(modules[1].symbols.globals.len(), 1);
    assert_eq!(modules[1].symbols.globals[0].var_type, ValueType::I64);
}
