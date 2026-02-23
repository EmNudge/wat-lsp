//! Core semantic diagnostic functions shared between native and WASM builds.
//!
//! This module provides platform-agnostic functions for:
//! - Stack tracking through instruction lists
//! - Type inference for instructions
//! - Return type validation
//!
//! Uses a TypeChecker implementing the Wasm spec validation algorithm (§3.3)
//! to detect both stack underflow AND type mismatches.

// Allow useless_asref because kind.as_ref() is needed for WASM (String -> &str)
// but is a no-op for native (&str -> &str)
#![allow(clippy::useless_asref)]

use crate::core::types::Diagnostic;
use crate::instruction_metadata::{
    infer_simd_instruction_arity, is_terminating_instruction, lookup_instruction_arity, OperandMode,
};
use crate::symbols::{ElemSegment, Memory, SymbolTable, Table, TypeDef, TypeKind, ValueType};
use crate::utils::node_to_range;

use super::sequence_always_terminates;
use super::type_check::{types_compatible_with_symbols, CtrlOpcode, TypeChecker};
use crate::parser::parse_wat_nat;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

#[cfg(feature = "native")]
type RetNode<'a> = Node<'a>;

#[cfg(all(feature = "wasm", not(feature = "native")))]
type RetNode<'a> = Node;

/// Get the ValueType corresponding to an instruction's prefix (i32., i64., f32., f64., v128.).
fn type_from_prefix(name: &str) -> Option<ValueType> {
    if name.starts_with("i32.") {
        Some(ValueType::I32)
    } else if name.starts_with("i64.") {
        Some(ValueType::I64)
    } else if name.starts_with("f32.") {
        Some(ValueType::F32)
    } else if name.starts_with("f64.") {
        Some(ValueType::F64)
    } else if name.starts_with("v128.") {
        Some(ValueType::V128)
    } else {
        None
    }
}

/// Check if an instruction name refers to a SIMD lane operation (contains x2./x4./x8./x16.).
fn is_simd_instruction(name: &str) -> bool {
    name.contains("x2.") || name.contains("x4.") || name.contains("x8.") || name.contains("x16.")
}

/// Derive the splat input type from an instruction prefix (e.g. "i8x16" -> I32, "f32x4" -> F32).
fn simd_splat_input_type(name: &str) -> ValueType {
    if name.starts_with("i8x16.") || name.starts_with("i16x8.") || name.starts_with("i32x4.") {
        ValueType::I32
    } else if name.starts_with("i64x2.") {
        ValueType::I64
    } else if name.starts_with("f32x4.") {
        ValueType::F32
    } else if name.starts_with("f64x2.") {
        ValueType::F64
    } else {
        ValueType::I32
    }
}

/// Derive consumed types for SIMD lane instructions (i8x16.*, i16x8.*, i32x4.*, i64x2.*, f32x4.*, f64x2.*).
fn derive_simd_consumed_types(name: &str) -> Option<Vec<ValueType>> {
    // Splat: scalar -> v128
    if name.ends_with(".splat") {
        return Some(vec![simd_splat_input_type(name)]);
    }

    // Extract lane: v128 -> scalar (lane index is immediate, not stack operand)
    if name.contains(".extract_lane") {
        return Some(vec![ValueType::V128]);
    }

    // Replace lane: v128 + scalar -> v128
    if name.contains(".replace_lane") {
        return Some(vec![ValueType::V128, simd_splat_input_type(name)]);
    }

    // Reductions: v128 -> i32
    if name.ends_with(".all_true") || name.ends_with(".bitmask") {
        return Some(vec![ValueType::V128]);
    }

    // Shift operations: v128 + i32 -> v128
    if name.ends_with(".shl") || name.ends_with(".shr_s") || name.ends_with(".shr_u") {
        return Some(vec![ValueType::V128, ValueType::I32]);
    }

    // Unary ops: v128 -> v128
    if name.ends_with(".neg")
        || name.ends_with(".abs")
        || name.ends_with(".not")
        || name.ends_with(".popcnt")
        || name.ends_with(".ceil")
        || name.ends_with(".floor")
        || name.ends_with(".trunc")
        || name.ends_with(".nearest")
        || name.ends_with(".sqrt")
        || name.contains(".trunc_sat_")
        || name.contains(".extend_low_")
        || name.contains(".extend_high_")
        || name.contains(".convert_")
        || name.contains(".promote_low_")
        || name.contains(".demote_")
        || name.contains(".extadd_pairwise_")
    {
        return Some(vec![ValueType::V128]);
    }

    // Ternary relaxed SIMD ops: (v128, v128, v128) -> v128
    if name.contains(".relaxed_madd")
        || name.contains(".relaxed_nmadd")
        || name.contains(".relaxed_laneselect")
        || name.contains(".relaxed_dot_i8x16_i7x16_add")
    {
        return Some(vec![ValueType::V128, ValueType::V128, ValueType::V128]);
    }

    // Unary relaxed SIMD ops
    if name.contains(".relaxed_trunc") {
        return Some(vec![ValueType::V128]);
    }

    // i8x16.shuffle: (v128, v128) -> v128 (lane indices are immediates)
    if name == "i8x16.shuffle" {
        return Some(vec![ValueType::V128, ValueType::V128]);
    }

    // Narrow operations: (v128, v128) -> v128
    if name.contains(".narrow_") {
        return Some(vec![ValueType::V128, ValueType::V128]);
    }

    // Swizzle: (v128, v128) -> v128
    if name.ends_with(".swizzle") || name.contains(".relaxed_swizzle") {
        return Some(vec![ValueType::V128, ValueType::V128]);
    }

    // Dot product: (v128, v128) -> v128
    if name.contains(".dot_") {
        return Some(vec![ValueType::V128, ValueType::V128]);
    }

    // Extmul: (v128, v128) -> v128
    if name.contains(".extmul_") {
        return Some(vec![ValueType::V128, ValueType::V128]);
    }

    // Default: binary ops (v128, v128) -> v128 (add, sub, mul, min, max, eq, ne, lt, gt, le, ge, etc.)
    Some(vec![ValueType::V128, ValueType::V128])
}

/// Track stack state through an instruction list and report underflow/type errors.
/// If expected_results is None, skip return type validation (e.g., when function uses type reference).
/// Returns diagnostics as core::types::Diagnostic.
///
/// Uses TypeChecker with typed value stack + control stack per Wasm spec §3.3.
pub fn track_stack_in_instr_list(
    instr_list: &Node,
    source: &str,
    symbols: &SymbolTable,
    expected_results: Option<&[ValueType]>,
) -> Vec<Diagnostic> {
    let mut checker = TypeChecker::new();

    // Get function result types for the control frame
    let func_line = instr_list.start_position().row as u32;
    let result_types = if let Some(func) = symbols.find_function_containing_line(func_line) {
        expected_results.unwrap_or(&func.results)
    } else {
        expected_results.unwrap_or(&[])
    };

    // Push function-level control frame
    // Note: function parameters are local variables, NOT on the value stack.
    // The value stack starts empty; start_types is empty for functions.
    checker.push_ctrl(CtrlOpcode::Function, vec![], result_types.to_vec());

    // Process all children
    let mut cursor = instr_list.walk();
    for child in instr_list.children(&mut cursor) {
        node_kind!(kind = child);

        match kind.as_ref() {
            "instr" => {
                process_instr_node(&child, source, symbols, &mut checker);
            }
            "instr_plain" => {
                if let Some(instr_name) = get_instruction_name(&child, source) {
                    process_instruction(&child, instr_name, &mut checker, symbols, source);
                }
            }
            "expr" => {
                process_folded_expr(&child, source, symbols, &mut checker);
            }
            "instr_block" | "instr_loop" => {
                process_block_node(&child, source, symbols, &mut checker);
            }
            "instr_if" => {
                process_if_node(&child, source, symbols, &mut checker);
            }
            "instr_call" => {
                process_instr_call_node(&child, source, symbols, &mut checker);
            }
            "instr_list_call" => {
                process_call_indirect_node(&child, source, symbols, &mut checker);
            }
            _ => {}
        }
    }

    // Pop function frame — validates return types automatically
    checker.pop_ctrl(instr_list);

    checker.take_diagnostics()
}

/// Process an 'instr' wrapper node
fn process_instr_node(
    instr_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    let mut cursor = instr_node.walk();
    for child in instr_node.children(&mut cursor) {
        node_kind!(kind = child);

        match kind.as_ref() {
            "instr_plain" => {
                if let Some(instr_name) = get_instruction_name(&child, source) {
                    process_instruction(&child, instr_name, checker, symbols, source);
                }
            }
            "expr" => {
                process_folded_expr(&child, source, symbols, checker);
            }
            "instr_block" | "instr_loop" => {
                process_block_node(&child, source, symbols, checker);
            }
            "instr_if" => {
                process_if_node(&child, source, symbols, checker);
            }
            "instr_call" => {
                process_instr_call_node(&child, source, symbols, checker);
            }
            "instr_list_call" => {
                process_call_indirect_node(&child, source, symbols, checker);
            }
            _ => {}
        }
    }
}

/// Process a block or loop instruction (instr_block or instr_loop).
/// Uses push_ctrl/pop_ctrl for proper control frame tracking.
fn process_block_node(node: &Node, source: &str, symbols: &SymbolTable, checker: &mut TypeChecker) {
    let is_if = contains_block_if(node);

    // If this is a linear if, pop i32 condition from outer stack
    if is_if {
        checker.pop_expect(&ValueType::I32, node);
    }

    let label = get_block_label(node, source);
    let params = get_block_param_types(node, source, symbols);
    let results = get_block_result_types(node, source, symbols);

    let opcode = if is_if {
        CtrlOpcode::If
    } else if is_loop_node(node) {
        CtrlOpcode::Loop
    } else if is_try_table_node(node) {
        CtrlOpcode::TryTable
    } else {
        CtrlOpcode::Block
    };

    // Pop param types from outer stack (typed)
    if !params.is_empty() {
        let block_name = match opcode {
            CtrlOpcode::If => "if",
            CtrlOpcode::Loop => "loop",
            CtrlOpcode::TryTable => "try_table",
            _ => "block",
        };
        checker.pop_vals_for_instr(&params, node, block_name);
    }

    checker.push_ctrl_labeled(opcode, params, results, label);

    // Process body
    process_block_body(node, source, symbols, checker);

    // Pop control frame
    checker.pop_ctrl(node);
}

/// Process an if instruction (instr_if — folded or linear format with explicit if keyword).
fn process_if_node(node: &Node, source: &str, symbols: &SymbolTable, checker: &mut TypeChecker) {
    // Pop i32 condition
    checker.pop_expect(&ValueType::I32, node);

    let label = get_block_label(node, source);
    let params = get_block_param_types(node, source, symbols);
    let results = get_block_result_types(node, source, symbols);

    // Pop param types from outer stack
    if !params.is_empty() {
        checker.pop_vals_for_instr(&params, node, "if");
    }

    checker.push_ctrl_labeled(CtrlOpcode::If, params, results, label);

    // Process body
    process_block_body(node, source, symbols, checker);

    checker.pop_ctrl(node);
}

/// Process the body of a block/loop/if by recursing into children
fn process_block_body(node: &Node, source: &str, symbols: &SymbolTable, checker: &mut TypeChecker) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        match kind.as_ref() {
            "instr_list" => {
                // Process all instructions in the list
                let mut list_cursor = child.walk();
                for list_child in child.children(&mut list_cursor) {
                    node_kind!(list_kind = list_child);

                    match list_kind.as_ref() {
                        "instr" => {
                            process_instr_node(&list_child, source, symbols, checker);
                        }
                        "instr_plain" => {
                            if let Some(instr_name) = get_instruction_name(&list_child, source) {
                                process_instruction(
                                    &list_child,
                                    instr_name,
                                    checker,
                                    symbols,
                                    source,
                                );
                            }
                        }
                        "expr" => {
                            process_folded_expr(&list_child, source, symbols, checker);
                        }
                        "instr_block" | "instr_loop" => {
                            process_block_node(&list_child, source, symbols, checker);
                        }
                        "instr_if" => {
                            process_if_node(&list_child, source, symbols, checker);
                        }
                        "instr_call" => {
                            process_instr_call_node(&list_child, source, symbols, checker);
                        }
                        "instr_list_call" => {
                            process_call_indirect_node(&list_child, source, symbols, checker);
                        }
                        _ => {}
                    }
                }
            }
            "block_block" | "block_loop" => {
                // Nested block in linear format
                process_block_body(&child, source, symbols, checker);
            }
            "block_if" | "if_block" => {
                // Nested if block in linear format
                process_block_body(&child, source, symbols, checker);
            }
            "block_try_table" | "block_try" => {
                // Exception handling block — recurse into its body
                process_block_body(&child, source, symbols, checker);
            }
            "catch_clause" => {
                // Validate catch clause types against the target label's expected types.
                check_catch_clause_types(&child, source, symbols, checker);
            }
            "instr_else" | "else" => {
                // At else: validate then branch, reset stack for else branch
                checker.else_transition(&child);
                process_block_body(&child, source, symbols, checker);
            }
            // Direct instruction children
            "instr" => {
                process_instr_node(&child, source, symbols, checker);
            }
            "instr_plain" => {
                if let Some(instr_name) = get_instruction_name(&child, source) {
                    process_instruction(&child, instr_name, checker, symbols, source);
                }
            }
            "expr" => {
                process_folded_expr(&child, source, symbols, checker);
            }
            "instr_block" | "instr_loop" => {
                process_block_node(&child, source, symbols, checker);
            }
            "instr_if" => {
                process_if_node(&child, source, symbols, checker);
            }
            "instr_call" => {
                process_instr_call_node(&child, source, symbols, checker);
            }
            "instr_list_call" => {
                process_call_indirect_node(&child, source, symbols, checker);
            }
            _ => {}
        }
    }
}

/// Extract a label name from a block/loop/if/try_table node.
/// Searches immediate children and inner block children for an identifier.
fn get_block_label(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "identifier" {
            return Some(source[child.byte_range()].trim().to_string());
        }
        // Check inside block_block, loop_block, block_if, block_try_table, etc.
        let is_inner_block = kind == "block_block"
            || kind == "block_loop"
            || kind == "if_block"
            || kind == "block_if"
            || kind == "block_try_table"
            || kind == "block_try";
        if is_inner_block {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(ik = inner_child);
                if ik == "identifier" {
                    return Some(source[inner_child.byte_range()].trim().to_string());
                }
            }
        }
    }
    None
}

/// Check if a node is a loop (instr_loop or contains block_loop)
fn is_loop_node(node: &Node) -> bool {
    node_kind!(kind = node);

    if kind == "instr_loop" {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "block_loop" {
            return true;
        }
    }
    false
}

/// Check if an instr_block node contains a block_if child (linear if format)
fn contains_block_if(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "block_if" {
            return true;
        }
    }
    false
}

/// Check if a node is or contains a try_table block
fn is_try_table_node(node: &Node) -> bool {
    node_kind!(kind = node);

    if kind == "block_try_table" {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "block_try_table" {
            return true;
        }
    }
    false
}

/// Get the instruction name from an instr_plain node.
/// Returns a borrowed slice from `source`, avoiding allocation.
pub fn get_instruction_name<'a>(instr_node: &Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = instr_node.walk();
    for child in instr_node.children(&mut cursor) {
        node_kind!(kind = child);

        // Handle different instruction types
        if kind.starts_with("op_") {
            // For op_const, the instruction name is in the first child (pat01)
            if kind == "op_const" {
                let mut inner_cursor = child.walk();
                for inner_child in child.children(&mut inner_cursor) {
                    node_kind!(inner_kind = inner_child);

                    // pat01 contains "i32.const", "i64.const", etc.
                    if inner_kind == "pat01" || inner_kind.contains("const") {
                        return Some(source[inner_child.byte_range()].trim());
                    }
                }
                // Fallback: extract first token from the whole op_const text
                let text = &source[child.byte_range()];
                return text.split_whitespace().next();
            }
            // For other op_ nodes (like op_nullary, op_index, op_table_copy),
            // extract just the instruction name (first token)
            let text = &source[child.byte_range()];
            return text.split_whitespace().next();
        }
    }
    // Fallback: get the text of the first token
    let text = &source[instr_node.byte_range()];
    text.split_whitespace().next()
}

