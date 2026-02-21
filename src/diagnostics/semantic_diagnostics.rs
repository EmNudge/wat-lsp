use crate::diagnostics_core::tree_walk::{walk_tree_for_diagnostics, DiagnosticConfig};
use crate::symbols::SymbolTable;
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

/// Provide semantic diagnostics for undefined references and parameter count validation
pub fn provide_semantic_diagnostics(
    tree: &Tree,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let config = DiagnosticConfig::from_symbols(symbols);

    // Single unified tree walk for all semantic checks (shared implementation)
    let mut core_diagnostics = Vec::new();
    walk_tree_for_diagnostics(
        tree.root_node(),
        source,
        symbols,
        &config,
        &mut core_diagnostics,
    );

    // Validate subtype hierarchy
    core_diagnostics.extend(crate::diagnostics_core::subtype::validate_subtype_hierarchy(symbols));

    // Module-level structural validations
    core_diagnostics.extend(
        crate::diagnostics_core::module_checks::validate_module_structure(
            &tree.root_node(),
            source,
            symbols,
        ),
    );

    // Convert all core diagnostics to tower_lsp types
    core_diagnostics.into_iter().map(Into::into).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics_core::memory_checks::{
        is_atomic_memory_operation, is_memory_load_store,
    };
    use crate::parser::parse_document;
    use crate::tree_sitter_bindings::create_parser;

    #[test]
    fn test_valid_label_reference() {
        let document = r#"(func $test
  (block $myblock
    (br $myblock)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid label reference should have no diagnostics"
        );
    }

    #[test]
    fn test_undefined_label_reference() {
        let document = r#"(func $test
  (block $myblock
    (br $undefined)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            1,
            "Undefined label should produce one diagnostic"
        );
        assert!(diagnostics[0].message.contains("Undefined label"));
        assert!(diagnostics[0].message.contains("$undefined"));
    }

    #[test]
    fn test_nested_blocks_valid_reference() {
        let document = r#"(func $test
  (block $outer
    (block $inner
      (br $outer)
      (br $inner))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid nested label references should have no diagnostics"
        );
    }

    #[test]
    fn test_numeric_label_reference() {
        // Numeric references (0, 1, 2) are valid and refer to block depth
        // We should not produce diagnostics for these
        let document = r#"(func $test
  (block
    (br 0)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Numeric label references should not produce diagnostics"
        );
    }

    #[test]
    fn test_undefined_function_call() {
        let document = r#"(func $defined
  (nop))

(func $test
  (call $defined)
  (call $undefined))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            1,
            "Undefined function should produce one diagnostic"
        );
        assert!(diagnostics[0].message.contains("Undefined function"));
        assert!(diagnostics[0].message.contains("$undefined"));
    }

    #[test]
    fn test_undefined_start_function() {
        let document = r#"(module
  (start $main)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined function"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            1,
            "Undefined start function should produce one diagnostic"
        );
        assert!(undefined_diags[0].message.contains("$main"));
    }

    #[test]
    fn test_valid_start_function() {
        let document = r#"(module
  (func $main (nop))
  (start $main)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            0,
            "Valid start function should not produce diagnostics"
        );
    }

    #[test]
    fn test_undefined_local() {
        let document = r#"(func $test (param $x i32) (local $y i32)
  local.get $x
  local.get $y
  local.set $undefined)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            1,
            "Undefined local should produce one diagnostic"
        );
        assert!(undefined_diags[0].message.contains("Undefined local"));
        assert!(undefined_diags[0].message.contains("$undefined"));
    }

    #[test]
    fn test_undefined_global() {
        let document = r#"(global $g i32 (i32.const 42))

(func $test
  (global.get $g)
  (global.set $undefined (i32.const 0)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            1,
            "Undefined global should produce one diagnostic"
        );
        assert!(undefined_diags[0].message.contains("Undefined global"));
        assert!(undefined_diags[0].message.contains("$undefined"));
    }

    #[test]
    fn test_all_valid_references() {
        let document = r#"(global $g i32 (i32.const 0))

(func $helper
  nop)

(func $test (param $p i32) (local $l i32)
  (block $label
    local.get $p
    local.set $l
    global.get $g
    call $helper
    br $label))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "All valid references should produce no diagnostics"
        );
    }

    #[test]
    fn test_ref_type_in_result() {
        // Regression test: (ref $type) in result/param types should not be flagged as undefined
        let document = r#"(module
  (type $point (struct (field $x i32) (field $y i32)))

  (func $make_point (param $x i32) (param $y i32) (result (ref $point))
    (struct.new $point (local.get $x) (local.get $y))
  )

  (func $get_x (param $p (ref $point)) (result i32)
    (struct.get $point $x (local.get $p))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid type references in (ref $type) should not produce diagnostics, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_br_if_with_local_get() {
        // Regression test: local.get inside br_if condition should not be flagged as undefined label
        let document = r#"(func $test (param $n i32)
    (local $i i32)
    (block $break
      (br_if $break
        (i32.gt_s (local.get $i) (local.get $n)))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid local.get inside br_if should not produce diagnostics"
        );
    }

    // Parameter count validation tests

    #[test]
    fn test_local_set_correct_param_count() {
        let document = r#"(func $test (local $x i32)
  local.set $x)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        // Should have no parameter count errors (local.set expects 1 param and has 1)
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid local.set with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_local_set_missing_param() {
        // Note: Completely missing parameters like (local.set) create syntax errors
        // This test is skipped as tree-sitter will flag it as an ERROR node
        // Our parameter validation only runs on syntactically valid AST nodes
    }

    #[test]
    fn test_i32_const_correct() {
        let document = r#"(func $test
  (i32.const 42))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid i32.const with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_i32_const_missing_value() {
        // Note: Completely missing parameters like (i32.const) create syntax errors
        // This test is skipped as tree-sitter will flag it as an ERROR node
        // Our parameter validation only runs on syntactically valid AST nodes
    }

    #[test]
    fn test_drop_linear_format() {
        // Linear format: drop consumes from stack (no explicit operands)
        let document = r#"(func $test
  drop)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid drop in linear format should have no diagnostics"
        );
    }

    #[test]
    fn test_i32_add_linear_format() {
        // Linear format: i32.add consumes from stack (no explicit operands)
        let document = r#"(func $test
  i32.add)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid i32.add in linear format should have no diagnostics"
        );
    }

    #[test]
    fn test_folded_expression_correct() {
        // Folded expressions with correct operand counts should NOT trigger errors
        let document = r#"(func $test
  (i32.add (i32.const 1) (i32.const 2)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Correct folded expressions should not produce errors"
        );
    }

    #[test]
    fn test_folded_expression_too_many_operands() {
        // i32.add with 3 operands instead of 2
        let document = r#"(func $test
  (i32.add (i32.const 1) (i32.const 2) (i32.const 3)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("i32.add") && d.message.contains("expects"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            1,
            "i32.add with 3 operands should produce one error"
        );
        assert!(operand_errors[0].message.contains("2 operands"));
        assert!(operand_errors[0].message.contains("got 3"));
    }

    #[test]
    fn test_folded_expression_partial_is_valid() {
        // i32.add with 1 inline operand - the other comes from the stack (mixed style)
        // This is valid WAT: (i32.const 5) (i32.add (i32.const 1)) means add 1 to stack top
        let document = r#"(func $test
  (i32.add (i32.const 1)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("i32.add") && d.message.contains("expects"))
            .collect();
        // Partial folding is valid - remaining operands come from the stack
        assert_eq!(
            operand_errors.len(),
            0,
            "i32.add with 1 operand is valid (other from stack)"
        );
    }

    #[test]
    fn test_folded_unary_op_correct() {
        // i32.eqz with 1 operand (correct)
        let document = r#"(func $test
  (i32.eqz (i32.const 42)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Correct unary operation should not produce errors"
        );
    }

    #[test]
    fn test_folded_unary_op_too_many() {
        // i32.eqz with 2 operands instead of 1
        let document = r#"(func $test
  (i32.eqz (i32.const 1) (i32.const 2)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("i32.eqz") && d.message.contains("expects"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            1,
            "i32.eqz with 2 operands should produce one error"
        );
        assert!(operand_errors[0].message.contains("1 operand"));
        assert!(operand_errors[0].message.contains("got 2"));
    }

    #[test]
    fn test_folded_nested_correct() {
        // Nested folded expressions with correct operand counts
        let document = r#"(func $test
  (i32.add
    (i32.mul (i32.const 2) (i32.const 3))
    (i32.const 4)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Correct nested folded expressions should not produce errors"
        );
    }

    #[test]
    fn test_global_get_correct() {
        let document = r#"(global $g i32 (i32.const 0))
(func $test
  (global.get $g))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid global.get with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_br_correct() {
        let document = r#"(func $test
  (block $myblock
    (br $myblock)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid br with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_call_correct() {
        let document = r#"(func $helper (nop))
(func $test
  (call $helper))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid call with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_call_with_unfolded_args() {
        // Unfolded style: arguments are pushed to stack before the call
        // This should NOT produce false errors (issue #78)
        let document = r#"(module
  (func $add (param i32 i32) (result i32)
    i32.add)

  (func $main (result i32)
    (i32.const 1)
    (i32.const 2)
    (call $add)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("operand"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            0,
            "Unfolded call style should not produce false operand errors: {:?}",
            operand_errors
        );
    }

    #[test]
    fn test_call_with_too_many_operands() {
        // Extra operands in folded call stay on the stack; the type checker
        // catches the resulting stack mismatch at the function level.
        let document = r#"(module
  (func $single (param i32) (result i32)
    local.get 0)

  (func $main (result i32)
    (call $single (i32.const 1) (i32.const 2))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.message.contains("type mismatch") || d.message.contains("Stack underflow")
            })
            .collect();
        assert!(
            !type_errors.is_empty(),
            "Call with extra operands should produce a type checker error: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_mixed_valid_invalid() {
        // Test a mix of valid instructions in linear format
        let document = r#"(func $test (local $x i32)
  local.set $x
  i32.const 42
  drop
  nop)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let param_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects"))
            .collect();
        assert_eq!(
            param_errors.len(),
            0,
            "All instructions have correct parameter counts, should have no errors"
        );
    }

    #[test]
    fn test_throw_with_correct_operand_count() {
        // Tag with 1 param, throw with 1 operand - should be valid
        let document = r#"(module
  (tag $div_error (param i32))

  (func $test
    (throw $div_error (i32.const 400))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        // Verify tag params were extracted correctly
        let tag = symbols.get_tag_by_name("$div_error").unwrap();
        assert_eq!(tag.params.len(), 1, "Tag should have 1 param");

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("operand"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            0,
            "throw with correct operand count should have no errors, got: {:?}",
            operand_errors
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_throw_with_extra_operands() {
        // Tag with 0 params, throw with 1 operand in folded form — the extra
        // value stays on the stack. Since throw transfers control, this is valid
        // WAT (the type checker accepts unreachable stack states).
        let document = r#"(module
  (tag $empty_error)

  (func $test
    (throw $empty_error (i32.const 400))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        // Verify tag params were extracted correctly
        let tag = symbols.get_tag_by_name("$empty_error").unwrap();
        assert_eq!(tag.params.len(), 0, "Tag should have 0 params");

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("operand"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            0,
            "Extra operands before throw are valid in folded form: {:?}",
            operand_errors
        );
    }

    #[test]
    fn test_try_table_catch_clause_valid() {
        // Test that catch clause correctly distinguishes tag vs label references
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

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        // Filter out any errors about the catch clause
        let catch_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("$caught") || d.message.contains("$div_error"))
            .collect();

        assert_eq!(
            catch_errors.len(),
            0,
            "Valid catch clause should not produce errors for tag or label, got: {:?}",
            catch_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_try_table_catch_undefined_tag() {
        // Test that undefined tag in catch clause is reported correctly
        let document = r#"(module
  (func $test (result i32)
    (block $caught (result i32)
      (try_table (result i32) (catch $undefined_tag $caught)
        (i32.const 42)
      )
    )
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let tag_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined tag") && d.message.contains("$undefined_tag"))
            .collect();

        assert_eq!(
            tag_errors.len(),
            1,
            "Undefined tag in catch clause should produce one error"
        );
    }

    #[test]
    fn test_try_table_catch_undefined_label() {
        // Test that undefined label in catch clause is reported correctly
        let document = r#"(module
  (tag $my_error)

  (func $test (result i32)
    (block $other (result i32)
      (try_table (result i32) (catch $my_error $undefined_label)
        (i32.const 42)
      )
    )
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let label_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.message.contains("Undefined label") && d.message.contains("$undefined_label")
            })
            .collect();

        assert_eq!(
            label_errors.len(),
            1,
            "Undefined label in catch clause should produce one error"
        );
    }

    #[test]
    fn test_try_table_catch_all_valid() {
        // Test that catch_all clause (no tag, just label) works correctly
        let document = r#"(module
  (func $test (result i32)
    (block $caught (result i32)
      (try_table (result i32) (catch_all $caught)
        (i32.const 42)
      )
    )
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let catch_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("$caught"))
            .collect();

        assert_eq!(
            catch_errors.len(),
            0,
            "Valid catch_all clause should not produce errors, got: {:?}",
            catch_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_call_ref_with_correct_operand_count() {
        // Function type with 2 params, call_ref should expect 3 operands (2 params + funcref)
        let document = r#"(module
  (type $binop (func (param i32 i32) (result i32)))

  (func $test (param $fn (ref $binop)) (result i32)
    (call_ref $binop (i32.const 1) (i32.const 2) (local.get $fn))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        // Verify type was parsed with correct params
        let type_def = symbols.get_type_by_name("$binop").unwrap();
        if let crate::symbols::TypeKind::Func { params, .. } = &type_def.kind {
            assert_eq!(params.len(), 2, "Type should have 2 params");
        }

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("operand"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            0,
            "call_ref with correct operand count should have no errors, got: {:?}",
            operand_errors
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_call_ref_with_too_many_operands() {
        // Function type with 1 param, call_ref with 4 operands — extra values
        // stay on the stack and the type checker catches the mismatch.
        let document = r#"(module
  (type $unary (func (param i32) (result i32)))

  (func $test (param $fn (ref $unary)) (result i32)
    (call_ref $unary (i32.const 1) (i32.const 2) (i32.const 3) (local.get $fn))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.message.contains("type mismatch") || d.message.contains("Stack underflow")
            })
            .collect();
        assert!(
            !type_errors.is_empty(),
            "call_ref with extra operands should produce a type checker error: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_call_ref_bare_index_syntax() {
        // call_ref takes a bare type index (not type_use like call_indirect)
        let document = r#"(module
  (type $t (func (param i32) (result i32)))
  (func $inc (type $t) (param $x i32) (result i32)
    local.get $x
    i32.const 1
    i32.add)
  (table 1 funcref)
  (elem (i32.const 0) $inc)

  (func (export "dispatch") (param $x i32) (result i32)
    local.get $x
    ref.func $inc
    call_ref $t)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("call_ref") || d.message.contains("type"))
            .collect();
        assert_eq!(
            errors.len(),
            0,
            "call_ref $t should produce no errors, got: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_return_call_ref_bare_index_syntax() {
        // return_call_ref takes a bare type index
        let document = r#"(module
  (type $t (func (param i32) (result i32)))
  (func $inc (type $t) (param $x i32) (result i32)
    local.get $x
    i32.const 1
    i32.add)

  (func $dispatch (type $t) (param $x i32) (param $f funcref) (result i32)
    local.get $x
    local.get $f
    return_call_ref $t)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("return_call_ref") || d.message.contains("Syntax error"))
            .collect();
        assert_eq!(
            errors.len(),
            0,
            "return_call_ref $t should produce no errors, got: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    // Atomic operations validation tests

    #[test]
    fn test_atomic_op_with_shared_memory_no_warning() {
        // Atomic operation with shared memory should not produce warnings
        let document = r#"(module
  (memory $mem 1 1 shared)

  (func $test
    (i32.atomic.load (i32.const 0))
    drop
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        // Verify memory is shared
        assert!(
            symbols.memories[0].shared,
            "Memory should be marked as shared"
        );

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let atomic_warnings: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Atomic operation"))
            .collect();
        assert_eq!(
            atomic_warnings.len(),
            0,
            "Atomic op with shared memory should not produce warnings"
        );
    }

    #[test]
    fn test_atomic_op_without_shared_memory_warning() {
        // Atomic operation without shared memory should produce a warning
        let document = r#"(module
  (memory $mem 1)

  (func $test
    (i32.atomic.load (i32.const 0))
    drop
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        // Verify memory is NOT shared
        assert!(!symbols.memories[0].shared, "Memory should not be shared");

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let atomic_warnings: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Atomic operation") && d.message.contains("shared"))
            .collect();
        assert_eq!(
            atomic_warnings.len(),
            1,
            "Atomic op without shared memory should produce a warning"
        );
        assert!(atomic_warnings[0].message.contains("i32.atomic.load"));
    }

    #[test]
    fn test_atomic_fence_no_warning() {
        // atomic.fence doesn't require shared memory
        let document = r#"(module
  (memory $mem 1)

  (func $test
    atomic.fence
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let atomic_warnings: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Atomic operation"))
            .collect();
        assert_eq!(
            atomic_warnings.len(),
            0,
            "atomic.fence should not require shared memory"
        );
    }

    #[test]
    fn test_multiple_atomic_ops_warnings() {
        // Multiple atomic operations should each produce a warning
        let document = r#"(module
  (memory 1)

  (func $test
    (i32.atomic.load (i32.const 0))
    drop
    (i64.atomic.store (i32.const 0) (i64.const 42))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let atomic_warnings: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Atomic operation"))
            .collect();
        assert_eq!(
            atomic_warnings.len(),
            2,
            "Each atomic op should produce a warning"
        );
    }

    #[test]
    fn test_is_atomic_memory_operation() {
        // Test the helper function
        assert!(is_atomic_memory_operation("i32.atomic.load"));
        assert!(is_atomic_memory_operation("i64.atomic.store"));
        assert!(is_atomic_memory_operation("i32.atomic.rmw.add"));
        assert!(is_atomic_memory_operation("memory.atomic.wait32"));
        assert!(is_atomic_memory_operation("memory.atomic.notify"));

        // atomic.fence should NOT require shared memory
        assert!(!is_atomic_memory_operation("atomic.fence"));

        // Regular operations should not match
        assert!(!is_atomic_memory_operation("i32.load"));
        assert!(!is_atomic_memory_operation("i64.store"));
        assert!(!is_atomic_memory_operation("memory.grow"));
    }

    // Memory64 tests

    #[test]
    fn test_memory64_no_hints_for_regular_memory() {
        // Regular memory should not produce memory64 hints
        let document = r#"(module
  (memory 1)

  (func $test (result i32)
    (memory.size)
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory64_hints: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("memory64-type".to_string())))
            .collect();
        assert_eq!(
            memory64_hints.len(),
            0,
            "Regular memory should not produce memory64 hints"
        );
    }

    #[test]
    fn test_memory64_hints_for_memory_size() {
        // memory64 memory should produce hints for memory.size
        let document = r#"(module
  (memory i64 1)

  (func $test (result i64)
    (memory.size)
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        // Verify memory64 was detected
        assert!(
            symbols
                .memories
                .first()
                .map(|m| m.is_memory64)
                .unwrap_or(false),
            "Memory should be detected as memory64"
        );

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory64_hints: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("memory64"))
            .collect();
        assert!(
            !memory64_hints.is_empty(),
            "memory64 should produce hints for memory.size"
        );
    }

    #[test]
    fn test_memory64_hints_for_memory_grow() {
        // memory64 memory should produce hints for memory.grow
        let document = r#"(module
  (memory i64 1)

  (func $test (result i64)
    (memory.grow (i64.const 1))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory64_hints: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("memory64") && d.message.contains("memory.grow"))
            .collect();
        assert!(
            !memory64_hints.is_empty(),
            "memory64 should produce hints for memory.grow"
        );
    }

    #[test]
    fn test_memory64_hints_for_load_store() {
        // memory64 memory should produce hints for load/store
        let document = r#"(module
  (memory i64 1)

  (func $test (result i32)
    (i32.load (i64.const 0))
  )
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory64_hints: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("i64 address"))
            .collect();
        assert!(
            !memory64_hints.is_empty(),
            "memory64 should produce hints about i64 address for load"
        );
    }

    #[test]
    fn test_is_memory_load_store() {
        // Test the helper function
        assert!(is_memory_load_store("i32.load"));
        assert!(is_memory_load_store("i64.load"));
        assert!(is_memory_load_store("f32.load"));
        assert!(is_memory_load_store("f64.load"));
        assert!(is_memory_load_store("i32.load8_s"));
        assert!(is_memory_load_store("i32.load8_u"));
        assert!(is_memory_load_store("i32.load16_s"));
        assert!(is_memory_load_store("i64.load32_u"));

        assert!(is_memory_load_store("i32.store"));
        assert!(is_memory_load_store("i64.store"));
        assert!(is_memory_load_store("f32.store"));
        assert!(is_memory_load_store("i32.store8"));
        assert!(is_memory_load_store("i64.store32"));

        assert!(is_memory_load_store("v128.load"));
        assert!(is_memory_load_store("v128.store"));

        // Non-memory operations should not match
        assert!(!is_memory_load_store("i32.add"));
        assert!(!is_memory_load_store("memory.size"));
        assert!(!is_memory_load_store("local.get"));
    }

    #[test]
    fn test_legacy_try_catch_valid() {
        // Test that legacy try syntax with valid tag reference works
        let document = r#"(module
  (tag $my_error (param i32))

  (func (export "test") (result i32)
    (try (result i32)
      (do (i32.const 42))
      (catch $my_error (drop) (i32.const 0))))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let tag_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined tag") && d.message.contains("$my_error"))
            .collect();

        assert_eq!(
            tag_errors.len(),
            0,
            "Valid tag in legacy try catch should not produce errors, got: {:?}",
            tag_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_legacy_try_catch_undefined_tag() {
        // Test that undefined tag in legacy try catch clause is reported correctly
        let document = r#"(module
  (func (export "test")
    (try
      (do (nop))
      (catch $missing (drop))))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let tag_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined tag") && d.message.contains("$missing"))
            .collect();

        assert_eq!(
            tag_errors.len(),
            1,
            "Undefined tag in legacy try catch should produce one error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_legacy_try_delegate_valid() {
        // Test that legacy try delegate with valid label reference works
        let document = r#"(module
  (func (export "test")
    (block $outer
      (try
        (do (nop))
        (delegate $outer))))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let label_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined label") && d.message.contains("$outer"))
            .collect();

        assert_eq!(
            label_errors.len(),
            0,
            "Valid label in legacy try delegate should not produce errors, got: {:?}",
            label_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_legacy_try_delegate_undefined_label() {
        // Test that undefined label in legacy try delegate clause is reported correctly
        let document = r#"(module
  (func (export "test")
    (try
      (do (nop))
      (delegate $missing)))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let label_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined label") && d.message.contains("$missing"))
            .collect();

        assert_eq!(
            label_errors.len(),
            1,
            "Undefined label in legacy try delegate should produce one error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    // Multi-memory load/store validation tests

    #[test]
    fn test_undefined_memory_in_load_instruction() {
        // Load instruction referencing undefined memory should produce an error
        let document = r#"(module
  (memory $other 1)
  (func (export "test") (result i32)
    (i32.load $missing (i32.const 0))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined memory") && d.message.contains("$missing"))
            .collect();
        assert_eq!(
            memory_errors.len(),
            1,
            "Undefined memory in i32.load should produce one error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_undefined_memory_in_store_instruction() {
        // Store instruction referencing undefined memory should produce an error
        let document = r#"(module
  (memory $mem0 1)
  (func (export "test")
    (i32.store $nonexistent (i32.const 0) (i32.const 42))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.message.contains("Undefined memory") && d.message.contains("$nonexistent")
            })
            .collect();
        assert_eq!(
            memory_errors.len(),
            1,
            "Undefined memory in i32.store should produce one error"
        );
    }

    #[test]
    fn test_valid_memory_in_load_store_instructions() {
        // Load/store with valid memory references should not produce errors
        let document = r#"(module
  (memory $mem0 1)
  (memory $mem1 2)
  (func $test (param $addr i32) (result i32)
    (i32.store $mem0 (local.get $addr) (i32.const 42))
    (i32.store $mem1 (local.get $addr) (i32.const 43))
    (i32.load $mem0 (local.get $addr))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined memory"))
            .collect();
        assert_eq!(
            memory_errors.len(),
            0,
            "Valid memory references should not produce errors, got: {:?}",
            memory_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_undefined_memory_in_load_with_offset() {
        // Load with offset and align still needs memory validation
        let document = r#"(module
  (memory $mem0 1)
  (func (param $addr i32) (result i32)
    (i32.load $undefined offset=4 align=4 (local.get $addr))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined memory") && d.message.contains("$undefined"))
            .collect();
        assert_eq!(
            memory_errors.len(),
            1,
            "Undefined memory in i32.load with offset should produce one error"
        );
    }

    #[test]
    fn test_undefined_memory_various_load_types() {
        // Test various load instruction types
        let document = r#"(module
  (memory $mem 1)
  (func (result i64)
    (i64.load $bad (i32.const 0))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let memory_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined memory") && d.message.contains("$bad"))
            .collect();
        assert_eq!(
            memory_errors.len(),
            1,
            "Undefined memory in i64.load should produce one error"
        );
    }

    // === Tests for missing operand errors in nested expressions ===

    #[test]
    fn test_binary_op_missing_operand_nested() {
        // i32.sub is nested inside i32.add - should error because it must be self-contained
        // i32.add is at top level - should NOT error (valid partial folding)
        let document = r#"(module
  (func $test (result i32)
    (i32.add
      (i32.sub (i32.const 5)))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let missing_operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects") && d.message.contains("operands, but got"))
            .collect();

        // Only i32.sub should error (nested, missing 1 operand)
        // i32.add is at top level, so valid partial folding
        assert_eq!(
            missing_operand_errors.len(),
            1,
            "Only nested i32.sub should error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );

        // Verify it's an error for i32.sub
        assert!(missing_operand_errors[0].message.contains("i32.sub"));
        assert_eq!(
            missing_operand_errors[0].severity,
            Some(DiagnosticSeverity::ERROR)
        );
    }

    #[test]
    fn test_binary_op_top_level_no_error() {
        // Binary op with 1 operand at top level - valid partial folding, no error
        // (another value could be on the stack from preceding instructions)
        let document = r#"(module
  (func $test (result i32)
    (i32.sub (i32.const 5))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let missing_operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects") && d.message.contains("operands, but got"))
            .collect();

        assert_eq!(
            missing_operand_errors.len(),
            0,
            "Top-level binary op with 1 operand is valid partial folding, should not error"
        );
    }

    #[test]
    fn test_valid_partial_folding_no_error() {
        // Partial folding with value on stack - should NOT error
        let document = r#"(module
  (func $test (result i32)
    (i32.const 5)
    (i32.add (i32.const 3))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let missing_operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects") && d.message.contains("operands, but got"))
            .collect();

        assert_eq!(
            missing_operand_errors.len(),
            0,
            "Valid partial folding should not produce errors"
        );
    }

    #[test]
    fn test_fully_folded_correct_no_warning() {
        // Fully folded expression with all operands - should NOT warn
        let document = r#"(module
  (func $test (result i32)
    (i32.add (i32.const 5) (i32.const 3))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("operand"))
            .collect();

        assert_eq!(
            operand_errors.len(),
            0,
            "Fully folded expression should produce no operand warnings/errors"
        );
    }

    #[test]
    fn test_call_missing_operand_nested() {
        // Call with missing operand when nested - should error
        let document = r#"(module
  (func $double (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1)))
  (func $test (result i32)
    (i32.add
      (call $double (i32.const 1))
      (i32.const 0))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let missing_operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects") && d.message.contains("operands, but got"))
            .collect();

        assert_eq!(
            missing_operand_errors.len(),
            1,
            "Nested call with missing operand should produce error"
        );
    }

    #[test]
    fn test_select_missing_operands() {
        // Select with only 1 operand - should error (expects 3)
        let document = r#"(module
  (func $test (result i32)
    (i32.add
      (select (i32.const 1))
      (i32.const 0))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let missing_operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects") && d.message.contains("operands, but got"))
            .collect();

        assert_eq!(
            missing_operand_errors.len(),
            1,
            "Select with 1 operand (expects 3) should produce error"
        );
    }

    #[test]
    fn test_unary_op_no_operand_no_error() {
        // Unary op with no explicit operands - could be intentional stack usage
        // This should NOT error because operand_count is 0 (not > 0)
        let document = r#"(module
  (func $test (result i32)
    (i32.const 5)
    (i32.eqz)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let missing_operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects") && d.message.contains("operands, but got"))
            .collect();

        assert_eq!(
            missing_operand_errors.len(),
            0,
            "Unary op with no explicit operands should not error (valid stack usage)"
        );
    }

    #[test]
    fn test_error_has_correct_code() {
        // Verify error has proper diagnostic code for filtering
        // Use a fully folded outer expression with one incomplete nested expression
        let document = r#"(module
  (func $test (result i32)
    (i32.add
      (i32.sub (i32.const 5))
      (i32.const 3))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let missing_operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects") && d.message.contains("operands, but got"))
            .collect();

        // Only i32.sub should error (missing 1 operand), i32.add has both operands
        assert_eq!(
            missing_operand_errors.len(),
            1,
            "Only the nested incomplete expression should error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        // Verify the diagnostic message is about missing operands
        assert!(
            missing_operand_errors[0].message.contains("expects"),
            "Expected diagnostic about missing operands"
        );
    }

    // === Stack underflow tests for unfolded WAT ===

    #[test]
    fn test_stack_underflow_basic() {
        // i32.add with completely empty stack
        let document = r#"(module
  (func $test
    i32.add))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "i32.add with empty stack should produce underflow error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(underflow_errors[0].message.contains("i32.add"));
        assert!(underflow_errors[0].message.contains("requires 2"));
        assert!(underflow_errors[0].message.contains("0 available"));
    }

    #[test]
    fn test_stack_underflow_partial() {
        // i32.add with only 1 value on stack (needs 2)
        let document = r#"(module
  (func $test
    i32.const 1
    i32.add))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "i32.add with 1 value should produce underflow error"
        );
        assert!(underflow_errors[0].message.contains("requires 2"));
        assert!(underflow_errors[0].message.contains("1 available"));
    }

    #[test]
    fn test_stack_underflow_mixed_folded_linear() {
        // Issue #93: folded expression with insufficient stack values
        let document = r#"(module
  (func $test (result i32)
    i32.const 42
    (i32.add)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "folded i32.add with only 1 stack value should produce underflow error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(underflow_errors[0].message.contains("i32.add"));
        assert!(underflow_errors[0].message.contains("requires 2"));
        assert!(underflow_errors[0].message.contains("1 available"));
    }

    #[test]
    fn test_stack_no_underflow_folded_with_inline_operands() {
        // Folded expression with all operands provided inline - should be fine
        let document = r#"(module
  (func $test (result i32)
    (i32.add (i32.const 1) (i32.const 2))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "folded i32.add with both operands inline should not produce underflow error"
        );
    }

    #[test]
    fn test_stack_no_underflow_mixed_partial_inline() {
        // Folded expression with one operand on stack, one inline - should be fine
        let document = r#"(module
  (func $test (result i32)
    i32.const 42
    (i32.add (i32.const 1))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "folded i32.add with 1 inline operand and 1 stack value should not produce underflow error"
        );
    }

    #[test]
    fn test_stack_underflow_folded_empty_stack() {
        // Folded expression with no operands and empty stack
        let document = r#"(module
  (func $test (result i32)
    (i32.add)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "folded i32.add with empty stack should produce underflow error"
        );
        assert!(underflow_errors[0].message.contains("requires 2"));
        assert!(underflow_errors[0].message.contains("0 available"));
    }

    #[test]
    fn test_stack_no_underflow_correct_sequence() {
        // Correct sequence: push 2 values, then add
        let document = r#"(module
  (func $test (result i32)
    i32.const 1
    i32.const 2
    i32.add))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "Correct stack sequence should not produce underflow"
        );
    }

    #[test]
    fn test_stack_underflow_drop() {
        // drop with empty stack
        let document = r#"(module
  (func $test
    drop))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "drop with empty stack should produce underflow error"
        );
        assert!(underflow_errors[0].message.contains("drop"));
    }

    #[test]
    fn test_stack_underflow_local_set() {
        // local.set without value on stack
        let document = r#"(module
  (func $test (local $x i32)
    local.set $x))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "local.set with empty stack should produce underflow error"
        );
        assert!(underflow_errors[0].message.contains("local.set"));
    }

    #[test]
    fn test_stack_no_underflow_local_get_then_set() {
        // local.get produces 1, local.set consumes 1
        let document = r#"(module
  (func $test (local $x i32) (local $y i32)
    local.get $x
    local.set $y))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "local.get then local.set should balance"
        );
    }

    #[test]
    fn test_stack_underflow_call() {
        // call a function that takes 2 params with only 1 on stack
        let document = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  (func $test (result i32)
    i32.const 1
    call $add))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "call with insufficient args should produce underflow error"
        );
        assert!(underflow_errors[0].message.contains("call"));
        assert!(underflow_errors[0].message.contains("requires 2"));
    }

    #[test]
    fn test_stack_no_underflow_after_unreachable() {
        // After unreachable, subsequent code is dead - don't report errors
        let document = r#"(module
  (func $test
    unreachable
    i32.add))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "Code after unreachable should not report underflow"
        );
    }

    #[test]
    fn test_stack_mixed_folded_unfolded() {
        // Folded expression pushes result, then unfolded uses it
        let document = r#"(module
  (func $test (result i32)
    (i32.const 5)
    (i32.const 3)
    i32.add))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "Mixed folded/unfolded should work correctly"
        );
    }

    #[test]
    fn test_stack_select_underflow() {
        // select needs 3 values (val1, val2, condition)
        let document = r#"(module
  (func $test (result i32)
    i32.const 1
    i32.const 2
    select))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            1,
            "select with 2 values should produce underflow (needs 3)"
        );
        assert!(underflow_errors[0].message.contains("select"));
        assert!(underflow_errors[0].message.contains("requires 3"));
    }

    #[test]
    fn test_stack_chain_of_operations() {
        // Chain: const 1, const 2, add (produces 1), const 3, mul (produces 1)
        let document = r#"(module
  (func $test (result i32)
    i32.const 1
    i32.const 2
    i32.add
    i32.const 3
    i32.mul))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "Valid chain of operations should not produce underflow"
        );
    }

    #[test]
    fn test_linear_format_with_call() {
        // Linear format: function call consumes stack values and produces result
        let document = r#"(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)
  (func $compute (result i32)
    i32.const 1
    i32.const 2
    call $add))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert_eq!(
            underflow_errors.len(),
            0,
            "Linear format with call should not produce underflow"
        );

        let return_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("return-arity-mismatch".to_string())))
            .collect();
        assert_eq!(
            return_errors.len(),
            0,
            "Functions should have correct return values on stack"
        );
    }

    #[test]
    fn test_try_table_catch_branch_to_outer_block() {
        // Regression test for issue #108:
        // Control flow analysis should handle try_table catch clauses that branch to outer blocks.
        // Even though the try_table body always terminates (return), the catch clause can
        // branch to $outer_catch, so the enclosing block doesn't "always terminate".
        //
        // Pattern: catch branches to outer block, block result IS the function result
        let document = r#"(module
  (tag $error (param i32))

  (func $safe_op (param $x i32) (result i32)
    (block $catch_block (result i32)
      (try_table (result i32) (catch $error $catch_block)
        (if (i32.lt_s (local.get $x) (i32.const 0))
          (then (throw $error (i32.const -1))))
        (return (local.get $x)))))
)"#;
        // Normal path: body returns $x
        // Exception path: catch branches to $catch_block with error payload (i32), block produces i32

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let return_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.code == Some(NumberOrString::String("return-arity-mismatch".to_string()))
                    || d.code == Some(NumberOrString::String("return-type-mismatch".to_string()))
            })
            .collect();

        assert_eq!(
            return_errors.len(),
            0,
            "try_table with catch branch to outer block should not cause false positive errors, got: {:?}",
            return_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_try_table_body_terminates_with_catch() {
        // Another test for issue #108:
        // When try_table body terminates but has catch clauses, the outer block
        // doesn't "always terminate" because catch can branch out.
        //
        // Pattern: body always terminates (unreachable), but catch provides exit path
        let document = r#"(module
  (tag $error (param i32))

  (func $test (result i32)
    (block $catch_handler (result i32)
      (try_table (result i32) (catch $error $catch_handler)
        (unreachable))))
)"#;
        // Only path: exception caught, branches to $catch_handler with i32 payload

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.code == Some(NumberOrString::String("return-arity-mismatch".to_string()))
                    || d.code == Some(NumberOrString::String("return-type-mismatch".to_string()))
            })
            .collect();

        assert_eq!(
            type_errors.len(),
            0,
            "try_table with unreachable body but catch clause should not cause false positives, got: {:?}",
            type_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_try_table_with_code_after_block() {
        // Test pattern from issue #108 where code after block is only reached via catch
        // The catch branches to an outer block with no result type
        let document = r#"(module
  (tag $error)

  (func $test (result i32)
    (block $on_error
      (try_table (catch $error $on_error)
        (return (i32.const 42))))
    (i32.const -1))
)"#;
        // Normal path: body returns 42
        // Exception path: catch branches to $on_error (no result), continues to i32.const -1

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);

        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.code == Some(NumberOrString::String("return-arity-mismatch".to_string()))
                    || d.code == Some(NumberOrString::String("return-type-mismatch".to_string()))
            })
            .collect();

        assert_eq!(
            type_errors.len(),
            0,
            "try_table with code after block (reached via catch) should not cause false positives, got: {:?}",
            type_errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_relaxed_simd_stack_underflow() {
        // f32x4.relaxed_madd needs 3 v128 operands, only 2 provided
        let document = r#"(module
  (func $bad_madd (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    f32x4.relaxed_madd)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow = diagnostics
            .iter()
            .filter(|d| d.message.contains("Stack underflow"))
            .collect::<Vec<_>>();
        assert_eq!(
            underflow.len(),
            1,
            "relaxed_madd with 2 operands (needs 3) should produce stack underflow"
        );
    }

    #[test]
    fn test_simd_i32x4_add_no_false_positive() {
        // Regression test: SIMD binary ops must consume 2 and produce 1 on the stack.
        // Previously i32x4.add was unknown to the arity map, so the stack tracker
        // skipped it, leaving 2 values on the stack and emitting a false type mismatch.
        let document = r#"(module
  (func (export "add4") (param $x v128) (param $y v128) (result v128)
    local.get $x
    local.get $y
    i32x4.add)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid SIMD i32x4.add should produce no diagnostics, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_simd_various_ops_no_false_positive() {
        // Broader SIMD coverage: unary, ternary, splat, extract, replace, reduction
        let document = r#"(module
  (func $unary (param $a v128) (result v128)
    local.get $a
    f32x4.neg)

  (func $splat (param $x i32) (result v128)
    local.get $x
    i32x4.splat)

  (func $bitwise (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    v128.and)

  (func $not (param $a v128) (result v128)
    local.get $a
    v128.not)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid SIMD functions should produce no diagnostics, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_if_return_no_else_linear() {
        // Linear if without else — return inside if, fallthrough produces value
        let document = r#"(module
  (func (param $x i32) (result i32)
    local.get $x
    i32.eqz
    if
      i32.const 0
      return
    end
    i32.const 1
  )
)"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid linear if/return/end should have no diagnostics, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    // --- Multi-value block param tests (issue #123) ---

    #[test]
    fn test_block_with_params_no_false_positive() {
        // Block with (param i32 i32) (result i32) containing i32.add
        // The block consumes 2 values from the outer stack and produces 1
        let document = r#"(module
  (func $test (result i32)
    i32.const 1
    i32.const 2
    block (param i32 i32) (result i32)
      i32.add
    end))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.code == Some(NumberOrString::String("return-arity-mismatch".to_string()))
                    || d.code == Some(NumberOrString::String("stack-underflow".to_string()))
            })
            .collect();
        assert_eq!(
            errors.len(),
            0,
            "Block with params should not produce false errors, got: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_loop_with_params_no_false_positive() {
        // Loop with (param i32) (result i32) — consumes 1 from outer stack, produces 1
        let document = r#"(module
  (func $test (result i32)
    i32.const 42
    loop (param i32) (result i32)
    end))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.code == Some(NumberOrString::String("return-arity-mismatch".to_string()))
                    || d.code == Some(NumberOrString::String("stack-underflow".to_string()))
            })
            .collect();
        assert_eq!(
            errors.len(),
            0,
            "Loop with params should not produce false errors, got: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_if_with_block_params_no_false_positive() {
        // If with block params: consumes condition (1) + block params from outer stack
        let document = r#"(module
  (func $test (param $x i32) (result i32)
    i32.const 10
    local.get $x
    if (param i32) (result i32)
      i32.const 1
      i32.add
    else
      i32.const 2
      i32.add
    end))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.code == Some(NumberOrString::String("return-arity-mismatch".to_string()))
                    || d.code == Some(NumberOrString::String("stack-underflow".to_string()))
            })
            .collect();
        assert_eq!(
            errors.len(),
            0,
            "If with block params should not produce false errors, got: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_folded_block_with_params_no_false_positive() {
        // Folded block: (block (param i32) (result i32) ...)
        let document = r#"(module
  (func $test (result i32)
    (i32.const 5)
    (block (param i32) (result i32)
      (i32.const 1)
      i32.add)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.code == Some(NumberOrString::String("return-arity-mismatch".to_string()))
                    || d.code == Some(NumberOrString::String("stack-underflow".to_string()))
            })
            .collect();
        assert_eq!(
            errors.len(),
            0,
            "Folded block with params should not produce false errors, got: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_block_params_underflow() {
        // Block with params but insufficient values on outer stack should produce underflow
        let document = r#"(module
  (func $test (result i32)
    i32.const 1
    block (param i32 i32) (result i32)
      i32.add
    end))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let underflow_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("stack-underflow".to_string())))
            .collect();
        assert!(
            !underflow_errors.is_empty(),
            "Block with insufficient outer stack values should produce underflow error"
        );
    }

    #[test]
    fn test_call_indirect_pops_args_and_table_index() {
        // Issue #121: call_indirect should pop N params + 1 table index
        let document = r#"(module
  (type $sig (func (param i32) (result i32)))
  (func $f (type $sig) (param $x i32) (result i32) local.get $x)
  (table 1 funcref)
  (elem (i32.const 0) $f)

  (func (export "go") (param $n i32) (result i32)
    i32.const 0
    local.get $n
    call_indirect (type $sig))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "call_indirect with correct stack should have no errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_call_indirect_folded_pops_args_and_table_index() {
        // Folded form of call_indirect should also count operands correctly
        let document = r#"(module
  (type $sig (func (param i32) (result i32)))
  (table 1 funcref)

  (func (export "go") (param $n i32) (result i32)
    (call_indirect (type $sig) (i32.const 0) (local.get $n)))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Folded call_indirect with correct operands should have no errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_call_indirect_multi_param() {
        // call_indirect with multi-param type should pop all params + table index
        let document = r#"(module
  (type $binop (func (param i32 i32) (result i32)))
  (table 1 funcref)

  (func (export "go") (result i32)
    i32.const 10
    i32.const 20
    i32.const 0
    call_indirect (type $binop))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "call_indirect with 2 params + table index should have no errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_call_indirect_chained_linear() {
        // Issue #132: linear call_indirect followed by another instruction should not
        // produce false type-mismatch diagnostics. The grammar may parse the pair as
        // a single instr_call node (swallowing the trailing instruction).
        let document = r#"(module
  (type $unary (func (param i32) (result i32)))
  (table 2 funcref)
  (elem (i32.const 0) func $double $double)

  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func (export "chain") (param $x i32) (result i32)
    local.get $x
    i32.const 0
    call_indirect (type $unary)
    i32.const 1
    call_indirect (type $unary))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Chained linear call_indirect should have no errors, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_return_call_type_mismatch() {
        // return_call $callee where callee returns f64 but caller returns i32
        let document = r#"(module
  (func $callee (result f64)
    f64.const 1.0)
  (func $caller (result i32)
    return_call $callee)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let tail_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("mismatch"))
            .collect();
        assert_eq!(
            tail_diags.len(),
            1,
            "Should detect return type mismatch, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(tail_diags[0].message.contains("return"));
        assert!(tail_diags[0].message.contains("type"));
    }

    #[test]
    fn test_valid_return_call() {
        // return_call $callee where both return i32 — no diagnostics
        let document = r#"(module
  (func $callee (result i32)
    i32.const 42)
  (func $caller (result i32)
    return_call $callee)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        assert_eq!(
            errors.len(),
            0,
            "Valid return_call should produce no errors, got: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_return_call_arity_mismatch() {
        // return_call $callee where callee returns 1 value but caller returns 2
        let document = r#"(module
  (func $callee (result i32)
    i32.const 42)
  (func $caller (result i32 i32)
    return_call $callee)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let tail_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("mismatch"))
            .collect();
        assert!(
            !tail_diags.is_empty(),
            "Should detect return arity mismatch, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_invalid_struct_subtype_missing_parent_fields() {
        let document = r#"(module
  (type $parent (sub (struct (field i32) (field i64))))
  (type $child (sub $parent (struct (field i32))))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let subtype_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("fields"))
            .collect();
        assert!(
            !subtype_errors.is_empty(),
            "Expected struct field count error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_valid_struct_subtype() {
        let document = r#"(module
  (type $parent (sub (struct (field i32))))
  (type $child (sub $parent (struct (field i32) (field i64))))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let subtype_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.message.contains("field")
                    || d.message.contains("parent")
                    || d.message.contains("final")
                    || d.message.contains("extend")
            })
            .collect();
        assert_eq!(
            subtype_errors.len(),
            0,
            "Expected no subtype errors, got: {:?}",
            subtype_errors
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_sub_final_cannot_be_extended() {
        let document = r#"(module
  (type $sealed (sub final (struct (field i32))))
  (type $child (sub $sealed (struct (field i32) (field i64))))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let final_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("final"))
            .collect();
        assert!(
            !final_errors.is_empty(),
            "Expected 'cannot extend final' error, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_undefined_export_func() {
        let document = r#"(module
  (export "f" (func $missing))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined function"))
            .collect();
        assert_eq!(undefined_diags.len(), 1);
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_undefined_export_global() {
        let document = r#"(module
  (export "g" (global $missing))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined global"))
            .collect();
        assert_eq!(undefined_diags.len(), 1);
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_undefined_export_table() {
        let document = r#"(module
  (export "t" (table $missing))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined table"))
            .collect();
        assert_eq!(undefined_diags.len(), 1);
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_undefined_export_memory() {
        let document = r#"(module
  (export "m" (memory $missing))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined memory"))
            .collect();
        assert_eq!(undefined_diags.len(), 1);
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_undefined_export_tag() {
        let document = r#"(module
  (export "e" (tag $missing))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined tag"))
            .collect();
        assert_eq!(undefined_diags.len(), 1);
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_valid_exports() {
        let document = r#"(module
  (func $f (nop))
  (global $g i32 (i32.const 0))
  (table $t 1 funcref)
  (memory $m 1)
  (tag $e (param i32))
  (export "f" (func $f))
  (export "g" (global $g))
  (export "t" (table $t))
  (export "m" (memory $m))
  (export "e" (tag $e))
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            0,
            "Valid exports should not produce diagnostics, got: {:?}",
            undefined_diags
        );
    }

    #[test]
    fn test_undefined_elem_func_refs() {
        let document = r#"(module
  (table 1 funcref)
  (func $defined (nop))
  (elem (i32.const 0) $defined $missing)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined function"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            1,
            "Undefined elem func ref should produce one diagnostic, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_undefined_elem_table_use() {
        let document = r#"(module
  (table $t 1 funcref)
  (func $f (nop))
  (elem (table $missing) (i32.const 0) func $f)
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined table"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            1,
            "Undefined table_use should produce one diagnostic, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_undefined_data_memory_use() {
        let document = r#"(module
  (memory $m 1)
  (data (memory $missing) (i32.const 0) "hello")
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined memory"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            1,
            "Undefined memory_use should produce one diagnostic, got: {:?}",
            diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(undefined_diags[0].message.contains("$missing"));
    }

    #[test]
    fn test_valid_elem_and_data() {
        let document = r#"(module
  (table $t 2 funcref)
  (memory $m 1)
  (func $f (nop))
  (func $g (nop))
  (elem (table $t) (i32.const 0) func $f $g)
  (data (memory $m) (i32.const 0) "hello")
)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let undefined_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Undefined"))
            .collect();
        assert_eq!(
            undefined_diags.len(),
            0,
            "Valid elem and data should not produce diagnostics, got: {:?}",
            undefined_diags
        );
    }

    // ======================================================================
    // br_table label consistency tests (#190)
    // ======================================================================

    #[test]
    fn test_br_table_consistent_labels_ok() {
        // Linear format: both blocks have (result i32)
        let document = r#"(module (func (result i32)
  block (result i32)
    block (result i32)
      i32.const 1
      i32.const 0
      br_table 0 1
    end
  end
))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let br_table_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("br_table"))
            .collect();
        assert!(
            br_table_diags.is_empty(),
            "Consistent br_table targets should not produce diagnostics, got: {:?}",
            br_table_diags
        );
    }

    #[test]
    fn test_br_table_mismatched_labels() {
        // Linear format: inner block has no result, outer block has (result i32)
        let document = r#"(module (func
  block (result i32)
    block
      i32.const 0
      br_table 0 1
    end
    i32.const 42
  end
  drop
))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let br_table_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("br_table targets have inconsistent"))
            .collect();
        assert!(
            !br_table_diags.is_empty(),
            "Mismatched br_table targets should produce a diagnostic"
        );
    }

    // ======================================================================
    // Typed select tests (#192)
    // ======================================================================

    #[test]
    fn test_typed_select_ok() {
        let document = r#"(module (func (result i32)
  (i32.const 1)
  (i32.const 2)
  (i32.const 0)
  (select (result i32))
))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let type_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("type mismatch"))
            .collect();
        assert!(
            type_diags.is_empty(),
            "Valid typed select should not produce type mismatch, got: {:?}",
            type_diags
        );
    }

    #[test]
    fn test_untyped_select_ok() {
        let document = r#"(module (func (result i32)
  (i32.const 1)
  (i32.const 2)
  (i32.const 0)
  (select)
))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let stack_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                d.message.contains("Stack underflow") || d.message.contains("type mismatch")
            })
            .collect();
        assert!(
            stack_diags.is_empty(),
            "Valid untyped select should not produce diagnostics, got: {:?}",
            stack_diags
        );
    }

    // ======================================================================
    // Unused locals and parameters tests (Issue #198)
    // ======================================================================

    #[test]
    fn test_unused_local_warning() {
        let document = r#"(module (func (local $x i32) (i32.const 1)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused local"))
            .collect();
        assert_eq!(unused.len(), 1);
        assert!(unused[0].message.contains("$x"));
        assert_eq!(unused[0].severity, Some(DiagnosticSeverity::WARNING),);
    }

    #[test]
    fn test_used_local_via_get() {
        let document = r#"(module (func (local $x i32) (local.get $x) (drop)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(unused.is_empty(), "Used local should not be flagged");
    }

    #[test]
    fn test_used_local_via_set() {
        let document = r#"(module (func (local $x i32) (i32.const 1) (local.set $x)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(unused.is_empty(), "Set local should count as used");
    }

    #[test]
    fn test_used_local_via_tee() {
        let document = r#"(module (func (local $x i32) (i32.const 1) (local.tee $x) (drop)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(unused.is_empty(), "Tee local should count as used");
    }

    #[test]
    fn test_unused_parameter_hint() {
        let document = r#"(module (func (param $p i32)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused parameter"))
            .collect();
        assert_eq!(unused.len(), 1);
        assert!(unused[0].message.contains("$p"));
        assert_eq!(unused[0].severity, Some(DiagnosticSeverity::HINT),);
    }

    #[test]
    fn test_used_parameter() {
        let document = r#"(module (func (param $p i32) (result i32) (local.get $p)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(unused.is_empty(), "Used parameter should not be flagged");
    }

    #[test]
    fn test_numeric_index_counts_as_usage() {
        let document = r#"(module (func (param $p i32) (result i32) (local.get 0)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(unused.is_empty(), "Numeric index 0 should mark $p as used");
    }

    #[test]
    fn test_multiple_locals_one_unused() {
        let document = r#"(module (func
            (local $used i32) (local $unused i32)
            (local.get $used) (drop)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert_eq!(unused.len(), 1);
        assert!(unused[0].message.contains("$unused"));
    }

    #[test]
    fn test_usage_inside_nested_block() {
        let document = r#"(module (func
            (local $x i32)
            (block (local.get $x) (drop))))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(unused.is_empty(), "Usage in nested block should count");
    }

    #[test]
    fn test_unnamed_locals_no_warning() {
        let document = r#"(module (func (local i32)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(unused.is_empty(), "Unnamed locals should not get warnings");
    }

    #[test]
    fn test_no_false_positives_all_used() {
        let document = r#"(module (func (param $a i32) (param $b i32) (result i32)
            (local $c i32)
            (local.set $c (i32.add (local.get $a) (local.get $b)))
            (local.get $c)))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let unused: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("Unused"))
            .collect();
        assert!(
            unused.is_empty(),
            "All params/locals used — no warnings expected"
        );
    }

    // ======================================================================
    // Multi-memory/table bulk operation index validation (Issue #194)
    // ======================================================================

    #[test]
    fn test_memory_copy_valid_single_memory() {
        // memory.copy without explicit indices defaults to memory 0
        let document = r#"(module
            (memory 1)
            (func
                (memory.copy (i32.const 0) (i32.const 0) (i32.const 0))))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let ref_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("unknown memory"))
            .collect();
        assert!(ref_diags.is_empty());
    }

    #[test]
    fn test_memory_fill_invalid_index() {
        let document = r#"(module
            (memory 1)
            (func
                (memory.fill 5 (i32.const 0) (i32.const 0) (i32.const 0))))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let ref_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("unknown memory"))
            .collect();
        assert!(
            !ref_diags.is_empty(),
            "memory.fill with invalid index should be flagged"
        );
    }

    #[test]
    fn test_table_fill_invalid_index() {
        let document = r#"(module
            (table 1 funcref)
            (func
                (table.fill 5 (i32.const 0) (ref.null func) (i32.const 0))))"#;
        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();
        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let ref_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("unknown table"))
            .collect();
        assert!(
            !ref_diags.is_empty(),
            "table.fill with invalid index should be flagged"
        );
    }
}
