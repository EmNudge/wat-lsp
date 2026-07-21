use super::*;
use crate::parser::parse_modules_from_tree;
use crate::tree_sitter_bindings::create_parser;

/// Parse a document and produce semantic tokens.
fn tokens_for(source: &str) -> Vec<SemanticTokenInfo> {
    let mut parser = create_parser();
    let tree = parser.parse(source, None).unwrap();
    let modules = parse_modules_from_tree(&tree, source).unwrap();
    provide_semantic_tokens(source, &modules, &tree)
}

/// Find the (line, character) of the nth occurrence of `needle` in `source`.
fn pos_of(source: &str, needle: &str, occurrence: usize) -> (u32, u32) {
    let mut seen = 0;
    for (line_num, line) in source.lines().enumerate() {
        let mut from = 0;
        while let Some(col) = line[from..].find(needle) {
            if seen == occurrence {
                return (line_num as u32, (from + col) as u32);
            }
            seen += 1;
            from += col + 1;
        }
    }
    panic!("occurrence {} of {:?} not found", occurrence, needle);
}

/// Find the token covering a given position, if any.
fn token_at(tokens: &[SemanticTokenInfo], line: u32, ch: u32) -> Option<&SemanticTokenInfo> {
    tokens
        .iter()
        .find(|t| t.line == line && t.start_char <= ch && ch < t.start_char + t.length)
}

/// Assert a token exists `offset` characters into the nth occurrence of
/// `needle` (the offset points at the identifier/index inside the needle).
fn expect_token<'a>(
    tokens: &'a [SemanticTokenInfo],
    source: &str,
    needle: &str,
    occurrence: usize,
    offset: u32,
) -> &'a SemanticTokenInfo {
    let (line, ch) = pos_of(source, needle, occurrence);
    token_at(tokens, line, ch + offset).unwrap_or_else(|| {
        panic!(
            "no token at occurrence {} of {:?} + {} (line {}, char {})",
            occurrence,
            needle,
            offset,
            line,
            ch + offset
        )
    })
}

const SAMPLE: &str = r#"(module
  (type $point (struct (field $x (mut f64)) (field $y (mut f64))))
  (import "env" "ext" (func $ext (param i32)))
  (global $counter (mut i32) (i32.const 0))
  (global $limit i32 (i32.const 10))
  (memory $mem 1)
  (table $tbl 1 funcref)
  (tag $err (param i32))
  (func $add (param $a i32) (param $b i32) (result i32)
    (local $sum i32)
    local.get $a
    local.get $b
    i32.add
    local.set $sum
    global.get $limit
    drop
    local.get $sum)
  (func $main
    (block $exit
      (loop $top
        global.get $counter
        drop
        br $top))
    (call $add (i32.const 1) (i32.const 2))
    drop)
  (start $main)
  (export "add" (func $add))
)"#;

#[test]
fn test_function_declaration_and_references() {
    let tokens = tokens_for(SAMPLE);

    let decl = expect_token(&tokens, SAMPLE, "func $add", 0, 5);
    assert_eq!(decl.kind, SemanticTokenKind::Function);
    assert!(decl.is_declaration);
    assert_eq!(decl.length, 4);

    let call_ref = expect_token(&tokens, SAMPLE, "call $add", 0, 5);
    assert_eq!(call_ref.kind, SemanticTokenKind::Function);
    assert!(!call_ref.is_declaration);

    // (export "add" (func $add)) — second occurrence of "func $add"
    let export_ref = expect_token(&tokens, SAMPLE, "func $add", 1, 5);
    assert_eq!(export_ref.kind, SemanticTokenKind::Function);
    assert!(!export_ref.is_declaration);
}

#[test]
fn test_imported_function_declaration() {
    let tokens = tokens_for(SAMPLE);
    let decl = expect_token(&tokens, SAMPLE, "func $ext", 0, 5);
    assert_eq!(decl.kind, SemanticTokenKind::Function);
    assert!(decl.is_declaration);
}