/// Process a linear call_indirect/return_call_indirect node (instr_list_call)
fn process_call_indirect_node(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    let text = &source[node.byte_range()];
    let instr_name = text.split_whitespace().next().unwrap_or("call_indirect");

    // Get consumed types for call_indirect
    let consumed = get_call_indirect_consumed_types(node, instr_name, symbols, source);
    if !consumed.is_empty() {
        checker.pop_vals_for_instr(&consumed, node, instr_name);
    }

    if is_terminating_instruction(instr_name) {
        if let Some(diag) = validate_tail_call_return_types(node, instr_name, symbols, source) {
            checker.diagnostics.push(diag);
        }
        checker.mark_unreachable();
        return;
    }

    // Produce result types
    let result_types = get_call_indirect_result_types(node, symbols, source);
    checker.push_vals(&result_types);
}

/// Process an instr_call node (call_indirect/return_call_indirect with trailing instr).
fn process_instr_call_node(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    process_call_indirect_node(node, source, symbols, checker);

    // Find and process the trailing instr child (the swallowed instruction)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "instr" {
            process_instr_node(&child, source, symbols, checker);
        }
    }
}

/// Process an instruction: consume typed operands, produce typed results
fn process_instruction(
    node: &Node,
    instr_name: &str,
    checker: &mut TypeChecker,
    symbols: &SymbolTable,
    source: &str,
) {
    // Handle tail call instructions
    if matches!(
        instr_name,
        "return_call" | "return_call_ref" | "return_call_indirect"
    ) {
        let consumed = get_instruction_consumed_types(node, instr_name, symbols, source);
        if !consumed.is_empty() {
            checker.pop_vals_for_instr(&consumed, node, instr_name);
        }
        if let Some(diag) = validate_tail_call_return_types(node, instr_name, symbols, source) {
            checker.diagnostics.push(diag);
        }
        checker.mark_unreachable();
        return;
    }

    // Handle branch instructions
    if instr_name == "br" {
        // Pop label types then mark unreachable
        if let Some(depth) = get_branch_depth(node, source, checker) {
            checker.pop_label_types_for_instr(depth, node, instr_name);
        }
        checker.mark_unreachable();
        return;
    }

    if instr_name == "br_if" {
        // Pop i32 condition first, then pop+push label types
        checker.pop_expect(&ValueType::I32, node);
        if let Some(depth) = get_branch_depth(node, source, checker) {
            if let Some(label_types) = checker.pop_label_types_for_instr(depth, node, instr_name) {
                checker.push_vals(&label_types);
            }
        }
        return;
    }

    if instr_name == "br_on_null" {
        // br_on_null l : [t* (ref null ht)] -> [t* (ref ht)]
        // Pop ref operand, validate label types, push non-nullable ref
        let ref_type = checker.pop_val().unwrap_or(ValueType::Unknown);
        if let Some(depth) = get_branch_depth(node, source, checker) {
            if let Some(label_types) = checker.pop_label_types_for_instr(depth, node, instr_name) {
                checker.push_vals(&label_types);
            }
        }
        checker.push_val(make_non_nullable(&ref_type));
        return;
    }

    if instr_name == "br_on_non_null" {
        // br_on_non_null l : [t* (ref null ht)] -> [t*]
        // The branch target label expects [t* (ref ht)]
        checker.pop_expect(&ValueType::Unknown, node);
        if let Some(depth) = get_branch_depth(node, source, checker) {
            if let Some(label_types) = checker.label_types_vec(depth) {
                if !label_types.is_empty() {
                    let t_star = &label_types[..label_types.len() - 1];
                    checker.pop_vals_for_instr(t_star, node, instr_name);
                    checker.push_vals(t_star);
                }
            }
        }
        return;
    }

    // br_on_cast and br_on_cast_fail are conditional branches:
    // They consume 1 ref value and push 1 ref value (the fallthrough type).
    // br_on_cast: branches if cast succeeds, falls through with source type.
    // br_on_cast_fail: branches if cast fails, falls through with target type.
    if instr_name == "br_on_cast" || instr_name == "br_on_cast_fail" {
        // Pop the source ref operand
        checker.pop_expect(&ValueType::Unknown, node);
        // Check cast types, validate against label, compute fallthrough (pushes result)
        let is_fail = instr_name == "br_on_cast_fail";
        check_br_on_cast_types(node, source, symbols, checker, is_fail);
        return;
    }

    if instr_name == "br_table" {
        // Pop i32 index
        checker.pop_expect(&ValueType::I32, node);
        let is_unreachable = checker.is_unreachable();
        let depths = get_all_branch_depths(node, source, checker);
        if let Some(&default_depth) = depths.last() {
            // Type-check using the default (last) label
            checker.pop_label_types_for_instr(default_depth, node, instr_name);
            // Validate all non-default labels have consistent arity with default.
            // Arity must always match. Type compatibility is only checked when reachable
            // (after unreachable, the polymorphic stack bottom satisfies any type).
            if let Some(default_types) = checker.label_types_vec(default_depth) {
                let default_arity = default_types.len();
                let mut reported = false;
                for &depth in depths.iter().take(depths.len().saturating_sub(1)) {
                    if reported {
                        break;
                    }
                    if let Some(target_types) = checker.label_types(depth) {
                        if target_types.len() != default_arity {
                            checker.diagnostics.push(
                                Diagnostic::error(
                                    node_to_range(node),
                                    "type mismatch: br_table targets have inconsistent types"
                                        .to_string(),
                                )
                                .with_code("type-mismatch"),
                            );
                            reported = true;
                        } else if !is_unreachable {
                            for (t, d) in target_types.iter().zip(default_types.iter()) {
                                if !types_compatible(t, d) {
                                    checker.diagnostics.push(
                                        Diagnostic::error(
                                            node_to_range(node),
                                            "type mismatch: br_table targets have inconsistent types"
                                                .to_string(),
                                        )
                                        .with_code("type-mismatch"),
                                    );
                                    reported = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        checker.mark_unreachable();
        return;
    }

    // Handle other terminating instructions
    if is_terminating_instruction(instr_name) {
        // For 'return', pop function result types
        if instr_name == "return" {
            checker.pop_function_return_types(node, instr_name);
        }
        // For 'throw', pop tag parameter types
        // For 'throw_ref', pop exnref operand
        if instr_name == "throw" || instr_name == "throw_ref" {
            let consumed = get_instruction_consumed_types(node, instr_name, symbols, source);
            if !consumed.is_empty() {
                checker.pop_vals_for_instr(&consumed, node, instr_name);
            }
        }
        checker.mark_unreachable();
        return;
    }

    // Validate bare select operand types before popping
    if instr_name == "select" && get_select_result_type(node, source).is_none() {
        // Bare select (no type annotation): first two operands must be same numeric type
        // Stack: [val1, val2, i32_cond] — peek(0)=cond, peek(1)=val2, peek(2)=val1
        let val2 = checker.peek(1).copied();
        let val1 = checker.peek(2).copied();
        let is_numeric = |t: &ValueType| {
            matches!(
                t,
                ValueType::I32
                    | ValueType::I64
                    | ValueType::F32
                    | ValueType::F64
                    | ValueType::V128
                    | ValueType::Unknown
            )
        };
        // Determine result type from peeked operands
        let result_type = match (&val1, &val2) {
            (Some(v1), Some(v2)) => {
                if !is_numeric(v1) || !is_numeric(v2) {
                    checker.diagnostics.push(
                        Diagnostic::error(
                            node_to_range(node),
                            "type mismatch: select without type annotation requires numeric operands"
                                .to_string(),
                        )
                        .with_code("type-mismatch"),
                    );
                    ValueType::Unknown
                } else if *v1 != ValueType::Unknown
                    && *v2 != ValueType::Unknown
                    && !types_compatible(v1, v2)
                {
                    checker.diagnostics.push(
                        Diagnostic::error(
                            node_to_range(node),
                            format!(
                                "type mismatch: select operands have different types ({:?} vs {:?})",
                                v1, v2
                            ),
                        )
                        .with_code("type-mismatch"),
                    );
                    ValueType::Unknown
                } else if *v1 != ValueType::Unknown {
                    *v1
                } else {
                    *v2
                }
            }
            (None, Some(v2)) => {
                // Only val2 available (val1 is in unreachable bottom).
                if !is_numeric(v2) {
                    checker.diagnostics.push(
                        Diagnostic::error(
                            node_to_range(node),
                            "type mismatch: select without type annotation requires numeric operands"
                                .to_string(),
                        )
                        .with_code("type-mismatch"),
                    );
                }
                *v2
            }
            _ => ValueType::Unknown,
        };
        // Pop: i32 condition + two operands (typed as Unknown to avoid double-reporting)
        checker.pop_vals_for_instr(
            &[ValueType::Unknown, ValueType::Unknown, ValueType::I32],
            node,
            instr_name,
        );
        // Push the actual result type (not Unknown)
        checker.push_val(result_type);
        return;
    }

    if instr_name == "ref.as_non_null" {
        // ref.as_non_null : [(ref null ht)] -> [(ref ht)]
        let ref_type = checker.pop_val().unwrap_or(ValueType::Unknown);
        checker.push_val(make_non_nullable(&ref_type));
        return;
    }

    // Regular instructions: consume typed operands, produce typed results
    let consumed = get_instruction_consumed_types(node, instr_name, symbols, source);
    if !consumed.is_empty() {
        checker.pop_vals_for_instr(&consumed, node, instr_name);
    }

    // Check global.set targets a mutable global
    if instr_name == "global.set" {
        if let Some(index) = get_index_from_node(node, source) {
            let is_immutable = if let Some(global) = symbols.get_global_by_name(index) {
                !global.is_mutable
            } else if let Ok(idx) = index.parse::<usize>() {
                symbols
                    .get_global_by_index(idx)
                    .is_some_and(|g| !g.is_mutable)
            } else {
                false
            };
            if is_immutable {
                checker.diagnostics.push(
                    Diagnostic::error(node_to_range(node), "global is immutable")
                        .with_code("immutable-global"),
                );
            }
        }
    }

    // Check table.init/table.copy type compatibility
    if instr_name == "table.init" {
        check_table_init_types(node, symbols, source, checker);
    } else if instr_name == "table.copy" {
        check_table_copy_types(node, symbols, source, checker);
    }

    // Check array mutation/bulk instructions for mutability and type compatibility
    check_array_operations(instr_name, node, symbols, source, checker);

    let produced = infer_instruction_result_types(instr_name, node, symbols, source);
    // If we know the instruction produces N values but couldn't infer types, pad with Unknown
    let produces_count = get_instruction_produces_count(instr_name);
    let mut types = produced;
    while types.len() < produces_count {
        types.push(ValueType::Unknown);
    }
    checker.push_vals(&types);
}

/// Get the number of values an instruction produces
fn get_instruction_produces_count(instr_name: &str) -> usize {
    if let Some(arity) = lookup_instruction_arity(instr_name) {
        arity.produces
    } else if let Some((_c, p)) = infer_simd_instruction_arity(instr_name) {
        p
    } else {
        0
    }
}

/// Get the typed operands that an instruction consumes from the stack.
/// This is the key addition over the old untyped StackState approach.
fn get_instruction_consumed_types(
    node: &Node,
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    // Try pattern-based type derivation first
    if let Some(types) = derive_consumed_types_from_name(instr_name, node, symbols, source) {
        return types;
    }

    // Fall back to untyped count from arity map (use Unknown for each operand)
    let count = if let Some(arity) = lookup_instruction_arity(instr_name) {
        match arity.operand_mode {
            OperandMode::Fixed(n) => n,
            OperandMode::Dynamic => get_dynamic_operand_count(node, instr_name, symbols, source),
        }
    } else if let Some((c, _p)) = infer_simd_instruction_arity(instr_name) {
        c
    } else {
        return vec![];
    };

    vec![ValueType::Unknown; count]
}

/// Derive typed consumed operands from instruction name pattern.
/// Returns None if we can't derive types (fall back to untyped count).
fn derive_consumed_types_from_name(
    instr_name: &str,
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Option<Vec<ValueType>> {
    // Helper to check if an instruction is a scalar binary op (2 operands of prefix type)
    let is_binary_scalar = |name: &str| -> bool {
        if is_simd_instruction(name) {
            return false;
        }
        // Arithmetic: add, sub, mul, div, rem, and, or, xor, shl, shr, rot, copysign, min, max
        name.ends_with(".add") || name.ends_with(".sub") || name.ends_with(".mul")
            || name.ends_with(".div_s") || name.ends_with(".div_u")
            || name.ends_with(".rem_s") || name.ends_with(".rem_u")
            || name.ends_with(".and") || name.ends_with(".or") || name.ends_with(".xor")
            || name.ends_with(".shl") || name.ends_with(".shr_s") || name.ends_with(".shr_u")
            || name.ends_with(".rotl") || name.ends_with(".rotr")
            || name.ends_with(".copysign") || name.ends_with(".min") || name.ends_with(".max")
            || name.ends_with(".div")
            // Comparisons
            || name.ends_with(".eq") || name.ends_with(".ne")
            || name.ends_with(".lt_s") || name.ends_with(".lt_u")
            || name.ends_with(".gt_s") || name.ends_with(".gt_u")
            || name.ends_with(".le_s") || name.ends_with(".le_u")
            || name.ends_with(".ge_s") || name.ends_with(".ge_u")
            || name.ends_with(".lt") || name.ends_with(".gt")
            || name.ends_with(".le") || name.ends_with(".ge")
    };

    // Helper to check if an instruction is a scalar unary op
    let is_unary_scalar = |name: &str| -> bool {
        if is_simd_instruction(name) {
            return false;
        }
        name.ends_with(".eqz")
            || name.ends_with(".clz")
            || name.ends_with(".ctz")
            || name.ends_with(".popcnt")
            || name.ends_with(".abs")
            || name.ends_with(".neg")
            || name.ends_with(".ceil")
            || name.ends_with(".floor")
            || name.ends_with(".trunc")
            || name.ends_with(".nearest")
            || name.ends_with(".sqrt")
            || name.ends_with(".extend8_s")
            || name.ends_with(".extend16_s")
            || name.ends_with(".extend32_s")
    };

    match instr_name {
        // Constants — consume nothing
        "i32.const" | "i64.const" | "f32.const" | "f64.const" | "v128.const" => Some(vec![]),

        // Nop, unreachable — consume nothing
        "nop" | "unreachable" => Some(vec![]),

        // Drop — consume any one value
        "drop" => Some(vec![ValueType::Unknown]),

        // Select — 2 same-type values + i32 condition
        // Typed select: (select (result T)) uses declared type for operands
        "select" => {
            let ty = get_select_result_type(node, source).unwrap_or(ValueType::Unknown);
            Some(vec![ty, ty, ValueType::I32])
        }

        // Local/global get — consume nothing
        "local.get" | "global.get" => Some(vec![]),

        // Local set — consume the local's type
        "local.set" => {
            let ty = get_local_type_from_node(node, symbols, source).unwrap_or(ValueType::Unknown);
            Some(vec![ty])
        }

        // Local tee — consume and re-produce the local's type
        "local.tee" => {
            let ty = get_local_type_from_node(node, symbols, source).unwrap_or(ValueType::Unknown);
            Some(vec![ty])
        }

        // Global set — consume the global's type
        "global.set" => {
            let ty = get_global_type_from_node(node, symbols, source).unwrap_or(ValueType::Unknown);
            Some(vec![ty])
        }

        // Call — consume parameter types
        "call" | "return_call" => {
            if let Some(func_ref) = get_index_from_node(node, source) {
                let func = symbols.get_function_by_name(func_ref).or_else(|| {
                    func_ref
                        .parse::<usize>()
                        .ok()
                        .and_then(|idx| symbols.get_function_by_index(idx))
                });
                if let Some(f) = func {
                    return Some(f.parameters.iter().map(|p| p.param_type).collect());
                }
            }
            None // fall back to untyped
        }

        // Call_ref — consume param types + typed funcref (ref null $type)
        "call_ref" | "return_call_ref" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(td) = resolve_type_ref(type_ref, symbols) {
                    if let TypeKind::Func { params, .. } = &td.kind {
                        let mut types = Vec::with_capacity(params.len() + 1);
                        types.extend_from_slice(params);
                        types.push(ValueType::RefNull(td.index as u32));
                        return Some(types);
                    }
                }
            }
            None
        }

        // Conversions — consume source type
        "i32.wrap_i64" => Some(vec![ValueType::I64]),
        "i64.extend_i32_s" | "i64.extend_i32_u" => Some(vec![ValueType::I32]),
        "i32.trunc_f32_s" | "i32.trunc_f32_u" | "i32.trunc_sat_f32_s" | "i32.trunc_sat_f32_u" => {
            Some(vec![ValueType::F32])
        }
        "i32.trunc_f64_s" | "i32.trunc_f64_u" | "i32.trunc_sat_f64_s" | "i32.trunc_sat_f64_u" => {
            Some(vec![ValueType::F64])
        }
        "i64.trunc_f32_s" | "i64.trunc_f32_u" | "i64.trunc_sat_f32_s" | "i64.trunc_sat_f32_u" => {
            Some(vec![ValueType::F32])
        }
        "i64.trunc_f64_s" | "i64.trunc_f64_u" | "i64.trunc_sat_f64_s" | "i64.trunc_sat_f64_u" => {
            Some(vec![ValueType::F64])
        }
        "f32.convert_i32_s" | "f32.convert_i32_u" => Some(vec![ValueType::I32]),
        "f32.convert_i64_s" | "f32.convert_i64_u" => Some(vec![ValueType::I64]),
        "f64.convert_i32_s" | "f64.convert_i32_u" => Some(vec![ValueType::I32]),
        "f64.convert_i64_s" | "f64.convert_i64_u" => Some(vec![ValueType::I64]),
        "f32.demote_f64" => Some(vec![ValueType::F64]),
        "f64.promote_f32" => Some(vec![ValueType::F32]),
        "i32.reinterpret_f32" => Some(vec![ValueType::F32]),
        "i64.reinterpret_f64" => Some(vec![ValueType::F64]),
        "f32.reinterpret_i32" => Some(vec![ValueType::I32]),
        "f64.reinterpret_i64" => Some(vec![ValueType::I64]),

        // ---- SIMD instruction type signatures ----
        // v128 bitwise: (v128, v128) -> v128
        "v128.and" | "v128.or" | "v128.xor" | "v128.andnot" => {
            Some(vec![ValueType::V128, ValueType::V128])
        }
        // v128 bitselect: (v128, v128, v128) -> v128
        "v128.bitselect" => Some(vec![ValueType::V128, ValueType::V128, ValueType::V128]),
        // v128 not: (v128) -> v128
        "v128.not" => Some(vec![ValueType::V128]),
        // v128.any_true: (v128) -> i32
        "v128.any_true" => Some(vec![ValueType::V128]),

        // SIMD lane instructions
        name if is_simd_instruction(name) => derive_simd_consumed_types(name),

        // SIMD lane load/store — consume address + v128 vector
        name if name.contains("_lane") && name.starts_with("v128.load") => {
            let addr_type = get_memory_address_type(node, symbols, source);
            Some(vec![addr_type, ValueType::V128])
        }
        name if name.contains("_lane") && name.starts_with("v128.store") => {
            let addr_type = get_memory_address_type(node, symbols, source);
            Some(vec![addr_type, ValueType::V128])
        }

        // Memory load — consume address (i32 for memory32, i64 for memory64)
        name if name.contains(".load") => {
            let addr_type = get_memory_address_type(node, symbols, source);
            Some(vec![addr_type])
        }

        // Memory store — consume address + value
        name if name.contains(".store") => {
            let addr_type = get_memory_address_type(node, symbols, source);
            let value_type = type_from_prefix(name).unwrap_or(ValueType::Unknown);
            Some(vec![addr_type, value_type])
        }

        // Memory size — no operands
        "memory.size" => Some(vec![]),
        // Memory grow — consume delta (i32 for memory32, i64 for memory64)
        "memory.grow" => {
            let addr_type = get_memory_address_type(node, symbols, source);
            Some(vec![addr_type])
        }

        // Reference instructions
        "ref.null" => Some(vec![]),
        "ref.func" => Some(vec![]),
        "ref.is_null" => Some(vec![ValueType::Unknown]), // any ref
        // ref.as_non_null handled specially in process_instruction
        "ref.eq" => Some(vec![ValueType::Eqref, ValueType::Eqref]),

        // GC instructions
        "ref.i31" => Some(vec![ValueType::I32]),
        "i31.get_s" | "i31.get_u" => Some(vec![ValueType::I31ref]),
        "any.convert_extern" => Some(vec![ValueType::Externref]),
        "extern.convert_any" => Some(vec![ValueType::Anyref]),

        // Struct operations
        "struct.get" | "struct.get_s" | "struct.get_u" => Some(vec![ValueType::Unknown]), // structref
        "struct.set" => Some(vec![ValueType::Unknown, ValueType::Unknown]), // structref + value
        "struct.new_default" => Some(vec![]),

        // Array operations
        "array.new" => Some(vec![ValueType::Unknown, ValueType::I32]), // value, length
        "array.new_default" => Some(vec![ValueType::I32]),             // length
        "array.get" | "array.get_s" | "array.get_u" => {
            Some(vec![ValueType::Unknown, ValueType::I32])
        } // arrayref, index
        "array.set" => Some(vec![ValueType::Unknown, ValueType::I32, ValueType::Unknown]), // arrayref, index, value
        "array.len" => Some(vec![ValueType::Unknown]), // arrayref
        "array.fill" => {
            // arrayref, i32 offset, value, i32 length
            // Resolve element type from the type index to type-check the value operand
            let val_ty = resolve_first_type_index(node, symbols, source)
                .and_then(|td| match &td.kind {
                    TypeKind::Array { element_type, .. } => {
                        Some(unpack_storage_type(*element_type))
                    }
                    _ => None,
                })
                .unwrap_or(ValueType::Unknown);
            Some(vec![
                ValueType::Unknown,
                ValueType::I32,
                val_ty,
                ValueType::I32,
            ])
        }
        "array.copy" => Some(vec![
            ValueType::Unknown,
            ValueType::I32,
            ValueType::Unknown,
            ValueType::I32,
            ValueType::I32,
        ]),
        "array.new_fixed" => {
            // array.new_fixed $type N: consumes N elements of the array's element type
            let elem_ty = resolve_first_type_index(node, symbols, source)
                .and_then(|td| match &td.kind {
                    TypeKind::Array { element_type, .. } => {
                        Some(unpack_storage_type(*element_type))
                    }
                    _ => None,
                })
                .unwrap_or(ValueType::Unknown);
            let count = get_array_new_fixed_count(node, source);
            Some(vec![elem_ty; count])
        }
        "array.new_data" | "array.new_elem" => Some(vec![ValueType::I32, ValueType::I32]), // offset, length
        "array.init_data" | "array.init_elem" => Some(vec![
            ValueType::Unknown,
            ValueType::I32,
            ValueType::I32,
            ValueType::I32,
        ]),

        // Ref test/cast
        "ref.test" | "ref.cast" | "ref.cast_null" => Some(vec![ValueType::Unknown]),
        // br_on_cast/br_on_cast_fail handled as branch instructions above

        // Exceptions
        "throw" => {
            // throw $tag: consumes tag's param types
            if let Some(tag_ref) = get_index_from_node(node, source) {
                let tag = symbols
                    .get_tag_by_name(tag_ref)
                    .or_else(|| parse_wat_nat(tag_ref).and_then(|i| symbols.tags.get(i as usize)));
                if let Some(tag) = tag {
                    return Some(tag.params.clone());
                }
            }
            Some(vec![])
        }
        "throw_ref" => Some(vec![ValueType::Unknown]), // exnref

        // Bulk memory — addresses are i32/i64 depending on memory type
        "memory.copy" => {
            let addr_type = get_memory_address_type(node, symbols, source);
            Some(vec![addr_type, addr_type, addr_type])
        }
        "memory.fill" => {
            let addr_type = get_memory_address_type(node, symbols, source);
            Some(vec![addr_type, ValueType::I32, addr_type])
        }
        "memory.init" => {
            let addr_type = get_memory_address_type(node, symbols, source);
            Some(vec![addr_type, ValueType::I32, ValueType::I32])
        }
        "data.drop" | "elem.drop" => Some(vec![]),

        // Table operations — resolve element type and index type from symbol table
        "table.get" => {
            let idx_type = get_table_index_type(node, symbols, source);
            Some(vec![idx_type])
        }
        "table.set" => {
            let idx_type = get_table_index_type(node, symbols, source);
            let elem_type = get_table_elem_type(node, symbols, source);
            Some(vec![idx_type, elem_type])
        }
        "table.size" => Some(vec![]),
        "table.grow" => {
            let idx_type = get_table_index_type(node, symbols, source);
            let elem_type = get_table_elem_type(node, symbols, source);
            Some(vec![elem_type, idx_type])
        }
        "table.fill" => {
            let idx_type = get_table_index_type(node, symbols, source);
            let elem_type = get_table_elem_type(node, symbols, source);
            Some(vec![idx_type, elem_type, idx_type])
        }
        "table.copy" => {
            // table.copy $dst $src: (d:dst_addr, s:src_addr, n:min(dst_addr, src_addr))
            let (dst_type, src_type) = resolve_table_copy_addr_types(node, symbols, source);
            // n uses i32 unless both tables are i64
            let n_type = if dst_type == ValueType::I64 && src_type == ValueType::I64 {
                ValueType::I64
            } else {
                ValueType::I32
            };
            Some(vec![dst_type, src_type, n_type])
        }
        "table.init" => {
            // table.init $table $elem: (d:table_addr, s:i32, n:i32)
            let table = resolve_table_for_table_init(node, symbols, source);
            let addr_type = table
                .map(|t| {
                    if t.is_table64 {
                        ValueType::I64
                    } else {
                        ValueType::I32
                    }
                })
                .unwrap_or(ValueType::I32);
            Some(vec![addr_type, ValueType::I32, ValueType::I32])
        }

        // Atomic fence — no operands
        "atomic.fence" => Some(vec![]),

        // Wait/notify
        "memory.atomic.wait32" => Some(vec![ValueType::I32, ValueType::I32, ValueType::I64]),
        "memory.atomic.wait64" => Some(vec![ValueType::I32, ValueType::I64, ValueType::I64]),
        "memory.atomic.notify" => Some(vec![ValueType::I32, ValueType::I32]),

        // Atomic RMW — address (i32) + value (prefix type)
        name if name.contains(".atomic.rmw") => {
            if let Some(ty) = type_from_prefix(name) {
                if name.contains("cmpxchg") {
                    Some(vec![ValueType::I32, ty, ty]) // addr + expected + replacement
                } else {
                    Some(vec![ValueType::I32, ty]) // addr + value
                }
            } else {
                None
            }
        }

        // br_on_null/br_on_non_null handled specially in process_instruction

        // Scalar binary operations — 2 operands of prefix type
        name if is_binary_scalar(name) => {
            if let Some(ty) = type_from_prefix(name) {
                Some(vec![ty, ty])
            } else {
                None
            }
        }

        // Scalar unary operations — 1 operand of prefix type
        name if is_unary_scalar(name) => {
            if let Some(ty) = type_from_prefix(name) {
                Some(vec![ty])
            } else {
                None
            }
        }

        _ => None, // fall back to untyped count
    }
}

/// Get consumed types for call_indirect/return_call_indirect
fn get_call_indirect_consumed_types(
    node: &Node,
    _instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    // call_indirect consumes: param_types... + table index (i32 or i64 for table64)
    let table_idx_type = get_table_index_type(node, symbols, source);
    // First, try to resolve from type_use (handles multi-table where first index is table ref)
    if let Some(td) = resolve_call_indirect_type(node, source, symbols) {
        if let TypeKind::Func { params, .. } = &td.kind {
            let mut types = Vec::with_capacity(params.len() + 1);
            types.extend_from_slice(params);
            types.push(table_idx_type);
            return types;
        }
    }
    // Try implicit type resolution (for modules without explicit type declarations)
    if let Some(sig) = resolve_call_indirect_type_implicit(node, source, symbols) {
        let mut types = sig.0;
        types.push(table_idx_type);
        return types;
    }
    // Check for explicit func_type_params_many (inline params without type_use)
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "func_type_params_many" {
            params.extend(parse_func_type_results(&child, source, Some(symbols)));
        }
    }
    params.push(table_idx_type);
    params
}

/// Get branch target depth from a br/br_if instruction node.
/// Emits "unknown label" diagnostic for numeric depths that exceed the control stack.
/// Named label resolution failure is handled by the reference checker.
fn get_branch_depth(node: &Node, source: &str, checker: &mut TypeChecker) -> Option<usize> {
    let index = get_index_from_node(node, source)?;
    // Try as numeric index first (supports hex like 0x10000001 and underscore separators)
    if !index.starts_with('$') {
        if let Some(depth) = parse_wat_nat(index) {
            let depth = depth as usize;
            if depth < checker.ctrl_depth() {
                return Some(depth);
            }
            // Out-of-range label depth
            checker.diagnostics.push(
                Diagnostic::error(node_to_range(node), format!("unknown label {}", depth))
                    .with_code("unknown-label"),
            );
            return None;
        }
    }
    // Try named label resolution
    if index.starts_with('$') {
        return checker.resolve_label_depth(index);
    }
    None
}

/// Get all branch depths from a br_table instruction (last one is the default).
/// Emits "unknown label" diagnostics for out-of-range depths.
fn get_all_branch_depths(node: &Node, source: &str, checker: &mut TypeChecker) -> Vec<usize> {
    let mut indices: Vec<&str> = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "index" || kind == "identifier" || kind == "nat" {
            indices.push(source[child.byte_range()].trim());
        }
        if kind.starts_with("op_") {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "index" || inner_kind == "identifier" || inner_kind == "nat" {
                    indices.push(source[inner_child.byte_range()].trim());
                }
            }
        }
    }

    let ctrl_depth = checker.ctrl_depth();
    let mut result = Vec::new();
    for idx_str in &indices {
        if idx_str.starts_with('$') {
            if let Some(depth) = checker.resolve_label_depth(idx_str) {
                result.push(depth);
            }
        } else if let Some(depth) = parse_wat_nat(idx_str) {
            let depth = depth as usize;
            if depth < ctrl_depth {
                result.push(depth);
            } else {
                checker.diagnostics.push(
                    Diagnostic::error(node_to_range(node), format!("unknown label {}", depth))
                        .with_code("unknown-label"),
                );
            }
        }
    }
    result
}

/// Check if two value types are compatible for br_table target consistency
fn types_compatible(a: &ValueType, b: &ValueType) -> bool {
    if *a == ValueType::Unknown || *b == ValueType::Unknown {
        return true;
    }
    a == b
}

/// Get the result type from a typed select instruction: `(select (result T))`
/// Returns None for untyped select.
fn get_select_result_type(node: &Node, source: &str) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "op_select" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "func_type_results" {
                    // Look for value_type or ref_type children inside (result T)
                    let mut vt_cursor = inner_child.walk();
                    for vt_child in inner_child.children(&mut vt_cursor) {
                        node_kind!(vt_kind = vt_child);

                        if vt_kind == "value_type" || vt_kind == "ref_type" {
                            let text = &source[vt_child.byte_range()];
                            return ValueType::try_parse(text.trim()).or(Some(ValueType::Unknown));
                        }
                    }
                }
            }
        }
    }
    None
}

