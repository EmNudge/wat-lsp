use wat_lsp_rust::parser::parse_document;
use wat_lsp_rust::symbols::TypeKind;

#[test]
fn test_parse_wasmgc_struct() {
    let wat = r#"
(module
  (type $point (struct
    (field $x i32)
    (field $y (mut i32))
  ))

  (func $create_point (result (ref $point))
    (struct.new $point (i32.const 10) (i32.const 20))
  )
)
"#;

    let symbols = parse_document(wat).unwrap();

    // Verify struct type was parsed
    assert_eq!(symbols.types.len(), 1);
    let point_type = symbols.get_type_by_name("$point").unwrap();

    if let TypeKind::Struct { fields } = &point_type.kind {
        assert_eq!(fields.len(), 2);
    } else {
        panic!("Expected Struct type");
    }

    // Verify function was parsed
    assert_eq!(symbols.functions.len(), 1);
}

#[test]
fn test_parse_wasmgc_array() {
    let wat = r#"
(module
  (type $arr (array (field (mut i32))))

  (func $create_array (result (ref $arr))
    (array.new $arr (i32.const 0) (i32.const 10))
  )
)
"#;

    let symbols = parse_document(wat).unwrap();

    // Verify array type was parsed
    assert_eq!(symbols.types.len(), 1);
    let arr_type = symbols.get_type_by_name("$arr").unwrap();

    if let TypeKind::Array { .. } = &arr_type.kind {
        // Success
    } else {
        panic!("Expected Array type");
    }
}

#[test]
fn test_parse_exception_handling_tag() {
    let wat = r#"
(module
  (tag $error (param i32))
  (tag $overflow)

  (func $test
    (throw $error (i32.const 1))
  )
)
"#;

    let symbols = parse_document(wat).unwrap();

    // Verify tags were parsed
    assert_eq!(symbols.tags.len(), 2);

    // Verify tag with param
    let error_tag = symbols.get_tag_by_name("$error").unwrap();
    assert_eq!(error_tag.params.len(), 1, "Tag $error should have 1 param");

    // Verify tag without param
    let overflow_tag = symbols.get_tag_by_name("$overflow").unwrap();
    assert_eq!(
        overflow_tag.params.len(),
        0,
        "Tag $overflow should have 0 params"
    );
}

#[test]
fn test_parse_exception_handling_try_table() {
    let wat = r#"
(module
  (tag $error (param i32))

  (func $safe_op (result i32)
    (block $handler (result i32)
      (try_table (result i32) (catch $error $handler)
        (i32.const 42)
      )
    )
  )
)
"#;

    let symbols = parse_document(wat).unwrap();

    // Verify function with try_table was parsed
    assert_eq!(symbols.functions.len(), 1);
}

#[test]
fn test_parse_i31_operations() {
    let wat = r#"
(module
  (func $i31_test (result i32)
    (i31.get_s (ref.i31 (i32.const 42)))
  )
)
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.functions.len(), 1);
}

#[test]
fn test_parse_ref_test_cast() {
    let wat = r#"
(module
  (type $point (struct (field i32)))

  (func $ref_test (param $ref anyref) (result i32)
    (ref.test (ref $point) (local.get $ref))
  )

  (func $ref_cast (param $ref anyref) (result (ref $point))
    (ref.cast (ref $point) (local.get $ref))
  )
)
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.functions.len(), 2);
}

#[test]
fn test_parse_rec_types() {
    let wat = r#"
(module
  (rec
    (type $node (struct
      (field $value i32)
      (field $next (ref null $node))
    ))
  )
)
"#;

    let symbols = parse_document(wat).unwrap();

    // Verify rec type was parsed
    assert_eq!(symbols.types.len(), 1);
    assert!(symbols.get_type_by_name("$node").is_some());
}

#[test]
fn test_parse_relaxed_simd() {
    let wat = r#"
(module
  (func $relaxed_test (param $v v128) (result v128)
    (f32x4.relaxed_madd (local.get $v) (local.get $v) (local.get $v))
  )
)
"#;

    let symbols = parse_document(wat).unwrap();
    assert_eq!(symbols.functions.len(), 1);
}