#[test]
fn test_params_and_locals() {
    let tokens = tokens_for(SAMPLE);

    let param_decl = expect_token(&tokens, SAMPLE, "param $a", 0, 6);
    assert_eq!(param_decl.kind, SemanticTokenKind::Parameter);
    assert!(param_decl.is_declaration);

    let param_ref = expect_token(&tokens, SAMPLE, "local.get $a", 0, 10);
    assert_eq!(param_ref.kind, SemanticTokenKind::Parameter);
    assert!(!param_ref.is_declaration);

    let local_decl = expect_token(&tokens, SAMPLE, "local $sum", 0, 7);
    assert_eq!(local_decl.kind, SemanticTokenKind::Local);
    assert!(local_decl.is_declaration);

    let local_ref = expect_token(&tokens, SAMPLE, "local.set $sum", 0, 10);
    assert_eq!(local_ref.kind, SemanticTokenKind::Local);
    assert!(!local_ref.is_declaration);
}

#[test]
fn test_globals_readonly_modifier() {
    let tokens = tokens_for(SAMPLE);

    let mutable_decl = expect_token(&tokens, SAMPLE, "global $counter", 0, 7);
    assert_eq!(mutable_decl.kind, SemanticTokenKind::Global);
    assert!(mutable_decl.is_declaration);
    assert!(!mutable_decl.is_readonly);

    let immutable_decl = expect_token(&tokens, SAMPLE, "global $limit", 0, 7);
    assert_eq!(immutable_decl.kind, SemanticTokenKind::Global);
    assert!(immutable_decl.is_readonly);

    let immutable_ref = expect_token(&tokens, SAMPLE, "global.get $limit", 0, 11);
    assert_eq!(immutable_ref.kind, SemanticTokenKind::Global);
    assert!(!immutable_ref.is_declaration);
    assert!(immutable_ref.is_readonly);

    let mutable_ref = expect_token(&tokens, SAMPLE, "global.get $counter", 0, 11);
    assert_eq!(mutable_ref.kind, SemanticTokenKind::Global);
    assert!(!mutable_ref.is_readonly);
}

#[test]
fn test_block_labels() {
    let tokens = tokens_for(SAMPLE);

    let block_decl = expect_token(&tokens, SAMPLE, "block $exit", 0, 6);
    assert_eq!(block_decl.kind, SemanticTokenKind::Label);
    assert!(block_decl.is_declaration);

    let loop_decl = expect_token(&tokens, SAMPLE, "loop $top", 0, 5);
    assert_eq!(loop_decl.kind, SemanticTokenKind::Label);
    assert!(loop_decl.is_declaration);

    let br_ref = expect_token(&tokens, SAMPLE, "br $top", 0, 3);
    assert_eq!(br_ref.kind, SemanticTokenKind::Label);
    assert!(!br_ref.is_declaration);
}

#[test]
fn test_type_and_struct_fields() {
    let tokens = tokens_for(SAMPLE);

    let type_decl = expect_token(&tokens, SAMPLE, "type $point", 0, 5);
    assert_eq!(type_decl.kind, SemanticTokenKind::Type);
    assert!(type_decl.is_declaration);

    let field_decl = expect_token(&tokens, SAMPLE, "field $x", 0, 6);
    assert_eq!(field_decl.kind, SemanticTokenKind::Property);
    assert!(field_decl.is_declaration);
}

#[test]
fn test_module_level_entities() {
    let tokens = tokens_for(SAMPLE);

    let mem = expect_token(&tokens, SAMPLE, "memory $mem", 0, 7);
    assert_eq!(mem.kind, SemanticTokenKind::Memory);

    let tbl = expect_token(&tokens, SAMPLE, "table $tbl", 0, 6);
    assert_eq!(tbl.kind, SemanticTokenKind::Table);

    let tag = expect_token(&tokens, SAMPLE, "tag $err", 0, 4);
    assert_eq!(tag.kind, SemanticTokenKind::Tag);
}

#[test]
fn test_start_references_function() {
    let tokens = tokens_for(SAMPLE);
    let start_ref = expect_token(&tokens, SAMPLE, "start $main", 0, 6);
    assert_eq!(start_ref.kind, SemanticTokenKind::Function);
    assert!(!start_ref.is_declaration);
}