/// Get the operand count for a dynamic instruction
fn get_dynamic_operand_count(
    node: &Node,
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> usize {
    match instr_name {
        "call" | "return_call" => {
            if let Some(func_ref) = get_index_from_node(node, source) {
                let func = symbols.get_function_by_name(func_ref).or_else(|| {
                    func_ref
                        .parse::<usize>()
                        .ok()
                        .and_then(|idx| symbols.get_function_by_index(idx))
                });
                if let Some(f) = func {
                    return f.parameters.len();
                }
            }
            0
        }
        "call_ref" | "return_call_ref" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(td) = resolve_type_ref(type_ref, symbols) {
                    if let TypeKind::Func { params, .. } = &td.kind {
                        return params.len() + 1;
                    }
                }
            }
            1
        }
        "call_indirect" | "return_call_indirect" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(td) = resolve_type_ref(type_ref, symbols) {
                    if let TypeKind::Func { params, .. } = &td.kind {
                        return params.len() + 1;
                    }
                }
            }
            1
        }
        "struct.new" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(td) = resolve_type_ref(type_ref, symbols) {
                    if let TypeKind::Struct { fields } = &td.kind {
                        return fields.len();
                    }
                }
            }
            0
        }
        "array.new_fixed" => get_array_new_fixed_count(node, source),
        "throw" => {
            if let Some(tag_ref) = get_index_from_node(node, source) {
                let tag = symbols.get_tag_by_name(tag_ref).or_else(|| {
                    tag_ref
                        .parse::<usize>()
                        .ok()
                        .and_then(|idx| symbols.get_tag_by_index(idx))
                });
                if let Some(t) = tag {
                    return t.params.len();
                }
            }
            0
        }
        "br" | "br_if" => {
            if instr_name == "br_if" {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Get the index/identifier from an instruction node
fn get_index_from_node<'a>(node: &Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "index" || kind == "identifier" {
            return Some(source[child.byte_range()].trim());
        }
        if kind == "type_use" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "index" || inner_kind == "identifier" {
                    return Some(source[inner_child.byte_range()].trim());
                }
            }
        }
        if kind.starts_with("op_") {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "index" || inner_kind == "identifier" {
                    return Some(source[inner_child.byte_range()].trim());
                }
            }
        }
    }
    None
}

/// Infer the result type(s) of an instruction from its name and context
fn infer_instruction_result_types(
    instr_name: &str,
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    let skip_early_return = matches!(
        instr_name,
        "call"
            | "call_ref"
            | "return_call"
            | "return_call_ref"
            | "call_indirect"
            | "return_call_indirect"
    );
    if !skip_early_return {
        if let Some(arity) = lookup_instruction_arity(instr_name) {
            if arity.produces == 0 {
                return vec![];
            }
        }
    }

    let is_scalar_comparison = |name: &str| -> bool {
        if is_simd_instruction(name) {
            return false;
        }
        name.ends_with(".eq")
            || name.ends_with(".ne")
            || name.ends_with(".lt_s")
            || name.ends_with(".lt_u")
            || name.ends_with(".gt_s")
            || name.ends_with(".gt_u")
            || name.ends_with(".le_s")
            || name.ends_with(".le_u")
            || name.ends_with(".ge_s")
            || name.ends_with(".ge_u")
            || name.ends_with(".lt")
            || name.ends_with(".gt")
            || name.ends_with(".le")
            || name.ends_with(".ge")
            || name.ends_with(".eqz")
    };

    match instr_name {
        "i32.const" => vec![ValueType::I32],
        "i64.const" => vec![ValueType::I64],
        "f32.const" => vec![ValueType::F32],
        "f64.const" => vec![ValueType::F64],
        "v128.const" => vec![ValueType::V128],

        name if is_scalar_comparison(name) => vec![ValueType::I32],

        "i32.wrap_i64"
        | "i32.trunc_f32_s"
        | "i32.trunc_f32_u"
        | "i32.trunc_f64_s"
        | "i32.trunc_f64_u"
        | "i32.reinterpret_f32"
        | "i32.trunc_sat_f32_s"
        | "i32.trunc_sat_f32_u"
        | "i32.trunc_sat_f64_s"
        | "i32.trunc_sat_f64_u" => {
            vec![ValueType::I32]
        }

        "i64.extend_i32_s"
        | "i64.extend_i32_u"
        | "i64.trunc_f32_s"
        | "i64.trunc_f32_u"
        | "i64.trunc_f64_s"
        | "i64.trunc_f64_u"
        | "i64.reinterpret_f64"
        | "i64.trunc_sat_f32_s"
        | "i64.trunc_sat_f32_u"
        | "i64.trunc_sat_f64_s"
        | "i64.trunc_sat_f64_u"
        | "i64.extend8_s"
        | "i64.extend16_s"
        | "i64.extend32_s" => {
            vec![ValueType::I64]
        }

        "f32.convert_i32_s"
        | "f32.convert_i32_u"
        | "f32.convert_i64_s"
        | "f32.convert_i64_u"
        | "f32.demote_f64"
        | "f32.reinterpret_i32" => vec![ValueType::F32],

        "f64.convert_i32_s"
        | "f64.convert_i32_u"
        | "f64.convert_i64_s"
        | "f64.convert_i64_u"
        | "f64.promote_f32"
        | "f64.reinterpret_i64" => vec![ValueType::F64],

        "i32.extend8_s" | "i32.extend16_s" => vec![ValueType::I32],

        name if name.contains(".load") => {
            type_from_prefix(name).map(|t| vec![t]).unwrap_or_default()
        }
        name if name.contains(".store") => vec![],

        "memory.size" | "memory.grow" => {
            vec![get_memory_address_type(node, symbols, source)]
        }

        "local.get" | "local.tee" => get_local_type_from_node(node, symbols, source)
            .map(|t| vec![t])
            .unwrap_or_else(|| vec![ValueType::Unknown]),

        "global.get" => get_global_type_from_node(node, symbols, source)
            .map(|t| vec![t])
            .unwrap_or_else(|| vec![ValueType::Unknown]),

        "call" | "return_call" => get_call_result_types(node, symbols, source),
        "call_ref" | "return_call_ref" => get_call_ref_result_types(node, symbols, source),
        "call_indirect" | "return_call_indirect" => {
            get_call_indirect_result_types(node, symbols, source)
        }

        "table.get" => vec![get_table_elem_type(node, symbols, source)],
        "table.grow" | "table.size" => vec![get_table_index_type(node, symbols, source)],

        "drop" | "local.set" | "global.set" | "return" | "br" | "unreachable" | "nop" => vec![],

        "ref.null" => vec![ValueType::Unknown],
        "ref.func" => get_ref_func_result_type(node, symbols, source),
        "ref.extern" => vec![ValueType::Externref],
        "ref.is_null" => vec![ValueType::I32],

        "select" => {
            vec![get_select_result_type(node, source).unwrap_or(ValueType::Unknown)]
        }
        "br_if" => vec![],

        "v128.any_true" => vec![ValueType::I32],
        name if name.ends_with(".all_true") || name.ends_with(".bitmask") => vec![ValueType::I32],

        name if name.ends_with(".extract_lane_s")
            || name.ends_with(".extract_lane_u")
            || name.ends_with(".extract_lane") =>
        {
            if name.starts_with("i8x") || name.starts_with("i16x") || name.starts_with("i32x") {
                vec![ValueType::I32]
            } else if name.starts_with("i64x") {
                vec![ValueType::I64]
            } else if name.starts_with("f32x") {
                vec![ValueType::F32]
            } else if name.starts_with("f64x") {
                vec![ValueType::F64]
            } else {
                vec![ValueType::Unknown]
            }
        }

        "struct.get" | "struct.get_s" | "struct.get_u" => {
            get_struct_field_result_type(node, symbols, source)
        }

        "struct.new" | "struct.new_default" | "array.new" | "array.new_default"
        | "array.new_fixed" | "array.new_data" | "array.new_elem" => {
            resolve_first_type_index(node, symbols, source)
                .map(|td| vec![ValueType::Ref(td.index as u32)])
                .unwrap_or_else(|| vec![ValueType::Unknown])
        }

        name if type_from_prefix(name).is_some() => {
            type_from_prefix(name).map(|t| vec![t]).unwrap_or_default()
        }

        _ => vec![ValueType::Unknown],
    }
}

/// Get the type of a local variable from an instruction node
fn get_local_type_from_node(node: &Node, symbols: &SymbolTable, source: &str) -> Option<ValueType> {
    let index = get_index_from_node(node, source)?;
    let func_line = node.start_position().row as u32;
    let func = symbols.find_function_containing_line(func_line)?;

    let name_to_check = index.strip_prefix('$').unwrap_or(index);

    for param in &func.parameters {
        if let Some(param_name) = &param.name {
            let param_name_stripped = param_name.strip_prefix('$').unwrap_or(param_name);
            if param_name_stripped == name_to_check || param_name == index {
                return Some(param.param_type);
            }
        }
    }

    for local in &func.locals {
        if let Some(local_name) = &local.name {
            let local_name_stripped = local_name.strip_prefix('$').unwrap_or(local_name);
            if local_name_stripped == name_to_check || local_name == index {
                return Some(local.var_type);
            }
        }
    }

    if let Ok(idx) = index.parse::<usize>() {
        if idx < func.parameters.len() {
            return Some(func.parameters[idx].param_type);
        }
        let local_idx = idx - func.parameters.len();
        if local_idx < func.locals.len() {
            return Some(func.locals[local_idx].var_type);
        }
    }

    None
}

/// Get the type of a global from an instruction node
fn get_global_type_from_node(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Option<ValueType> {
    let index = get_index_from_node(node, source)?;

    if let Some(global) = symbols.get_global_by_name(index) {
        return Some(global.var_type);
    }
    if let Ok(idx) = index.parse::<usize>() {
        if let Some(global) = symbols.get_global_by_index(idx) {
            return Some(global.var_type);
        }
    }
    None
}

/// Get the element type of the table referenced by a table instruction node.
/// Falls back to table 0 if no explicit index.
fn resolve_table<'a>(node: &Node, symbols: &'a SymbolTable, source: &str) -> Option<&'a Table> {
    if let Some(index) = get_index_from_node(node, source) {
        if let Some(table) = symbols.get_table_by_name(index) {
            return Some(table);
        }
        if let Some(idx) = parse_wat_nat(index) {
            if let Some(table) = symbols.tables.get(idx as usize) {
                return Some(table);
            }
        }
    }
    symbols.tables.first()
}

fn get_table_elem_type(node: &Node, symbols: &SymbolTable, source: &str) -> ValueType {
    resolve_table(node, symbols, source)
        .map(|t| t.ref_type)
        .unwrap_or(ValueType::Funcref)
}

/// Get the index type for table operations (i32 for regular tables, i64 for table64)
fn get_table_index_type(node: &Node, symbols: &SymbolTable, source: &str) -> ValueType {
    resolve_table(node, symbols, source)
        .map(|t| {
            if t.is_table64 {
                ValueType::I64
            } else {
                ValueType::I32
            }
        })
        .unwrap_or(ValueType::I32)
}

/// Resolve the memory referenced by a memory instruction node.
/// Checks for a memory index/identifier in the instruction, falls back to memory 0.
fn resolve_memory<'a>(node: &Node, symbols: &'a SymbolTable, source: &str) -> Option<&'a Memory> {
    if let Some(index) = get_index_from_node(node, source) {
        if let Some(mem) = symbols.get_memory_by_name(index) {
            return Some(mem);
        }
        if let Some(idx) = parse_wat_nat(index) {
            if let Some(mem) = symbols.memories.get(idx as usize) {
                return Some(mem);
            }
        }
    }
    symbols.memories.first()
}

/// Get the address type for memory operations (i32 for memory32, i64 for memory64)
fn get_memory_address_type(node: &Node, symbols: &SymbolTable, source: &str) -> ValueType {
    resolve_memory(node, symbols, source)
        .map(|m| {
            if m.is_memory64 {
                ValueType::I64
            } else {
                ValueType::I32
            }
        })
        .unwrap_or(ValueType::I32)
}

/// Get result type for ref.func instruction.
/// Returns Ref(type_index) when the function's type can be resolved, otherwise Funcref.
fn get_ref_func_result_type(node: &Node, symbols: &SymbolTable, source: &str) -> Vec<ValueType> {
    if let Some(func_ref) = get_index_from_node(node, source) {
        // Look up the function to find its type
        let func = if let Some(f) = symbols.get_function_by_name(func_ref) {
            Some(f)
        } else if let Ok(idx) = func_ref.parse::<usize>() {
            symbols.get_function_by_index(idx)
        } else {
            None
        };

        if let Some(func) = func {
            // Use pre-resolved type_index when available
            if let Some(tidx) = func.type_index {
                return vec![ValueType::Ref(tidx as u32)];
            }
        }
    }
    vec![ValueType::Funcref]
}

/// Get result types for a call instruction
fn get_call_result_types(node: &Node, symbols: &SymbolTable, source: &str) -> Vec<ValueType> {
    if let Some(func_ref) = get_index_from_node(node, source) {
        let func = symbols.get_function_by_name(func_ref).or_else(|| {
            func_ref
                .parse::<usize>()
                .ok()
                .and_then(|idx| symbols.get_function_by_index(idx))
        });
        if let Some(f) = func {
            return f.results.clone();
        }
    }
    vec![]
}

/// Get result types for a call_ref instruction
fn get_call_ref_result_types(node: &Node, symbols: &SymbolTable, source: &str) -> Vec<ValueType> {
    if let Some(type_ref) = get_index_from_node(node, source) {
        if let Some(td) = resolve_type_ref(type_ref, symbols) {
            if let TypeKind::Func { results, .. } = &td.kind {
                return results.clone();
            }
        }
    }
    vec![]
}

/// Resolve the function type for a call_indirect instruction.
/// Handles both multi-table (index + type_use) and single-table (just type_use or bare index) forms.
fn resolve_call_indirect_type<'a>(
    node: &Node,
    source: &str,
    symbols: &'a SymbolTable,
) -> Option<&'a TypeDef> {
    let mut cursor = node.walk();
    let mut first_index: Option<&str> = None;
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        // type_use is authoritative — use it directly
        if kind == "type_use" {
            return resolve_type_use_node(&child, source, symbols);
        }
        // Track first index (could be table ref in multi-table or type ref in single-table)
        if kind == "index" && first_index.is_none() {
            first_index = Some(source[child.byte_range()].trim());
        }
    }
    // No type_use found — try first index as a type reference (single-table mode)
    if let Some(idx_text) = first_index {
        return resolve_type_ref(idx_text, symbols);
    }
    None
}

/// Get result types for a call_indirect instruction
fn get_call_indirect_result_types(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    // Try to resolve from type_use (preferred — handles multi-table correctly)
    if let Some(td) = resolve_call_indirect_type(node, source, symbols) {
        if let TypeKind::Func { results: res, .. } = &td.kind {
            return res.clone();
        }
    }
    // Try implicit type resolution
    if let Some(sig) = resolve_call_indirect_type_implicit(node, source, symbols) {
        return sig.1;
    }
    // Check for explicit func_type_results (inline result types without type_use)
    let mut results = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "func_type_results" {
            results.extend(parse_func_type_results(&child, source, Some(symbols)));
        }
    }
    results
}