#[test]
fn test_numeric_indices() {
    let source = r#"(module
  (global $g (mut i32) (i32.const 0))
  (func $f (param i32) (result i32)
    (local i32)
    local.get 0
    local.set 1
    global.get 0
    drop
    call 0)
)"#;
    let tokens = tokens_for(source);

    let param_ref = expect_token(&tokens, source, "local.get 0", 0, 10);
    assert_eq!(param_ref.kind, SemanticTokenKind::Parameter);

    let local_ref = expect_token(&tokens, source, "local.set 1", 0, 10);
    assert_eq!(local_ref.kind, SemanticTokenKind::Local);

    let global_ref = expect_token(&tokens, source, "global.get 0", 0, 11);
    assert_eq!(global_ref.kind, SemanticTokenKind::Global);

    let call_ref = expect_token(&tokens, source, "call 0", 0, 5);
    assert_eq!(call_ref.kind, SemanticTokenKind::Function);
}

#[test]
fn test_const_operands_are_not_tokens() {
    let source = r#"(module
  (func $f (result i32)
    i32.const 0
    (i32.add (i32.const 1) (i32.const 2)))
)"#;
    let tokens = tokens_for(source);

    // None of the i32.const operands should produce a token, even though a
    // function with index 0 exists.
    for occurrence in 0..3 {
        let (line, ch) = pos_of(source, "i32.const ", occurrence);
        assert!(
            token_at(&tokens, line, ch + 10).is_none(),
            "i32.const operand {} should not be classified",
            occurrence
        );
    }
}

#[test]
fn test_unresolved_reference_gets_no_token() {
    let source = r#"(module
  (func $f
    call $undefined)
)"#;
    let tokens = tokens_for(source);
    let (line, ch) = pos_of(source, "$undefined", 0);
    assert!(token_at(&tokens, line, ch).is_none());
}

#[test]
fn test_multi_module_resolution() {
    let source = r#"(module
  (func $f (result i32) (i32.const 1))
)
(module
  (global $g i32 (i32.const 2))
  (func $f (result i32)
    global.get $g)
)"#;
    let tokens = tokens_for(source);

    // $f declared in both modules
    let decl_a = expect_token(&tokens, source, "func $f", 0, 5);
    assert_eq!(decl_a.kind, SemanticTokenKind::Function);
    assert!(decl_a.is_declaration);
    let decl_b = expect_token(&tokens, source, "func $f", 1, 5);
    assert_eq!(decl_b.kind, SemanticTokenKind::Function);
    assert!(decl_b.is_declaration);

    // $g reference resolves against the second module's symbols
    let g_ref = expect_token(&tokens, source, "global.get $g", 0, 11);
    assert_eq!(g_ref.kind, SemanticTokenKind::Global);
    assert!(g_ref.is_readonly);
}

#[test]
fn test_tokens_sorted_by_position() {
    let tokens = tokens_for(SAMPLE);
    assert!(!tokens.is_empty());
    for pair in tokens.windows(2) {
        assert!(
            (pair[0].line, pair[0].start_char) <= (pair[1].line, pair[1].start_char),
            "tokens must be sorted by document position"
        );
    }
}

#[test]
fn test_delta_encoding() {
    let source = "(module (func $f) (start $f))";
    let mut parser = create_parser();
    let tree = parser.parse(source, None).unwrap();
    let modules = parse_modules_from_tree(&tree, source).unwrap();

    let absolute = provide_semantic_tokens(source, &modules, &tree);
    let encoded = provide_semantic_tokens_lsp(source, &modules, &tree);
    assert_eq!(absolute.len(), encoded.len());

    // Reconstruct absolute positions from the deltas and compare.
    let mut line = 0u32;
    let mut ch = 0u32;
    for (abs, enc) in absolute.iter().zip(encoded.iter()) {
        line += enc.delta_line;
        if enc.delta_line > 0 {
            ch = enc.delta_start;
        } else {
            ch += enc.delta_start;
        }
        assert_eq!(
            (line, ch, enc.length),
            (abs.line, abs.start_char, abs.length)
        );
    }

    // $f decl and $f start-reference must both be functions
    assert_eq!(encoded[0].token_type, 0);
    assert_eq!(encoded[0].token_modifiers_bitset & 1, 1); // declaration
    assert_eq!(encoded[1].token_type, 0);
    assert_eq!(encoded[1].token_modifiers_bitset & 1, 0);
}