/// Try to resolve call_indirect type via implicit function types.
/// Falls back to implicit types when explicit type lookup fails.
fn resolve_call_indirect_type_implicit(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Option<(Vec<ValueType>, Vec<ValueType>)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "type_use" {
            // Extract index from type_use
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                node_kind!(ik = inner);
                if ik == "index" || ik == "identifier" {
                    let idx_text = source[inner.byte_range()].trim();
                    if let Ok(idx) = idx_text.parse::<usize>() {
                        return resolve_type_index_to_func_sig(idx, symbols);
                    }
                }
            }
        }
    }
    None
}

/// Find the instr_plain node inside a folded expression (for branch depth resolution).
/// Note: separate native/WASM signatures required due to Node<'a> lifetime differences.
macro_rules! find_instr_plain_in_expr_body {
    ($expr:ident) => {{
        let mut cursor = $expr.walk();
        for child in $expr.children(&mut cursor) {
            node_kind!(kind = child);
            if kind == "expr1" || kind.starts_with("expr1_") {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    node_kind!(ik = inner);
                    if ik == "instr_plain" {
                        return Some(node_copy!(&inner));
                    }
                    if ik.starts_with("expr1_") {
                        let mut deep_cursor = inner.walk();
                        for deep in inner.children(&mut deep_cursor) {
                            node_kind!(dk = deep);
                            if dk == "instr_plain" {
                                return Some(node_copy!(&deep));
                            }
                        }
                    }
                }
            }
        }
        None
    }};
}
#[cfg(feature = "native")]
fn find_instr_plain_in_expr<'a>(expr: &'a Node) -> Option<Node<'a>> {
    find_instr_plain_in_expr_body!(expr)
}
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_instr_plain_in_expr(expr: &Node) -> Option<Node> {
    find_instr_plain_in_expr_body!(expr)
}

/// Get instruction info from a folded expression
/// Returns (instruction_name, explicit_operand_count)
fn get_folded_expr_info<'a>(expr: &Node, source: &'a str) -> Option<(&'a str, usize)> {
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "expr1" {
            return get_expr1_wrapper_info(&child, source);
        } else if kind.starts_with("expr1_") {
            return get_expr1_info(&child, source);
        }
    }
    None
}

/// Get instruction info from an expr1 wrapper node
fn get_expr1_wrapper_info<'a>(expr1: &Node, source: &'a str) -> Option<(&'a str, usize)> {
    let mut cursor = expr1.walk();
    for child in expr1.children(&mut cursor) {
        node_kind!(kind = child);

        if kind.starts_with("expr1_") {
            return get_expr1_info(&child, source);
        }
    }
    None
}

/// Get instruction info from an expr1_* node
fn get_expr1_info<'a>(expr1: &Node, source: &'a str) -> Option<(&'a str, usize)> {
    let mut cursor = expr1.walk();
    let mut explicit_operands = 0usize;
    let mut instr_name: Option<&'a str> = None;

    for child in expr1.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "expr" {
            explicit_operands += 1;
        } else if instr_name.is_none() {
            if kind == "instr_plain" {
                instr_name = get_instruction_name(&child, source);
            } else if kind == "call_indirect" {
                instr_name = Some("call_indirect");
            } else if kind == "return_call_indirect" {
                instr_name = Some("return_call_indirect");
            } else if kind == "instr_call" {
                let text = &source[child.byte_range()];
                let first_token = text.split_whitespace().next().unwrap_or("");
                if !first_token.is_empty() {
                    instr_name = Some(first_token);
                }
            }
        }
    }
    instr_name.map(|name| (name, explicit_operands))
}

/// Get the expected operand count for an instruction (fixed or dynamic)
/// Check if a folded expression is a block-type (block, loop, if, try_table).
/// Returns the expr1_* node and its block kind if found.
macro_rules! find_folded_block_child_body {
    ($expr:ident) => {{
        let mut cursor = $expr.walk();
        for child in $expr.children(&mut cursor) {
            node_kind!(kind = child);
            let block_kind = match kind.as_ref() {
                "expr1_block" => Some("block"),
                "expr1_loop" => Some("loop"),
                "expr1_if" => Some("if"),
                "expr1_try_table" => Some("try_table"),
                _ => None,
            };
            if let Some(bk) = block_kind {
                return Some((node_copy!(&child), bk));
            }
            if kind == "expr1" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    node_kind!(ik = inner);
                    let inner_block_kind = match ik.as_ref() {
                        "expr1_block" => Some("block"),
                        "expr1_loop" => Some("loop"),
                        "expr1_if" => Some("if"),
                        "expr1_try_table" => Some("try_table"),
                        _ => None,
                    };
                    if let Some(bk) = inner_block_kind {
                        return Some((node_copy!(&inner), bk));
                    }
                }
            }
        }
        None
    }};
}
#[cfg(feature = "native")]
fn find_folded_block_child<'a>(expr: &'a Node) -> Option<(Node<'a>, &'static str)> {
    find_folded_block_child_body!(expr)
}
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_folded_block_child(expr: &Node) -> Option<(Node, &'static str)> {
    find_folded_block_child_body!(expr)
}

/// Process a folded block expression (block, loop, if, try_table) with control frames.
fn process_folded_block_expr(
    expr: &Node,
    block_node: &Node,
    block_kind: &str,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    let opcode = match block_kind {
        "loop" => CtrlOpcode::Loop,
        "if" => CtrlOpcode::If,
        "try_table" => CtrlOpcode::TryTable,
        _ => CtrlOpcode::Block,
    };

    let label = get_block_label(block_node, source);
    let params = get_block_param_types(block_node, source, symbols);
    let results = get_block_result_types(block_node, source, symbols);

    // For folded if, the condition sub-expressions are inside if_block.
    // Process them on the outer stack first, then pop the i32 condition.
    if opcode == CtrlOpcode::If {
        process_folded_if_conditions(block_node, source, symbols, checker);
        checker.pop_expect(&ValueType::I32, expr);

        if !params.is_empty() {
            checker.pop_vals_for_instr(&params, expr, "if");
        }

        checker.push_ctrl_labeled(CtrlOpcode::If, params, results, label);

        // Process only the then/else bodies (not condition exprs again)
        process_folded_if_bodies(block_node, source, symbols, checker);

        checker.pop_ctrl(expr);
        return;
    }

    // Pop param types from outer stack
    if !params.is_empty() {
        checker.pop_vals_for_instr(&params, expr, block_kind);
    }

    checker.push_ctrl_labeled(opcode, params, results, label);

    // Process body - find instr_list or direct instruction children
    process_folded_block_body(block_node, source, symbols, checker);

    // Pop control frame - validates end_types and detects excess values
    checker.pop_ctrl(expr);
}

/// Process the body of a folded block expression by finding and processing child nodes.
fn process_folded_block_body(
    block_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        node_kind!(kind = child);

        match kind.as_ref() {
            "instr_list" => {
                // Process all instructions in the list
                let mut list_cursor = child.walk();
                for list_child in child.children(&mut list_cursor) {
                    process_folded_block_child(&list_child, source, symbols, checker);
                }
            }
            "if_block" => {
                // For folded if inside a block body (shouldn't normally occur here since
                // folded ifs are handled by process_folded_block_expr, but handle as fallback)
                process_folded_if_bodies_from_if_block(&child, source, symbols, checker);
            }
            "catch_clause" => {
                // Validate catch clause types against the target label's expected types.
                check_catch_clause_types(&child, source, symbols, checker);
            }
            // Direct instruction children (body without instr_list wrapper)
            "instr" | "instr_plain" | "expr" | "instr_block" | "instr_loop" | "instr_if"
            | "instr_call" | "instr_list_call" => {
                process_folded_block_child(&child, source, symbols, checker);
            }
            _ => {}
        }
    }
}

/// Process a single child node in a folded block body.
fn process_folded_block_child(
    child: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    node_kind!(kind = child);

    match kind.as_ref() {
        "instr" => {
            process_instr_node(child, source, symbols, checker);
        }
        "instr_plain" => {
            if let Some(instr_name) = get_instruction_name(child, source) {
                process_instruction(child, instr_name, checker, symbols, source);
            }
        }
        "expr" => {
            process_folded_expr(child, source, symbols, checker);
        }
        "instr_block" | "instr_loop" => {
            process_block_node(child, source, symbols, checker);
        }
        "instr_if" => {
            process_if_node(child, source, symbols, checker);
        }
        "instr_call" => {
            process_instr_call_node(child, source, symbols, checker);
        }
        "instr_list_call" => {
            process_call_indirect_node(child, source, symbols, checker);
        }
        _ => {}
    }
}

/// Process an if_block directly (conditions + bodies), used as fallback in block body processing.
fn process_folded_if_bodies_from_if_block(
    if_block: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    let mut cursor = if_block.walk();
    for child in if_block.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "instr_list" {
            let mut list_cursor = child.walk();
            for list_child in child.children(&mut list_cursor) {
                process_folded_block_child(&list_child, source, symbols, checker);
            }
        }
    }
}

/// Process the condition sub-expressions of a folded if on the OUTER stack.
/// These are the `expr` children inside the `if_block` that appear before `(then ...)`.
fn process_folded_if_conditions(
    block_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    // Find the if_block child of expr1_if
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "if_block" {
            let mut if_cursor = child.walk();
            for if_child in child.children(&mut if_cursor) {
                node_kind!(ik = if_child);

                // Process condition expr children on the outer stack
                if ik == "expr" {
                    process_folded_expr(&if_child, source, symbols, checker);
                }
            }
        }
    }
}

/// Process only the then/else bodies of a folded if (not the condition exprs).
/// Uses else_transition to properly reset the stack between then and else branches.
fn process_folded_if_bodies(
    block_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    // Find the if_block child of expr1_if
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "if_block" {
            let mut found_then = false;
            let mut if_cursor = child.walk();
            for if_child in child.children(&mut if_cursor) {
                node_kind!(ik = if_child);

                if ik == "instr_list" {
                    if !found_then {
                        // Process then body
                        let mut list_cursor = if_child.walk();
                        for list_child in if_child.children(&mut list_cursor) {
                            process_folded_block_child(&list_child, source, symbols, checker);
                        }
                        found_then = true;
                        // Perform else transition: validate then, reset stack for else
                        checker.else_transition(&if_child);
                    } else {
                        // Process else body
                        let mut list_cursor = if_child.walk();
                        for list_child in if_child.children(&mut list_cursor) {
                            process_folded_block_child(&list_child, source, symbols, checker);
                        }
                    }
                }
            }
        }
    }
}

/// Process sub-expression children of a folded expression.
/// Finds the expr1_* node and recursively processes each `expr` child,
/// pushing their typed results onto the type checker's stack.
fn process_folded_sub_exprs(
    expr: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    let child_count = expr.child_count();
    for i in 0..child_count {
        if let Some(child) = expr.child(i) {
            node_kind!(kind = child);

            // Navigate to the expr1_* node
            let found = if kind == "expr1" {
                // Find inner expr1_* wrapper
                let inner_count = child.child_count();
                let mut inner_found = false;
                for j in 0..inner_count {
                    if let Some(inner) = child.child(j) {
                        if inner.kind().starts_with("expr1_") {
                            // Process expr children of the inner expr1_* node
                            let sub_count = inner.child_count();
                            for k in 0..sub_count {
                                if let Some(sub) = inner.child(k) {
                                    if sub.kind() == "expr" {
                                        process_folded_expr(&sub, source, symbols, checker);
                                    }
                                }
                            }
                            inner_found = true;
                            break;
                        }
                    }
                }
                if !inner_found {
                    // Process expr children of expr1 directly
                    for j in 0..inner_count {
                        if let Some(inner) = child.child(j) {
                            if inner.kind() == "expr" {
                                process_folded_expr(&inner, source, symbols, checker);
                            }
                        }
                    }
                }
                true
            } else if kind.starts_with("expr1_") {
                // Process expr children directly
                let sub_count = child.child_count();
                for j in 0..sub_count {
                    if let Some(sub) = child.child(j) {
                        if sub.kind() == "expr" {
                            process_folded_expr(&sub, source, symbols, checker);
                        }
                    }
                }
                true
            } else {
                false
            };
            if found {
                return;
            }
        }
    }
}

/// Process a folded expression: recursively process sub-expressions, then
/// type-check the instruction's operands against the values on the stack.
fn process_folded_expr(
    expr: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    // Check if this is a block-type expression (block, loop, if, try_table)
    // These need control frame tracking for proper stack validation.
    if let Some((block_node, block_kind)) = find_folded_block_child(expr) {
        process_folded_block_expr(expr, &block_node, block_kind, source, symbols, checker);
        return;
    }

    // Step 1: Recursively process all sub-expression children.
    // This pushes their typed results onto the value stack.
    process_folded_sub_exprs(expr, source, symbols, checker);

    // Step 2: Process the instruction itself with typed operand checking.
    // process_instruction handles all cases: branches, terminators, regular instructions.
    if let Some(instr_node) = find_instr_plain_in_expr(expr) {
        if let Some(instr_name) = get_instruction_name(&instr_node, source) {
            process_instruction(&instr_node, instr_name, checker, symbols, source);
            return;
        }
    }

    // Fallback for call_indirect/return_call_indirect (expr1_call nodes).
    // The grammar uses string literals for "call_indirect", so there's no named
    // call_indirect child — the expr1_call node itself holds type_use/index.
    if let Some((instr_name, _)) = get_folded_expr_info(expr, source) {
        if matches!(instr_name, "call_indirect" | "return_call_indirect") {
            if let Some(expr1_call_node) = find_expr1_call_in_expr(expr) {
                let consumed =
                    get_call_indirect_consumed_types(&expr1_call_node, instr_name, symbols, source);
                if !consumed.is_empty() {
                    checker.pop_vals_for_instr(&consumed, expr, instr_name);
                }

                if is_terminating_instruction(instr_name) {
                    if let Some(diag) =
                        validate_tail_call_in_folded_expr(expr, instr_name, symbols, source)
                    {
                        checker.diagnostics.push(diag);
                    }
                    checker.mark_unreachable();
                    return;
                }

                let result_types =
                    get_call_indirect_result_types(&expr1_call_node, symbols, source);
                checker.push_vals(&result_types);
                return;
            }
        }
    }

    // Check if expression always terminates
    if sequence_always_terminates(expr, source) {
        checker.mark_unreachable();
        return;
    }

    // Produce result values with actual types
    let result_types = get_expr_result_types(expr, source, symbols);
    checker.push_vals(&result_types);
}

/// Find the expr1_call node inside a folded expression.
/// The grammar uses string literals for "call_indirect", so the expr1_call node
/// itself holds type_use/index children needed for type resolution.
macro_rules! find_expr1_call_in_expr_body {
    ($expr:ident) => {{
        let mut cursor = $expr.walk();
        for child in $expr.children(&mut cursor) {
            node_kind!(kind = child);
            if kind == "expr1_call" {
                return Some(node_copy!(&child));
            }
            if kind == "expr1" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    node_kind!(ik = inner);
                    if ik == "expr1_call" {
                        return Some(node_copy!(&inner));
                    }
                }
            }
        }
        None
    }};
}
#[cfg(feature = "native")]
fn find_expr1_call_in_expr<'a>(expr: &'a Node) -> Option<Node<'a>> {
    find_expr1_call_in_expr_body!(expr)
}
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_expr1_call_in_expr(expr: &Node) -> Option<Node> {
    find_expr1_call_in_expr_body!(expr)
}

/// Get result types from a folded expression
fn get_expr_result_types(expr: &Node, source: &str, symbols: &SymbolTable) -> Vec<ValueType> {
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        node_kind!(kind = child);

        if kind.starts_with("expr1_") {
            return get_expr1_result_types(&child, source, symbols);
        }
        if kind == "expr1" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind.starts_with("expr1_") {
                    return get_expr1_result_types(&inner_child, source, symbols);
                }
            }
        }
    }
    vec![ValueType::Unknown]
}

/// Get result types for expr1_* nodes
fn get_expr1_result_types(expr1: &Node, source: &str, symbols: &SymbolTable) -> Vec<ValueType> {
    node_kind!(kind = expr1);

    match kind.as_ref() {
        "expr1_plain" => {
            let mut cursor = expr1.walk();
            for child in expr1.children(&mut cursor) {
                node_kind!(child_kind = child);

                if child_kind == "instr_plain" {
                    if let Some(instr_name) = get_instruction_name(&child, source) {
                        return infer_instruction_result_types(instr_name, &child, symbols, source);
                    }
                }
            }
            vec![ValueType::Unknown]
        }
        "expr1_block" | "expr1_loop" => get_block_result_types(expr1, source, symbols),
        "expr1_if" => get_block_result_types(expr1, source, symbols),
        "expr1_try" | "expr1_try_table" => get_block_result_types(expr1, source, symbols),
        "expr1_call" => {
            let mut cursor = expr1.walk();
            for child in expr1.children(&mut cursor) {
                node_kind!(child_kind = child);

                if child_kind == "call_indirect" || child_kind == "return_call_indirect" {
                    return get_call_indirect_result_types(expr1, symbols, source);
                }
            }
            if let Some(func_ref) = get_index_from_expr1_call(expr1, source) {
                let func = symbols.get_function_by_name(func_ref).or_else(|| {
                    func_ref
                        .parse::<usize>()
                        .ok()
                        .and_then(|idx| symbols.get_function_by_index(idx))
                });
                if let Some(f) = func {
                    return f.results.clone();
                }
            }
            vec![]
        }
        _ => vec![ValueType::Unknown],
    }
}

/// Get function reference from expr1_call node
fn get_index_from_expr1_call<'a>(node: &Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "index" || kind == "identifier" {
            return Some(source[child.byte_range()].trim());
        }
    }
    None
}

/// Resolve a type_use reference in a node's children to its TypeDef.
/// Looks for type_use nodes at the top level and inside block_block/loop_block/etc.
fn resolve_block_type_use<'a>(
    block_node: &Node,
    source: &str,
    symbols: &'a SymbolTable,
) -> Option<&'a TypeDef> {
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "type_use" {
            return resolve_type_use_node(&child, source, symbols);
        }
        if kind == "block_block"
            || kind == "block_loop"
            || kind == "if_block"
            || kind == "block_if"
            || kind == "block_try_table"
            || kind == "block_try"
        {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "type_use" {
                    return resolve_type_use_node(&inner_child, source, symbols);
                }
            }
        }
    }
    None
}

/// Resolve a type reference (name or numeric index) to a TypeDef.
fn resolve_type_ref<'a>(type_ref: &str, symbols: &'a SymbolTable) -> Option<&'a TypeDef> {
    symbols.get_type_by_name(type_ref).or_else(|| {
        type_ref
            .parse::<usize>()
            .ok()
            .and_then(|idx| symbols.get_type_by_index(idx))
    })
}

/// Resolve a type_use node to a TypeDef via the symbol table.
fn resolve_type_use_node<'a>(
    type_use_node: &Node,
    source: &str,
    symbols: &'a SymbolTable,
) -> Option<&'a TypeDef> {
    let mut cursor = type_use_node.walk();
    for child in type_use_node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "index" || kind == "identifier" {
            let index_text = source[child.byte_range()].trim();
            if let Some(td) = symbols.get_type_by_name(index_text) {
                return Some(td);
            }
            if let Ok(idx) = index_text.parse::<usize>() {
                return symbols.get_type_by_index(idx);
            }
        }
    }
    None
}

/// Resolve a numeric type index to a function signature, including implicit types.
///
/// WebAssembly modules can have implicit types created from function signatures.
/// When a function uses `(type N)` where N >= explicit type count, the index may
/// refer to an implicit type derived from function signatures.
///
/// Returns (params, results) for function types, None for non-function types.
fn resolve_type_index_to_func_sig(
    idx: usize,
    symbols: &SymbolTable,
) -> Option<(Vec<ValueType>, Vec<ValueType>)> {
    // First, try explicit types
    if let Some(td) = symbols.get_type_by_index(idx) {
        if let TypeKind::Func { params, results } = &td.kind {
            return Some((params.clone(), results.clone()));
        }
        return None;
    }

    // Build implicit type table: explicit type sigs + unique function sigs
    let capacity = symbols.types.len() + symbols.functions.len();
    let mut sigs: Vec<(Vec<ValueType>, Vec<ValueType>)> = Vec::with_capacity(capacity);
    let mut seen = std::collections::HashSet::with_capacity(capacity);
    for type_def in &symbols.types {
        if let TypeKind::Func { params, results } = &type_def.kind {
            let sig = (params.clone(), results.clone());
            seen.insert(sig.clone());
            sigs.push(sig);
        }
    }
    for func in &symbols.functions {
        if func.has_type_use {
            continue;
        }
        let params: Vec<ValueType> = func.parameters.iter().map(|p| p.param_type).collect();
        let sig = (params, func.results.clone());
        if !seen.contains(&sig) {
            seen.insert(sig.clone());
            sigs.push(sig);
        }
    }
    sigs.get(idx).cloned()
}

/// Get result types from a block/loop/if node
fn get_block_result_types(
    block_node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        node_kind!(kind = child);
        if kind == "func_type_results" {
            types.extend(parse_func_type_results(&child, source, Some(symbols)));
        }
        if kind == "block_type" {
            return parse_result_types(&child, source, symbols);
        }
        if kind == "block_block"
            || kind == "block_loop"
            || kind == "if_block"
            || kind == "block_if"
            || kind == "block_try_table"
            || kind == "block_try"
        {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "func_type_results" {
                    types.extend(parse_func_type_results(&inner_child, source, Some(symbols)));
                }
            }
        }
    }
    if !types.is_empty() {
        return types;
    }
    // If no explicit results, try resolving from type_use
    if let Some(td) = resolve_block_type_use(block_node, source, symbols) {
        if let TypeKind::Func { results, .. } = &td.kind {
            return results.clone();
        }
    }
    vec![]
}

/// Get param types from a block/loop/if node
fn get_block_param_types(block_node: &Node, source: &str, symbols: &SymbolTable) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut has_explicit_params = false;
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "func_type_params_many" {
            has_explicit_params = true;
            types.extend(parse_func_type_results(&child, source, Some(symbols)));
        }
        if kind == "block_block"
            || kind == "block_loop"
            || kind == "if_block"
            || kind == "block_if"
            || kind == "block_try_table"
            || kind == "block_try"
        {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "func_type_params_many" {
                    has_explicit_params = true;
                    types.extend(parse_func_type_results(&inner_child, source, Some(symbols)));
                }
            }
        }
    }
    // If no explicit params, try resolving from type_use
    if !has_explicit_params && types.is_empty() {
        if let Some(td) = resolve_block_type_use(block_node, source, symbols) {
            if let TypeKind::Func { params, .. } = &td.kind {
                return params.clone();
            }
        }
    }
    types
}

/// Parse result types from a func_type_results node.
/// Uses the full ref type parser for complex types like `(ref $t)` and `(ref null func)`.
/// When `symbols` is provided, named type references like `$t` are resolved.
fn parse_func_type_results(
    results_node: &Node,
    source: &str,
    symbols: Option<&SymbolTable>,
) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut cursor = results_node.walk();
    for child in results_node.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "ref_type" {
            let vt = resolve_parsed_ref_type(&child, source, symbols);
            types.push(vt);
        } else if kind == "value_type" {
            let type_text = &source[child.byte_range()];
            if let Some(t) = ValueType::try_parse(type_text.trim()) {
                types.push(t);
            } else {
                // May be a value_type wrapping a ref_type
                let vt = resolve_parsed_ref_type(&child, source, symbols);
                types.push(vt);
            }
        }
    }
    types
}

/// Parse a ref type node with optional symbol resolution for named types.
/// Handles both `ref_type` and `value_type` nodes.
fn resolve_parsed_ref_type(node: &Node, source: &str, symbols: Option<&SymbolTable>) -> ValueType {
    // First try symbol resolution for named types (works on any node by recursing)
    if let Some(syms) = symbols {
        if let Some(resolved) = crate::parser::try_resolve_named_ref_in_subtree(node, source, syms)
        {
            return resolved;
        }
    }
    // Fall back to extract_ref_type for abstract types and numeric indices.
    // For `value_type` nodes, find the nested `ref_type` child and extract from there.
    extract_ref_type_from_any_node(node, source)
}

/// Extract a ref type from any node, handling value_type wrappers.
/// Unlike `extract_ref_type` which expects a `ref_type` node, this handles
/// value_type → value_type_ref_type → ref_type nesting.
fn extract_ref_type_from_any_node(node: &Node, source: &str) -> ValueType {
    let kind = node.kind();
    if kind == "ref_type" || kind.starts_with("ref_type_") {
        return crate::parser::extract_ref_type(node, source);
    }
    // Search for nested ref_type children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        if ck == "ref_type" || ck.starts_with("ref_type_") {
            return crate::parser::extract_ref_type(&child, source);
        }
        if ck == "value_type_ref_type" {
            // Recurse into value_type_ref_type wrapper
            let mut inner = child.walk();
            for inner_child in child.children(&mut inner) {
                if inner_child.kind() == "ref_type" || inner_child.kind().starts_with("ref_type_") {
                    return crate::parser::extract_ref_type(&inner_child, source);
                }
            }
            // Try extract_ref_type on the wrapper itself
            return crate::parser::extract_ref_type(&child, source);
        }
    }
    // Last resort: try extract_ref_type on the node itself
    crate::parser::extract_ref_type(node, source)
}

/// Parse result types from a block_type node
fn parse_result_types(block_type: &Node, source: &str, symbols: &SymbolTable) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut cursor = block_type.walk();

    for child in block_type.children(&mut cursor) {
        node_kind!(kind = child);

        if kind == "func_type_results" {
            types.extend(parse_func_type_results(&child, source, Some(symbols)));
        }
        if kind == "ref_type" {
            let vt = resolve_parsed_ref_type(&child, source, Some(symbols));
            types.push(vt);
        } else if kind == "value_type" {
            let type_text = &source[child.byte_range()];
            if let Some(t) = ValueType::try_parse(type_text.trim()) {
                types.push(t);
            } else {
                let vt = resolve_parsed_ref_type(&child, source, Some(symbols));
                types.push(vt);
            }
        }
    }

    if types.is_empty() {
        let text = &source[block_type.byte_range()];
        if text.contains("result") {
            for keyword in ["i32", "i64", "f32", "f64", "v128", "funcref", "externref"] {
                for _ in 0..text.matches(keyword).count() {
                    if let Some(t) = ValueType::try_parse(keyword) {
                        types.push(t);
                    }
                }
            }
        }
    }

    types
}

/// Validate tail call return types in a folded expression.
fn validate_tail_call_in_folded_expr(
    expr: &Node,
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> Option<Diagnostic> {
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        node_kind!(kind = child);

        if kind.starts_with("expr1_") {
            if kind == "expr1_call" {
                return validate_tail_call_return_types(&child, instr_name, symbols, source);
            }
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                node_kind!(inner_kind = inner_child);

                if inner_kind == "instr_plain" {
                    return validate_tail_call_return_types(
                        &inner_child,
                        instr_name,
                        symbols,
                        source,
                    );
                }
            }
        }
        if kind == "expr1" {
            let mut mid_cursor = child.walk();
            for mid_child in child.children(&mut mid_cursor) {
                node_kind!(mid_kind = mid_child);

                if mid_kind == "expr1_call" {
                    return validate_tail_call_return_types(
                        &mid_child, instr_name, symbols, source,
                    );
                }
                if mid_kind.starts_with("expr1_") {
                    let mut inner_cursor = mid_child.walk();
                    for inner_child in mid_child.children(&mut inner_cursor) {
                        node_kind!(inner_kind = inner_child);

                        if inner_kind == "instr_plain" {
                            return validate_tail_call_return_types(
                                &inner_child,
                                instr_name,
                                symbols,
                                source,
                            );
                        }
                    }
                }
            }
        }
    }
    None
}

/// Format a slice of ValueTypes for display
fn format_types(types: &[ValueType]) -> String {
    if types.is_empty() {
        return "(none)".to_string();
    }
    use std::fmt::Write;
    let mut out = String::from("(");
    for (i, t) in types.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "{}", t);
    }
    out.push(')');
    out
}

/// Validate that a tail call instruction's callee return types match the enclosing function's.
fn validate_tail_call_return_types(
    node: &Node,
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> Option<Diagnostic> {
    let func_line = node.start_position().row as u32;
    let enclosing_func = symbols.find_function_containing_line(func_line)?;

    let callee_results = match instr_name {
        "return_call" => get_call_result_types(node, symbols, source),
        "return_call_ref" => get_call_ref_result_types(node, symbols, source),
        "return_call_indirect" => get_call_indirect_result_types(node, symbols, source),
        _ => return None,
    };

    if callee_results.contains(&ValueType::Unknown) {
        return None;
    }

    // Apply NonNull refinement to both sides for abstract heap types.
    // The symbol table stores (ref func) and funcref both as Funcref, losing the
    // nullability distinction. We recover it from the AST by checking for the
    // explicit `(ref ...)` syntax (without `null`).
    let enclosing_results = apply_nonnull_from_ast_func(node, &enclosing_func.results, source);
    let callee_results =
        apply_nonnull_to_callee_results(instr_name, node, &callee_results, symbols, source);

    // Check result type compatibility using subtyping
    let types_mismatch = callee_results.len() != enclosing_results.len()
        || callee_results
            .iter()
            .zip(enclosing_results.iter())
            .any(|(callee, enclosing)| !types_compatible_with_symbols(callee, enclosing, symbols));
    if types_mismatch {
        let callee_name = get_index_from_node(node, source).unwrap_or_default();
        let label = if callee_name.is_empty() {
            instr_name.to_string()
        } else {
            format!("{} {}", instr_name, callee_name)
        };
        let range = node_to_range(node);
        return Some(
            Diagnostic::error(
                range,
                format!(
                    "type mismatch: '{}' returns {} but enclosing function returns {}",
                    label,
                    format_types(&callee_results),
                    format_types(&enclosing_results),
                ),
            )
            .with_code("type-mismatch"),
        );
    }

    None
}

/// Apply NonNull refinement to enclosing function result types by finding the function's
/// AST node and checking each result type's `value_type` node for non-nullable ref syntax.
fn apply_nonnull_from_ast_func(
    instr_node: &Node,
    base_results: &[ValueType],
    source: &str,
) -> Vec<ValueType> {
    // Walk up from the instruction to find the enclosing module_field_func node
    let func_node = {
        let mut current = instr_node.parent();
        loop {
            match current {
                Some(n) => {
                    node_kind!(k = n);
                    if k == "module_field_func" {
                        break Some(n);
                    }
                    current = n.parent();
                }
                None => break None,
            }
        }
    };
    let func_node = match func_node {
        Some(f) => f,
        None => return base_results.to_vec(),
    };
    apply_nonnull_to_func_results(&func_node, base_results, source)
}

/// Apply NonNull refinement to callee type result types for tail call validation.
/// Finds the callee type definition's AST node by looking for the type_use or type index
/// on the instruction, then locates the corresponding type definition in the tree.
fn apply_nonnull_to_callee_results(
    instr_name: &str,
    node: &Node,
    base_results: &[ValueType],
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    // Find the callee's type definition
    let type_def = match instr_name {
        "return_call_ref" => get_index_from_node(node, source).and_then(|type_ref| {
            symbols.get_type_by_name(type_ref).or_else(|| {
                type_ref
                    .parse::<usize>()
                    .ok()
                    .and_then(|idx| symbols.get_type_by_index(idx))
            })
        }),
        "return_call" => {
            // For return_call, get the function's type via its type_index
            get_index_from_node(node, source).and_then(|func_ref| {
                let func = symbols.get_function_by_name(func_ref).or_else(|| {
                    func_ref
                        .parse::<usize>()
                        .ok()
                        .and_then(|idx| symbols.get_function_by_index(idx))
                })?;
                func.type_index
                    .and_then(|tidx| symbols.get_type_by_index(tidx))
            })
        }
        _ => None,
    };
    let type_def = match type_def {
        Some(td) => td,
        None => return base_results.to_vec(),
    };

    // Find the type definition's AST node in the tree by searching from root
    let type_line = type_def.line as usize;
    let root = {
        let mut current = match node.parent() {
            Some(n) => n,
            None => return base_results.to_vec(),
        };
        while let Some(parent) = current.parent() {
            current = parent;
        }
        current
    };

    // Search for the module_field_type at the right line
    if let Some(type_node) = find_type_node_at_line(&root, type_line) {
        // Find the func_type within the type definition and apply NonNull
        if let Some(func_type_node) = find_func_type_in_type_def(&type_node) {
            return apply_nonnull_to_func_results(&func_type_node, base_results, source);
        }
    }

    base_results.to_vec()
}

/// Walk func_type_results children and apply NonNull conversion to abstract types
/// where the source uses explicit non-nullable ref syntax.
fn apply_nonnull_to_func_results(
    func_node: &Node,
    base_results: &[ValueType],
    source: &str,
) -> Vec<ValueType> {
    let mut results = base_results.to_vec();
    let mut result_idx = 0;
    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        if child.kind() == "func_type_results" {
            let mut results_cursor = child.walk();
            for result_child in child.children(&mut results_cursor) {
                if result_child.kind() == "value_type" && result_idx < results.len() {
                    if crate::parser::is_ref_type_non_nullable(&result_child, source) {
                        results[result_idx] =
                            crate::parser::to_nonnull_variant(results[result_idx]);
                    }
                    result_idx += 1;
                }
            }
        }
    }
    results
}

/// Find a module_field_type node at the given line in the tree.
fn find_type_node_at_line<'a>(node: &RetNode<'a>, line: usize) -> Option<RetNode<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(k = child);
        if k == "module_field_type" && child.start_position().row == line {
            return Some(child);
        }
        if k == "module" || k == "module_field" || k == "module_field_rec" {
            if let Some(found) = find_type_node_at_line(&child, line) {
                return Some(found);
            }
        }
    }
    None
}

/// Find the func_type node within a type definition (handles sub_type, def_type, type_field).
fn find_func_type_in_type_def<'a>(node: &RetNode<'a>) -> Option<RetNode<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(k = child);
        if k == "func_type" {
            return Some(child);
        }
        // Recurse into sub_type, def_type, type_field containers
        if matches!(k, "sub_type" | "def_type" | "type_field") {
            if let Some(found) = find_func_type_in_type_def(&child) {
                return Some(found);
            }
        }
    }
    None
}

/// Resolve elem segment from an instruction's indices.
/// For table.init with 1 index: it's the elem idx. With 2 indices: second is elem idx.
fn resolve_elem_for_table_init<'a>(
    node: &Node,
    symbols: &'a SymbolTable,
    source: &str,
) -> Option<&'a crate::symbols::ElemSegment> {
    let mut indices = Vec::new();
    collect_instruction_indices(node, source, &mut indices);

    let elem_ref = if indices.len() == 1 {
        indices[0]
    } else if indices.len() >= 2 {
        indices[1]
    } else {
        return symbols.elem_segments.first();
    };

    if let Some(elem) = symbols.get_elem_by_name(elem_ref) {
        return Some(elem);
    }
    if let Some(idx) = parse_wat_nat(elem_ref) {
        return symbols.elem_segments.get(idx as usize);
    }
    None
}

/// Resolve table from table.init's indices.
/// With 1 index: table defaults to 0. With 2 indices: first is table idx.
fn resolve_table_for_table_init<'a>(
    node: &Node,
    symbols: &'a SymbolTable,
    source: &str,
) -> Option<&'a Table> {
    let mut indices = Vec::new();
    collect_instruction_indices(node, source, &mut indices);

    if indices.len() >= 2 {
        let table_ref = indices[0];
        if let Some(table) = symbols.get_table_by_name(table_ref) {
            return Some(table);
        }
        if let Some(idx) = parse_wat_nat(table_ref) {
            return symbols.tables.get(idx as usize);
        }
    }
    symbols.tables.first()
}

/// Collect index/identifier text values from instruction node children.
fn collect_instruction_indices<'a>(node: &Node, source: &'a str, out: &mut Vec<&'a str>) {
    let mut c = node.walk();
    for child in node.children(&mut c) {
        let kind = child.kind();
        if kind == "index" || kind == "identifier" {
            out.push(source[child.byte_range()].trim());
        } else if kind.starts_with("op_") {
            collect_instruction_indices(&child, source, out);
        }
    }
}

/// Check table.init type compatibility between table and elem segment ref types.
fn check_table_init_types(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
    checker: &mut TypeChecker,
) {
    let table = resolve_table_for_table_init(node, symbols, source);
    let elem = resolve_elem_for_table_init(node, symbols, source);
    if let (Some(table), Some(elem)) = (table, elem) {
        if !ref_types_compatible(elem.ref_type, table.ref_type, symbols) {
            checker.diagnostics.push(
                Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
            );
        }
    }
}

/// Check table.copy type compatibility between destination and source tables.
fn check_table_copy_types(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
    checker: &mut TypeChecker,
) {
    let mut indices = Vec::new();
    collect_instruction_indices(node, source, &mut indices);

    let (dst, src) = if indices.len() >= 2 {
        let dst = symbols
            .get_table_by_name(indices[0])
            .or_else(|| parse_wat_nat(indices[0]).and_then(|i| symbols.tables.get(i as usize)));
        let src = symbols
            .get_table_by_name(indices[1])
            .or_else(|| parse_wat_nat(indices[1]).and_then(|i| symbols.tables.get(i as usize)));
        (dst, src)
    } else {
        (symbols.tables.first(), symbols.tables.first())
    };

    if let (Some(dst), Some(src)) = (dst, src) {
        if !ref_types_compatible(src.ref_type, dst.ref_type, symbols) {
            checker.diagnostics.push(
                Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
            );
        }
    }
}

/// Resolve address types for table.copy $dst $src.
fn resolve_table_copy_addr_types(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> (ValueType, ValueType) {
    let mut indices = Vec::new();
    collect_instruction_indices(node, source, &mut indices);

    let addr = |t: Option<&Table>| -> ValueType {
        t.map(|t| {
            if t.is_table64 {
                ValueType::I64
            } else {
                ValueType::I32
            }
        })
        .unwrap_or(ValueType::I32)
    };

    if indices.len() >= 2 {
        let dst = symbols
            .get_table_by_name(indices[0])
            .or_else(|| parse_wat_nat(indices[0]).and_then(|i| symbols.tables.get(i as usize)));
        let src = symbols
            .get_table_by_name(indices[1])
            .or_else(|| parse_wat_nat(indices[1]).and_then(|i| symbols.tables.get(i as usize)));
        (addr(dst), addr(src))
    } else {
        let t = symbols.tables.first();
        (addr(t), addr(t))
    }
}

/// Check array mutation instructions for mutability and type compatibility.
///
/// Validates:
/// - array.set/fill/copy/init_data/init_elem: destination array must be mutable
/// - array.copy: source and destination element types must match
/// - array.fill: value type must match element type
/// - array.init_data: element type must be numeric or vector
/// - array.init_elem: elem segment type must match element type
fn check_array_operations(
    instr_name: &str,
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
    checker: &mut TypeChecker,
) {
    // Only check array mutation/bulk instructions
    if !matches!(
        instr_name,
        "array.set" | "array.fill" | "array.copy" | "array.init_data" | "array.init_elem"
    ) {
        return;
    }

    // Resolve the first (destination) type index
    let dest_type_def = resolve_first_type_index(node, symbols, source);

    // Check mutability for destination array
    if let Some(type_def) = dest_type_def {
        if let TypeKind::Array { mutable, .. } = &type_def.kind {
            if !mutable {
                checker.diagnostics.push(
                    Diagnostic::error(node_to_range(node), "immutable array")
                        .with_code("immutable-array"),
                );
                return; // Don't report additional errors after mutability failure
            }
        }
    }

    match instr_name {
        "array.copy" => {
            // Both type indices must resolve to array types with compatible element types
            let dest_elem = dest_type_def.and_then(|td| match &td.kind {
                TypeKind::Array { element_type, .. } => Some(*element_type),
                _ => None,
            });
            let src_elem =
                resolve_nth_type_index(node, symbols, source, 1).and_then(|td| match &td.kind {
                    TypeKind::Array { element_type, .. } => Some(*element_type),
                    _ => None,
                });
            if let (Some(dst), Some(src)) = (dest_elem, src_elem) {
                if dst != src && !ref_types_compatible(src, dst, symbols) {
                    checker.diagnostics.push(
                        Diagnostic::error(
                            node_to_range(node),
                            "array types do not match".to_string(),
                        )
                        .with_code("type-mismatch"),
                    );
                }
            }
        }
        "array.fill" => {
            // Type checking handled via consumed types in get_instruction_consumed_types
        }
        "array.init_data" => {
            // Element type must be numeric or vector (not a reference type)
            if let Some(type_def) = dest_type_def {
                if let TypeKind::Array { element_type, .. } = &type_def.kind {
                    if !is_numeric_or_vector(*element_type) {
                        checker.diagnostics.push(
                            Diagnostic::error(
                                node_to_range(node),
                                "array type is not numeric or vector".to_string(),
                            )
                            .with_code("type-mismatch"),
                        );
                    }
                }
            }
        }
        "array.init_elem" => {
            // Elem segment type must be compatible with array element type
            if let Some(type_def) = dest_type_def {
                if let TypeKind::Array { element_type, .. } = &type_def.kind {
                    if let Some(elem_seg) = resolve_elem_for_array_init(node, symbols, source) {
                        if !ref_types_compatible(elem_seg.ref_type, *element_type, symbols)
                            && *element_type != elem_seg.ref_type
                        {
                            checker.diagnostics.push(
                                Diagnostic::error(node_to_range(node), "type mismatch")
                                    .with_code("type-mismatch"),
                            );
                        }
                    }
                }
            }
        }
        _ => {} // array.set handled by mutability check only
    }
}

/// Get the element count (second immediate) from an array.new_fixed instruction.
/// Format: `array.new_fixed $type N` — the N is a nat child after the type index.
fn get_array_new_fixed_count(node: &Node, source: &str) -> usize {
    let mut cursor = node.walk();
    let mut past_index = false;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "index" || kind == "identifier" {
            past_index = true;
        } else if kind == "nat" && past_index {
            let text = &source[child.byte_range()];
            return text.trim().parse::<usize>().unwrap_or(0);
        }
        // Also check inside op_ wrappers
        if kind.starts_with("op_") {
            let mut inner = child.walk();
            for inner_child in child.children(&mut inner) {
                let ik = inner_child.kind();
                if ik == "index" || ik == "identifier" {
                    past_index = true;
                } else if ik == "nat" && past_index {
                    let text = &source[inner_child.byte_range()];
                    return text.trim().parse::<usize>().unwrap_or(0);
                }
            }
        }
    }
    0
}

/// Unpack packed storage types (i8, i16) to their stack representation (i32).
fn unpack_storage_type(ty: ValueType) -> ValueType {
    match ty {
        ValueType::I8 | ValueType::I16 => ValueType::I32,
        other => other,
    }
}

/// Check if a ValueType is numeric (i32, i64, f32, f64) or packed (i8, i16).
fn is_numeric_type(ty: ValueType) -> bool {
    matches!(
        ty,
        ValueType::I32
            | ValueType::I64
            | ValueType::F32
            | ValueType::F64
            | ValueType::I8
            | ValueType::I16
    )
}

/// Check if a ValueType is numeric or vector (valid for array.init_data).
fn is_numeric_or_vector(ty: ValueType) -> bool {
    is_numeric_type(ty) || ty == ValueType::V128
}

/// Resolve the Nth type index (0-based) in an instruction to a TypeDef.
fn resolve_nth_type_index<'a>(
    node: &Node,
    symbols: &'a SymbolTable,
    source: &str,
    n: usize,
) -> Option<&'a TypeDef> {
    let mut indices = Vec::new();
    collect_instruction_indices(node, source, &mut indices);
    let index_str = indices.get(n)?;
    if let Some(t) = symbols.get_type_by_name(index_str) {
        return Some(t);
    }
    if let Some(idx) = parse_wat_nat(index_str) {
        return symbols.types.get(idx as usize);
    }
    None
}

/// Resolve the elem segment for array.init_elem instruction.
/// array.init_elem $type $elem: first index is type, second is elem segment.
fn resolve_elem_for_array_init<'a>(
    node: &Node,
    symbols: &'a SymbolTable,
    source: &str,
) -> Option<&'a ElemSegment> {
    let mut indices = Vec::new();
    collect_instruction_indices(node, source, &mut indices);
    let elem_ref = indices.get(1)?;
    if let Some(elem) = symbols.get_elem_by_name(elem_ref) {
        return Some(elem);
    }
    if let Some(idx) = parse_wat_nat(elem_ref) {
        return symbols.elem_segments.get(idx as usize);
    }
    None
}

/// Resolve the first type index in an instruction to a TypeDef.
fn resolve_first_type_index<'a>(
    node: &Node,
    symbols: &'a SymbolTable,
    source: &str,
) -> Option<&'a TypeDef> {
    let index = get_index_from_node(node, source)?;
    if let Some(t) = symbols.get_type_by_name(index) {
        return Some(t);
    }
    if let Some(idx) = parse_wat_nat(index) {
        return symbols.types.get(idx as usize);
    }
    None
}

/// Resolve the struct field type for struct.get/struct.get_s/struct.get_u.
/// Extracts both indices (type_ref, field_ref), resolves the struct type,
/// and returns the unpacked field type.
fn get_struct_field_result_type(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    let mut indices = Vec::new();
    collect_instruction_indices(node, source, &mut indices);
    if indices.len() < 2 {
        return vec![ValueType::Unknown];
    }
    let type_ref = indices[0];
    let field_ref = indices[1];

    // Resolve the type def
    let type_def = if type_ref.starts_with('$') {
        symbols.get_type_by_name(type_ref)
    } else {
        type_ref
            .parse::<usize>()
            .ok()
            .and_then(|idx| symbols.get_type_by_index(idx))
    };

    let type_def = match type_def {
        Some(td) => td,
        None => return vec![ValueType::Unknown],
    };

    let fields = match &type_def.kind {
        TypeKind::Struct { fields } => fields,
        _ => return vec![ValueType::Unknown],
    };

    // Resolve the field index
    let field_idx = if field_ref.starts_with('$') {
        fields
            .iter()
            .position(|(name, _, _)| name.as_deref() == Some(field_ref))
    } else {
        field_ref
            .parse::<usize>()
            .ok()
            .filter(|&idx| idx < fields.len())
    };

    match field_idx {
        Some(idx) => vec![unpack_storage_type(fields[idx].1)],
        None => vec![ValueType::Unknown],
    }
}

/// Check if source ref type is compatible with destination ref type (with symbol table).
fn ref_types_compatible(src: ValueType, dst: ValueType, symbols: &SymbolTable) -> bool {
    if src == dst {
        return true;
    }
    super::type_check::types_compatible_with_symbols(&src, &dst, symbols)
}

/// Check br_on_cast / br_on_cast_fail type validation.
///
/// The instruction format is: `br_on_cast <label> <rt1> <rt2>`
/// Validation: rt2 must be a subtype of rt1 (including nullability).
/// For br_on_cast: branch carries rt2, fallthrough gets diff(rt1, rt2).
/// For br_on_cast_fail: branch carries diff(rt1, rt2), fallthrough gets rt2.
fn check_br_on_cast_types(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
    is_fail: bool,
) {
    let cast_types = extract_cast_ref_types(node, source, symbols);
    if cast_types.len() < 2 {
        return;
    }
    let (rt1, rt1_nullable) = &cast_types[0];
    let (rt2, rt2_nullable) = &cast_types[1];

    let mut has_error = false;

    // Validate rt2 <: rt1: nullable rt2 can't be subtype of non-null rt1
    if *rt2_nullable && !*rt1_nullable {
        checker.diagnostics.push(
            Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
        );
        has_error = true;
    }

    // Check heap type subtyping (use existing heap_types approach)
    if !has_error {
        let heap_types = extract_heap_types_from_cast(node, source);
        if heap_types.len() >= 2 && !is_heap_subtype(heap_types[1], heap_types[0]) {
            checker.diagnostics.push(
                Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
            );
            has_error = true;
        }
    }

    // Resolve branch label depth and check branch/fallthrough types
    let label_depth = resolve_cast_label_depth(node, source, checker);

    if !has_error {
        if let Some(depth) = label_depth {
            // Compute diff nullability: nullable iff rt1 nullable and rt2 not nullable
            let diff_nullable = *rt1_nullable && !*rt2_nullable;

            // Branch type
            let (branch_type, branch_nullable) = if !is_fail {
                (*rt2, *rt2_nullable)
            } else {
                (*rt1, diff_nullable)
            };

            // Check branch type against label
            if let Some(label_types) = checker.label_types(depth) {
                let label_types = label_types.to_vec();
                if label_types.len() == 1 {
                    let label_type = &label_types[0];
                    let branch_vt = if branch_nullable {
                        make_nullable(&branch_type)
                    } else {
                        make_non_nullable(&branch_type)
                    };
                    if !types_compatible_with_symbols(&branch_vt, label_type, symbols) {
                        checker.diagnostics.push(
                            Diagnostic::error(node_to_range(node), "type mismatch")
                                .with_code("type-mismatch"),
                        );
                    }
                }
            }
        }
    }

    // Always push fallthrough type regardless of errors
    if label_depth.is_some() {
        let diff_nullable = *rt1_nullable && !*rt2_nullable;
        let (ft_type, ft_nullable) = if !is_fail {
            (*rt1, diff_nullable)
        } else {
            (*rt2, *rt2_nullable)
        };
        let ft_vt = if ft_nullable {
            make_nullable(&ft_type)
        } else {
            make_non_nullable(&ft_type)
        };
        checker.push_val(ft_vt);
    } else {
        checker.push_val(ValueType::Unknown);
    }
}

/// Make a ValueType nullable (Ref→RefNull, NN→nullable variant).
fn make_nullable(vt: &ValueType) -> ValueType {
    match vt {
        ValueType::Ref(n) => ValueType::RefNull(*n),
        ValueType::NonNullFuncref => ValueType::Funcref,
        ValueType::NonNullExternref => ValueType::Externref,
        ValueType::NonNullAnyref => ValueType::Anyref,
        ValueType::NonNullEqref => ValueType::Eqref,
        ValueType::NonNullI31ref => ValueType::I31ref,
        ValueType::NonNullStructref => ValueType::Structref,
        ValueType::NonNullArrayref => ValueType::Arrayref,
        // Already nullable or not a ref
        other => *other,
    }
}

/// Make a ValueType non-nullable (RefNull→Ref, nullable abstract→NN variant).
fn make_non_nullable(vt: &ValueType) -> ValueType {
    match vt {
        ValueType::RefNull(n) => ValueType::Ref(*n),
        ValueType::Funcref => ValueType::NonNullFuncref,
        ValueType::Externref => ValueType::NonNullExternref,
        ValueType::Anyref => ValueType::NonNullAnyref,
        ValueType::Eqref => ValueType::NonNullEqref,
        ValueType::I31ref => ValueType::NonNullI31ref,
        ValueType::Structref => ValueType::NonNullStructref,
        ValueType::Arrayref => ValueType::NonNullArrayref,
        // Already non-nullable or not a ref
        other => *other,
    }
}

/// Extract the label depth from a br_on_cast/br_on_cast_fail instruction node.
fn resolve_cast_label_depth(node: &Node, source: &str, checker: &TypeChecker) -> Option<usize> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "index" || kind == "identifier" {
            let text = source[child.byte_range()].trim();
            if text.starts_with('$') {
                return checker.resolve_label_depth(text);
            } else {
                return text.parse::<usize>().ok();
            }
        }
        // Also look inside op_ nodes
        if kind.starts_with("op_") {
            let mut inner = child.walk();
            for ic in child.children(&mut inner) {
                if ic.kind() == "index" || ic.kind() == "identifier" {
                    let text = source[ic.byte_range()].trim();
                    if text.starts_with('$') {
                        return checker.resolve_label_depth(text);
                    } else {
                        return text.parse::<usize>().ok();
                    }
                }
            }
        }
    }
    None
}

/// Extract ref type arguments from br_on_cast/br_on_cast_fail as (ValueType, is_nullable) pairs.
fn extract_cast_ref_types(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<(ValueType, bool)> {
    let mut types = Vec::new();
    let mut cursor = node.walk();
    let mut past_index = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        // Skip the op_ prefix and label index
        if (kind == "index" || kind == "identifier") && !past_index {
            past_index = true;
            continue;
        }
        if !past_index {
            if kind.starts_with("op_") {
                let mut inner = child.walk();
                for ic in child.children(&mut inner) {
                    if ic.kind() == "index" || ic.kind() == "identifier" {
                        past_index = true;
                        break;
                    }
                }
            }
            continue;
        }
        // After the label, parse ref type nodes
        if kind == "ref_type_ref" {
            // Full form: (ref null? heap_type)
            // Parse directly: iterate children for null, ref_kind, index
            let mut is_null = false;
            let mut vt = ValueType::Unknown;
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                let gk = gc.kind();
                if gk == "null" || source[gc.byte_range()].trim() == "null" {
                    is_null = true;
                } else if gk == "ref_kind" {
                    let text = source[gc.byte_range()].trim();
                    vt = match text {
                        "func" => ValueType::Funcref,
                        "extern" => ValueType::Externref,
                        "any" => ValueType::Anyref,
                        "eq" => ValueType::Eqref,
                        "i31" => ValueType::I31ref,
                        "struct" => ValueType::Structref,
                        "array" => ValueType::Arrayref,
                        "null" | "none" => ValueType::Nullref,
                        "nofunc" => ValueType::NullFuncref,
                        "noextern" => ValueType::NullExternref,
                        _ => ValueType::Unknown,
                    };
                } else if gk == "index" || gk == "identifier" {
                    let text = source[gc.byte_range()].trim();
                    if text.starts_with('$') {
                        if let Some(t) = symbols.get_type_by_name(text) {
                            vt = if is_null {
                                ValueType::RefNull(t.index as u32)
                            } else {
                                ValueType::Ref(t.index as u32)
                            };
                        }
                    } else if let Some(n) = parse_wat_nat(text) {
                        vt = if is_null {
                            ValueType::RefNull(n as u32)
                        } else {
                            ValueType::Ref(n as u32)
                        };
                    }
                }
            }
            // For ref_kind-based types, apply nullability
            if is_null && !matches!(vt, ValueType::Ref(_) | ValueType::RefNull(_)) {
                vt = make_nullable(&vt);
            } else if !is_null && !matches!(vt, ValueType::Ref(_) | ValueType::RefNull(_)) {
                vt = make_non_nullable(&vt);
            }
            types.push((vt, is_null));
        } else if kind == "ref_kind" {
            // Bare heap type: func, any, struct, etc. → non-nullable
            let text = source[child.byte_range()].trim();
            let vt = match text {
                "func" => ValueType::Funcref,
                "extern" => ValueType::Externref,
                "any" => ValueType::Anyref,
                "eq" => ValueType::Eqref,
                "i31" => ValueType::I31ref,
                "struct" => ValueType::Structref,
                "array" => ValueType::Arrayref,
                "null" | "none" => ValueType::Nullref,
                "nofunc" => ValueType::NullFuncref,
                "noextern" => ValueType::NullExternref,
                _ => ValueType::Unknown,
            };
            // ref_kind in br_on_cast context means non-nullable (ref ht)
            types.push((vt, false));
        } else if kind == "index" || kind == "identifier" {
            // Concrete type index — non-nullable
            let text = source[child.byte_range()].trim();
            if text.starts_with('$') {
                if let Some(t) = symbols.get_type_by_name(text) {
                    types.push((ValueType::Ref(t.index as u32), false));
                } else {
                    types.push((ValueType::Unknown, false));
                }
            } else if let Some(n) = parse_wat_nat(text) {
                types.push((ValueType::Ref(n as u32), false));
            } else {
                types.push((ValueType::Unknown, false));
            }
        } else if kind.ends_with("ref") {
            // Abbreviated forms: funcref, anyref, etc. → nullable
            let text = source[child.byte_range()].trim();
            let vt = ValueType::try_parse(text).unwrap_or(ValueType::Unknown);
            types.push((vt, true));
        }
    }
    types
}

/// Validate catch clause types in try_table against the target label's expected types.
///
/// Grammar: `(catch $tag $label)` / `(catch_ref $tag $label)` / `(catch_all $label)` / `(catch_all_ref $label)`
///
/// The types routed to the label are:
/// - catch: tag parameter types
/// - catch_ref: tag parameter types + exnref
/// - catch_all: (empty)
/// - catch_all_ref: exnref
fn check_catch_clause_types(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    let mut cursor = node.walk();
    let mut catch_kind: Option<&str> = None;
    let mut indices: Vec<&str> = Vec::new();

    for child in node.children(&mut cursor) {
        node_kind!(kind = child);
        match kind.as_ref() {
            "catch" | "catch_ref" | "catch_all" | "catch_all_ref" => {
                catch_kind = Some(match kind.as_ref() {
                    "catch" => "catch",
                    "catch_ref" => "catch_ref",
                    "catch_all" => "catch_all",
                    _ => "catch_all_ref",
                });
            }
            "index" | "identifier" => {
                indices.push(source[child.byte_range()].trim());
            }
            _ => {}
        }
    }

    let catch_kind = match catch_kind {
        Some(k) => k,
        None => return,
    };

    // Determine what types are sent to the label
    let sent_types = match catch_kind {
        "catch" | "catch_ref" => {
            // First index is tag, second is label
            let tag_ref = match indices.first() {
                Some(r) => *r,
                None => return,
            };
            let tag = symbols
                .get_tag_by_name(tag_ref)
                .or_else(|| parse_wat_nat(tag_ref).and_then(|i| symbols.tags.get(i as usize)));
            let mut types = tag.map(|t| t.params.clone()).unwrap_or_default();
            if catch_kind == "catch_ref" {
                types.push(ValueType::Unknown); // exnref — no Exnref variant yet, Unknown matches anything
            }
            types
        }
        "catch_all" => vec![],
        "catch_all_ref" => vec![ValueType::Unknown], // exnref placeholder
        _ => return,
    };

    // Resolve the label index
    let label_ref = match catch_kind {
        "catch" | "catch_ref" => indices.get(1),
        _ => indices.first(),
    };
    let label_ref = match label_ref {
        Some(r) => *r,
        None => return,
    };

    // Catch clause labels are relative to the scope OUTSIDE the try_table.
    // Inside the type checker, the try_table frame is on top of the stack,
    // so we add 1 to convert from the catch clause's perspective to the checker's.
    let label_depth = if label_ref.starts_with('$') {
        checker.resolve_label_depth(label_ref)
    } else {
        label_ref.parse::<usize>().ok().map(|d| d + 1)
    };

    let label_depth = match label_depth {
        Some(d) => d,
        None => return,
    };

    // Get expected label types
    let label_types = match checker.label_types(label_depth) {
        Some(t) => t.to_vec(),
        None => return,
    };

    // Compare: sent types must match label types in count and type
    if sent_types.len() != label_types.len() {
        checker.diagnostics.push(
            Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
        );
        return;
    }

    for (sent, expected) in sent_types.iter().zip(label_types.iter()) {
        if !types_compatible_with_symbols(sent, expected, symbols) {
            checker.diagnostics.push(
                Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
            );
            return;
        }
    }
}

/// Extract heap type strings from br_on_cast/br_on_cast_fail instruction node.
///
/// Returns the two heap type text values (skipping the label index).
fn extract_heap_types_from_cast<'a>(node: &Node, source: &'a str) -> Vec<&'a str> {
    let mut types = Vec::new();
    let mut cursor = node.walk();
    let mut past_index = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        // Skip the op_ prefix and label index
        if (kind == "index" || kind == "identifier") && !past_index {
            past_index = true; // First index is the label
            continue;
        }
        if !past_index {
            // Look inside op_ nodes for the label index
            if kind.starts_with("op_") {
                let mut inner = child.walk();
                for ic in child.children(&mut inner) {
                    if ic.kind() == "index" || ic.kind() == "identifier" {
                        past_index = true;
                        break;
                    }
                }
            }
            continue;
        }
        // After the label, collect heap type values
        if kind == "ref_kind" || kind == "ref_type_ref" || kind == "index" || kind == "identifier" {
            types.push(source[child.byte_range()].trim());
        } else if kind.ends_with("ref") {
            // Abbreviated forms like "anyref", "funcref", etc.
            types.push(source[child.byte_range()].trim());
        }
    }
    types
}

/// Check if `sub` is a valid subtype of `sup` in the heap type hierarchy.
///
/// Hierarchy (from spec):
/// - any > eq > {i31, struct, array, $concrete_struct, $concrete_array}
/// - func > $concrete_func
/// - extern
/// - none (bottom of any), nofunc (bottom of func), noextern (bottom of extern)
///
/// Concrete type indices ($name or numeric) are considered subtypes of `any`/`eq`/`func`
/// depending on what they define (struct/array/func), but without the symbol table
/// we conservatively treat them as subtypes of both `any` and `func` hierarchies.
fn is_heap_subtype(sub: &str, sup: &str) -> bool {
    if sub == sup {
        return true;
    }
    // Normalize abbreviated ref types
    let sub = normalize_heap_type(sub);
    let sup = normalize_heap_type(sup);
    if sub == sup {
        return true;
    }

    let sub_is_concrete = is_concrete_type_ref(sub);
    let sup_is_concrete = is_concrete_type_ref(sup);

    // If either is a concrete type index, we can't fully validate without
    // symbol table — be conservative and only flag clearly invalid casts
    if sub_is_concrete || sup_is_concrete {
        // Concrete types are subtypes of most abstract supertypes
        // We don't have enough info to flag these, so allow them
        return true;
    }

    match sup {
        "any" => matches!(sub, "eq" | "i31" | "struct" | "array" | "none" | "null"),
        "eq" => matches!(sub, "i31" | "struct" | "array" | "none" | "null"),
        "func" => sub == "nofunc",
        "extern" => sub == "noextern",
        "exn" => sub == "noexn",
        "struct" => sub == "none" || sub == "null",
        "array" => sub == "none" || sub == "null",
        "i31" => sub == "none" || sub == "null",
        _ => false,
    }
}

/// Check if a heap type string refers to a concrete type (index or ref form).
fn is_concrete_type_ref(ty: &str) -> bool {
    ty.starts_with('$') || ty.starts_with("(ref") || ty.parse::<u32>().is_ok()
}

/// Normalize abbreviated heap type names to their canonical form.
/// Returns a `&str` (either static or a slice of the input) to avoid allocation.
fn normalize_heap_type(ty: &str) -> &str {
    match ty {
        "anyref" => "any",
        "funcref" => "func",
        "externref" => "extern",
        "eqref" => "eq",
        "i31ref" => "i31",
        "structref" => "struct",
        "arrayref" => "array",
        "nullref" => "null",
        "nullfuncref" => "nofunc",
        "nullexternref" => "noextern",
        "exnref" => "exn",
        "nullexnref" => "noexn",
        other => other,
    }
}
