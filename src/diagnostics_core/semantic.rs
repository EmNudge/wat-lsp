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
use crate::symbols::{SymbolTable, TypeKind, ValueType};
use crate::utils::node_to_range;

use super::sequence_always_terminates;
use super::type_check::{CtrlOpcode, TypeChecker};

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

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
        expected_results.unwrap_or(&func.results).to_vec()
    } else {
        expected_results.unwrap_or(&[]).to_vec()
    };

    // Push function-level control frame
    // Note: function parameters are local variables, NOT on the value stack.
    // The value stack starts empty; start_types is empty for functions.
    checker.push_ctrl(CtrlOpcode::Function, vec![], result_types.clone());

    // Process all children
    let mut cursor = instr_list.walk();
    for child in instr_list.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        match kind.as_ref() {
            "instr" => {
                process_instr_node(&child, source, symbols, &mut checker);
            }
            "instr_plain" => {
                if let Some(instr_name) = get_instruction_name(&child, source) {
                    process_instruction(&child, &instr_name, &mut checker, symbols, source);
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
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        match kind.as_ref() {
            "instr_plain" => {
                if let Some(instr_name) = get_instruction_name(&child, source) {
                    process_instruction(&child, &instr_name, checker, symbols, source);
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

    let params = get_block_param_types(node, source);
    let results = get_block_result_types(node, source);

    let opcode = if is_if {
        CtrlOpcode::If
    } else if is_loop_node(node) {
        CtrlOpcode::Loop
    } else {
        CtrlOpcode::Block
    };

    // Pop param types from outer stack (typed)
    if !params.is_empty() {
        let block_name = match opcode {
            CtrlOpcode::If => "if",
            CtrlOpcode::Loop => "loop",
            _ => "block",
        };
        checker.pop_vals_for_instr(&params, node, block_name);
    }

    checker.push_ctrl(opcode, params, results);

    // Process body
    process_block_body(node, source, symbols, checker);

    // Pop control frame
    checker.pop_ctrl(node);
}

/// Process an if instruction (instr_if — folded or linear format with explicit if keyword).
fn process_if_node(node: &Node, source: &str, symbols: &SymbolTable, checker: &mut TypeChecker) {
    // Pop i32 condition
    checker.pop_expect(&ValueType::I32, node);

    let params = get_block_param_types(node, source);
    let results = get_block_result_types(node, source);

    // Pop param types from outer stack
    if !params.is_empty() {
        checker.pop_vals_for_instr(&params, node, "if");
    }

    checker.push_ctrl(CtrlOpcode::If, params, results);

    // Process body
    process_block_body(node, source, symbols, checker);

    checker.pop_ctrl(node);
}

/// Process the body of a block/loop/if by recursing into children
fn process_block_body(node: &Node, source: &str, symbols: &SymbolTable, checker: &mut TypeChecker) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        match kind.as_ref() {
            "instr_list" => {
                // Process all instructions in the list
                let mut list_cursor = child.walk();
                for list_child in child.children(&mut list_cursor) {
                    #[cfg(feature = "native")]
                    let list_kind = list_child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let list_kind = list_child.kind();

                    match list_kind.as_ref() {
                        "instr" => {
                            process_instr_node(&list_child, source, symbols, checker);
                        }
                        "instr_plain" => {
                            if let Some(instr_name) = get_instruction_name(&list_child, source) {
                                process_instruction(
                                    &list_child,
                                    &instr_name,
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
            "block_block" | "loop_block" => {
                // Nested block in linear format
                process_block_body(&child, source, symbols, checker);
            }
            "block_if" | "if_block" => {
                // Nested if block in linear format
                process_block_body(&child, source, symbols, checker);
            }
            "instr_else" | "else" => {
                // At else, we need to reset to block params for the else branch
                // The then branch's values should match end_types, then reset
                // For simplicity, just process the else branch's instructions
                process_block_body(&child, source, symbols, checker);
            }
            // Direct instruction children
            "instr" => {
                process_instr_node(&child, source, symbols, checker);
            }
            "instr_plain" => {
                if let Some(instr_name) = get_instruction_name(&child, source) {
                    process_instruction(&child, &instr_name, checker, symbols, source);
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

/// Check if a node is a loop (instr_loop or contains loop_block)
fn is_loop_node(node: &Node) -> bool {
    #[cfg(feature = "native")]
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = node.kind();

    if kind == "instr_loop" {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = child.kind();
        if ck == "loop_block" {
            return true;
        }
    }
    false
}

/// Check if an instr_block node contains a block_if child (linear if format)
fn contains_block_if(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "block_if" {
            return true;
        }
    }
    false
}

/// Get the instruction name from an instr_plain node
pub fn get_instruction_name(instr_node: &Node, source: &str) -> Option<String> {
    let mut cursor = instr_node.walk();
    for child in instr_node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        // Handle different instruction types
        if kind.starts_with("op_") {
            // For op_const, the instruction name is in the first child (pat01)
            if kind == "op_const" {
                let mut inner_cursor = child.walk();
                for inner_child in child.children(&mut inner_cursor) {
                    #[cfg(feature = "native")]
                    let inner_kind = inner_child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let inner_kind = inner_child.kind();

                    // pat01 contains "i32.const", "i64.const", etc.
                    if inner_kind == "pat01" || inner_kind.contains("const") {
                        return Some(source[inner_child.byte_range()].trim().to_string());
                    }
                }
                // Fallback: extract first token from the whole op_const text
                let text = &source[child.byte_range()];
                return text.split_whitespace().next().map(|s| s.to_string());
            }
            // For other op_ nodes (like op_nullary, op_index, op_table_copy),
            // extract just the instruction name (first token)
            let text = &source[child.byte_range()];
            return text.split_whitespace().next().map(|s| s.to_string());
        }
    }
    // Fallback: get the text of the first token
    let text = &source[instr_node.byte_range()];
    text.split_whitespace().next().map(|s| s.to_string())
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
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

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
    if matches!(instr_name, "return_call" | "return_call_ref") {
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
            if let Some(label_types) = checker.label_types(depth) {
                let label_types = label_types.to_vec();
                checker.pop_vals_for_instr(&label_types, node, instr_name);
            }
        }
        checker.mark_unreachable();
        return;
    }

    if instr_name == "br_if" {
        // Pop i32 condition first, then pop+push label types
        checker.pop_expect(&ValueType::I32, node);
        if let Some(depth) = get_branch_depth(node, source, checker) {
            if let Some(label_types) = checker.label_types(depth) {
                let label_types = label_types.to_vec();
                checker.pop_vals_for_instr(&label_types, node, instr_name);
                checker.push_vals(&label_types);
            }
        }
        return;
    }

    if instr_name == "br_table" {
        // Pop i32 index, pop label types, mark unreachable
        checker.pop_expect(&ValueType::I32, node);
        // Use default label (last one) for type checking
        if let Some(depth) = get_last_branch_depth(node, source, checker) {
            if let Some(label_types) = checker.label_types(depth) {
                let label_types = label_types.to_vec();
                checker.pop_vals_for_instr(&label_types, node, instr_name);
            }
        }
        checker.mark_unreachable();
        return;
    }

    // Handle other terminating instructions
    if is_terminating_instruction(instr_name) {
        // For 'return', pop function result types
        if instr_name == "return" {
            if let Some(frame) = checker.function_frame() {
                let end_types = frame.end_types.clone();
                checker.pop_vals_for_instr(&end_types, node, instr_name);
            }
        }
        // For 'throw', pop tag parameter types
        if instr_name == "throw" {
            let consumed = get_instruction_consumed_types(node, instr_name, symbols, source);
            if !consumed.is_empty() {
                checker.pop_vals_for_instr(&consumed, node, instr_name);
            }
        }
        checker.mark_unreachable();
        return;
    }

    // Regular instructions: consume typed operands, produce typed results
    let consumed = get_instruction_consumed_types(node, instr_name, symbols, source);
    if !consumed.is_empty() {
        checker.pop_vals_for_instr(&consumed, node, instr_name);
    }

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
    // Helper to get type from instruction prefix
    let type_from_prefix = |name: &str| -> Option<ValueType> {
        if name.starts_with("i32.") {
            return Some(ValueType::I32);
        }
        if name.starts_with("i64.") {
            return Some(ValueType::I64);
        }
        if name.starts_with("f32.") {
            return Some(ValueType::F32);
        }
        if name.starts_with("f64.") {
            return Some(ValueType::F64);
        }
        if name.starts_with("v128.") {
            return Some(ValueType::V128);
        }
        None
    };

    // Helper to check if an instruction is a scalar binary op (2 operands of prefix type)
    let is_binary_scalar = |name: &str| -> bool {
        let is_simd = name.contains("x2.")
            || name.contains("x4.")
            || name.contains("x8.")
            || name.contains("x16.");
        if is_simd {
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
        let is_simd = name.contains("x2.")
            || name.contains("x4.")
            || name.contains("x8.")
            || name.contains("x16.");
        if is_simd {
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
        "select" => Some(vec![ValueType::Unknown, ValueType::Unknown, ValueType::I32]),

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
                if let Some(func) = symbols.get_function_by_name(&func_ref) {
                    return Some(
                        func.parameters
                            .iter()
                            .map(|p| p.param_type.clone())
                            .collect(),
                    );
                } else if let Ok(idx) = func_ref.parse::<usize>() {
                    if let Some(func) = symbols.get_function_by_index(idx) {
                        return Some(
                            func.parameters
                                .iter()
                                .map(|p| p.param_type.clone())
                                .collect(),
                        );
                    }
                }
            }
            None // fall back to untyped
        }

        // Call_ref — consume param types + typed funcref
        // Use Unknown for the funcref operand to be lenient (exact ref type varies)
        "call_ref" | "return_call_ref" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(type_def) = symbols.get_type_by_name(&type_ref) {
                    if let TypeKind::Func { params, .. } = &type_def.kind {
                        let mut types: Vec<ValueType> = params.clone();
                        types.push(ValueType::Unknown); // typed funcref (lenient)
                        return Some(types);
                    }
                } else if let Ok(idx) = type_ref.parse::<usize>() {
                    if let Some(type_def) = symbols.get_type_by_index(idx) {
                        if let TypeKind::Func { params, .. } = &type_def.kind {
                            let mut types: Vec<ValueType> = params.clone();
                            types.push(ValueType::Unknown);
                            return Some(types);
                        }
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

        // Memory load — consume address (i32 for memory32)
        name if name.contains(".load") => Some(vec![ValueType::I32]),

        // Memory store — consume address + value
        name if name.contains(".store") => {
            let value_type = type_from_prefix(name).unwrap_or(ValueType::Unknown);
            Some(vec![ValueType::I32, value_type])
        }

        // Memory size — no operands
        "memory.size" => Some(vec![]),
        // Memory grow — consume delta (i32)
        "memory.grow" => Some(vec![ValueType::I32]),

        // Reference instructions
        "ref.null" => Some(vec![]),
        "ref.func" => Some(vec![]),
        "ref.is_null" => Some(vec![ValueType::Unknown]), // any ref
        "ref.as_non_null" => Some(vec![ValueType::Unknown]), // any nullable ref
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
        "array.fill" => Some(vec![
            ValueType::Unknown,
            ValueType::I32,
            ValueType::Unknown,
            ValueType::I32,
        ]), // arrayref, index, value, len
        "array.copy" => Some(vec![
            ValueType::Unknown,
            ValueType::I32,
            ValueType::Unknown,
            ValueType::I32,
            ValueType::I32,
        ]),
        "array.new_data" | "array.new_elem" => Some(vec![ValueType::I32, ValueType::I32]), // offset, length
        "array.init_data" | "array.init_elem" => Some(vec![
            ValueType::Unknown,
            ValueType::I32,
            ValueType::I32,
            ValueType::I32,
        ]),

        // Ref test/cast
        "ref.test" | "ref.cast" | "ref.cast_null" => Some(vec![ValueType::Unknown]),
        "br_on_cast" | "br_on_cast_fail" => Some(vec![ValueType::Unknown]),

        // Exceptions
        "throw_ref" => Some(vec![ValueType::Unknown]), // exnref

        // Bulk memory
        "memory.copy" => Some(vec![ValueType::I32, ValueType::I32, ValueType::I32]),
        "memory.fill" => Some(vec![ValueType::I32, ValueType::I32, ValueType::I32]),
        "memory.init" => Some(vec![ValueType::I32, ValueType::I32, ValueType::I32]),
        "data.drop" | "elem.drop" => Some(vec![]),

        // Table operations
        "table.get" => Some(vec![ValueType::I32]),
        "table.set" => Some(vec![ValueType::I32, ValueType::Unknown]),
        "table.size" => Some(vec![]),
        "table.grow" => Some(vec![ValueType::Unknown, ValueType::I32]),
        "table.fill" => Some(vec![ValueType::I32, ValueType::Unknown, ValueType::I32]),
        "table.copy" => Some(vec![ValueType::I32, ValueType::I32, ValueType::I32]),
        "table.init" => Some(vec![ValueType::I32, ValueType::I32, ValueType::I32]),

        // Atomic fence — no operands
        "atomic.fence" => Some(vec![]),

        // Wait/notify
        "memory.atomic.wait32" => Some(vec![ValueType::I32, ValueType::I32, ValueType::I64]),
        "memory.atomic.wait64" => Some(vec![ValueType::I32, ValueType::I64, ValueType::I64]),
        "memory.atomic.notify" => Some(vec![ValueType::I32, ValueType::I32]),

        // Br_on_null/non_null
        "br_on_null" | "br_on_non_null" => Some(vec![ValueType::Unknown]),

        // Scalar binary operations — 2 operands of prefix type
        name if is_binary_scalar(name) => {
            if let Some(ty) = type_from_prefix(name) {
                Some(vec![ty.clone(), ty])
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
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    // call_indirect consumes: param_types... + i32 (table index)
    let type_ref = get_index_from_node(node, source);
    if let Some(ref type_ref) = type_ref {
        let type_def = symbols.get_type_by_name(type_ref).or_else(|| {
            type_ref
                .parse::<usize>()
                .ok()
                .and_then(|idx| symbols.get_type_by_index(idx))
        });
        if let Some(type_def) = type_def {
            if let TypeKind::Func { params, .. } = &type_def.kind {
                let mut types = params.clone();
                types.push(ValueType::I32); // table index
                return types;
            }
        }
    }
    // Fallback: use dynamic operand count with Unknown types
    let count = get_dynamic_operand_count(node, instr_name, symbols, source);
    vec![ValueType::Unknown; count]
}

/// Get branch target depth from a br/br_if instruction node
fn get_branch_depth(node: &Node, source: &str, checker: &TypeChecker) -> Option<usize> {
    let index = get_index_from_node(node, source)?;
    // Try as numeric index first
    if let Ok(depth) = index.parse::<usize>() {
        if depth < checker.ctrl_depth() {
            return Some(depth);
        }
    }
    // Named labels not yet supported for depth resolution
    None
}

/// Get the last (default) branch depth from a br_table instruction
fn get_last_branch_depth(node: &Node, source: &str, checker: &TypeChecker) -> Option<usize> {
    // br_table has multiple indices; the last one is the default
    let mut last_index = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "index" || kind == "identifier" || kind == "nat" {
            last_index = Some(source[child.byte_range()].trim().to_string());
        }
        if kind.starts_with("op_") {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "index" || inner_kind == "identifier" || inner_kind == "nat" {
                    last_index = Some(source[inner_child.byte_range()].trim().to_string());
                }
            }
        }
    }
    if let Some(ref idx_str) = last_index {
        if let Ok(depth) = idx_str.parse::<usize>() {
            if depth < checker.ctrl_depth() {
                return Some(depth);
            }
        }
    }
    None
}

/// Get the operand count for a dynamic instruction
pub fn get_dynamic_operand_count(
    node: &Node,
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> usize {
    match instr_name {
        "call" | "return_call" => {
            if let Some(func_ref) = get_index_from_node(node, source) {
                if let Some(func) = symbols.get_function_by_name(&func_ref) {
                    return func.parameters.len();
                } else if let Ok(idx) = func_ref.parse::<usize>() {
                    if let Some(func) = symbols.get_function_by_index(idx) {
                        return func.parameters.len();
                    }
                }
            }
            0
        }
        "call_ref" | "return_call_ref" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(type_def) = symbols.get_type_by_name(&type_ref) {
                    if let TypeKind::Func { params, .. } = &type_def.kind {
                        return params.len() + 1;
                    }
                } else if let Ok(idx) = type_ref.parse::<usize>() {
                    if let Some(type_def) = symbols.get_type_by_index(idx) {
                        if let TypeKind::Func { params, .. } = &type_def.kind {
                            return params.len() + 1;
                        }
                    }
                }
            }
            1
        }
        "call_indirect" | "return_call_indirect" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(type_def) = symbols.get_type_by_name(&type_ref) {
                    if let TypeKind::Func { params, .. } = &type_def.kind {
                        return params.len() + 1;
                    }
                } else if let Ok(idx) = type_ref.parse::<usize>() {
                    if let Some(type_def) = symbols.get_type_by_index(idx) {
                        if let TypeKind::Func { params, .. } = &type_def.kind {
                            return params.len() + 1;
                        }
                    }
                }
            }
            1
        }
        "struct.new" => {
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(type_def) = symbols.get_type_by_name(&type_ref) {
                    if let TypeKind::Struct { fields } = &type_def.kind {
                        return fields.len();
                    }
                } else if let Ok(idx) = type_ref.parse::<usize>() {
                    if let Some(type_def) = symbols.get_type_by_index(idx) {
                        if let TypeKind::Struct { fields } = &type_def.kind {
                            return fields.len();
                        }
                    }
                }
            }
            0
        }
        "throw" => {
            if let Some(tag_ref) = get_index_from_node(node, source) {
                if let Some(tag) = symbols.get_tag_by_name(&tag_ref) {
                    return tag.params.len();
                } else if let Ok(idx) = tag_ref.parse::<usize>() {
                    if let Some(tag) = symbols.get_tag_by_index(idx) {
                        return tag.params.len();
                    }
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
pub fn get_index_from_node(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "index" || kind == "identifier" {
            return Some(source[child.byte_range()].trim().to_string());
        }
        if kind == "type_use" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "index" || inner_kind == "identifier" {
                    return Some(source[inner_child.byte_range()].trim().to_string());
                }
            }
        }
        if kind.starts_with("op_") {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "index" || inner_kind == "identifier" {
                    return Some(source[inner_child.byte_range()].trim().to_string());
                }
            }
        }
    }
    None
}

/// Infer the result type(s) of an instruction from its name and context
pub fn infer_instruction_result_types(
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

    let type_from_prefix = |name: &str| -> Option<ValueType> {
        if name.starts_with("i32.") {
            return Some(ValueType::I32);
        }
        if name.starts_with("i64.") {
            return Some(ValueType::I64);
        }
        if name.starts_with("f32.") {
            return Some(ValueType::F32);
        }
        if name.starts_with("f64.") {
            return Some(ValueType::F64);
        }
        if name.starts_with("v128.") {
            return Some(ValueType::V128);
        }
        None
    };

    let is_scalar_comparison = |name: &str| -> bool {
        let is_simd = name.contains("x2.")
            || name.contains("x4.")
            || name.contains("x8.")
            || name.contains("x16.");
        if is_simd {
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

        "memory.size" | "memory.grow" => vec![ValueType::I32],

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

        "drop" | "local.set" | "global.set" | "return" | "br" | "unreachable" | "nop" => vec![],

        "ref.null" => vec![ValueType::Unknown],
        "ref.func" => vec![ValueType::Funcref],
        "ref.is_null" => vec![ValueType::I32],

        "select" => vec![ValueType::Unknown],
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

        name if type_from_prefix(name).is_some() => {
            type_from_prefix(name).map(|t| vec![t]).unwrap_or_default()
        }

        _ => vec![ValueType::Unknown],
    }
}

/// Get the type of a local variable from an instruction node
pub fn get_local_type_from_node(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Option<ValueType> {
    let index = get_index_from_node(node, source)?;
    let func_line = node.start_position().row as u32;
    let func = symbols.find_function_containing_line(func_line)?;

    let name_to_check = index.strip_prefix('$').unwrap_or(&index);

    for param in &func.parameters {
        if let Some(param_name) = &param.name {
            let param_name_stripped = param_name.strip_prefix('$').unwrap_or(param_name);
            if param_name_stripped == name_to_check || param_name == &index {
                return Some(param.param_type.clone());
            }
        }
    }

    for local in &func.locals {
        if let Some(local_name) = &local.name {
            let local_name_stripped = local_name.strip_prefix('$').unwrap_or(local_name);
            if local_name_stripped == name_to_check || local_name == &index {
                return Some(local.var_type.clone());
            }
        }
    }

    if let Ok(idx) = index.parse::<usize>() {
        if idx < func.parameters.len() {
            return Some(func.parameters[idx].param_type.clone());
        }
        let local_idx = idx - func.parameters.len();
        if local_idx < func.locals.len() {
            return Some(func.locals[local_idx].var_type.clone());
        }
    }

    None
}

/// Get the type of a global from an instruction node
pub fn get_global_type_from_node(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Option<ValueType> {
    let index = get_index_from_node(node, source)?;

    if let Some(global) = symbols.get_global_by_name(&index) {
        return Some(global.var_type.clone());
    }
    if let Ok(idx) = index.parse::<usize>() {
        if let Some(global) = symbols.get_global_by_index(idx) {
            return Some(global.var_type.clone());
        }
    }
    None
}

/// Get result types for a call instruction
pub fn get_call_result_types(node: &Node, symbols: &SymbolTable, source: &str) -> Vec<ValueType> {
    if let Some(func_ref) = get_index_from_node(node, source) {
        if let Some(func) = symbols.get_function_by_name(&func_ref) {
            return func.results.clone();
        } else if let Ok(idx) = func_ref.parse::<usize>() {
            if let Some(func) = symbols.get_function_by_index(idx) {
                return func.results.clone();
            }
        }
    }
    vec![]
}

/// Get result types for a call_ref instruction
pub fn get_call_ref_result_types(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    if let Some(type_ref) = get_index_from_node(node, source) {
        if let Some(type_def) = symbols.get_type_by_name(&type_ref) {
            if let TypeKind::Func { results, .. } = &type_def.kind {
                return results.clone();
            }
        } else if let Ok(idx) = type_ref.parse::<usize>() {
            if let Some(type_def) = symbols.get_type_by_index(idx) {
                if let TypeKind::Func { results, .. } = &type_def.kind {
                    return results.clone();
                }
            }
        }
    }
    vec![]
}

/// Get result types for a call_indirect instruction
pub fn get_call_indirect_result_types(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "type_use" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                #[cfg(feature = "native")]
                let type_child_kind = type_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let type_child_kind = type_child.kind();

                if type_child_kind == "index" || type_child_kind == "identifier" {
                    let type_ref = source[type_child.byte_range()].trim();
                    if let Some(type_def) = symbols.get_type_by_name(type_ref) {
                        if let TypeKind::Func { results, .. } = &type_def.kind {
                            return results.clone();
                        }
                    } else if let Ok(idx) = type_ref.parse::<usize>() {
                        if let Some(type_def) = symbols.get_type_by_index(idx) {
                            if let TypeKind::Func { results, .. } = &type_def.kind {
                                return results.clone();
                            }
                        }
                    }
                }
            }
        }
        if kind == "index" {
            let type_ref = source[child.byte_range()].trim();
            if let Some(type_def) = symbols.get_type_by_name(type_ref) {
                if let TypeKind::Func { results, .. } = &type_def.kind {
                    return results.clone();
                }
            } else if let Ok(idx) = type_ref.parse::<usize>() {
                if let Some(type_def) = symbols.get_type_by_index(idx) {
                    if let TypeKind::Func { results, .. } = &type_def.kind {
                        return results.clone();
                    }
                }
            }
        }
    }
    vec![ValueType::Unknown]
}

/// Get instruction info from a folded expression
/// Returns (instruction_name, explicit_operand_count)
pub fn get_folded_expr_info(expr: &Node, source: &str) -> Option<(String, usize)> {
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "expr1" {
            return get_expr1_wrapper_info(&child, source);
        } else if kind.starts_with("expr1_") {
            return get_expr1_info(&child, source);
        }
    }
    None
}

/// Get instruction info from an expr1 wrapper node
fn get_expr1_wrapper_info(expr1: &Node, source: &str) -> Option<(String, usize)> {
    let mut cursor = expr1.walk();
    for child in expr1.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind.starts_with("expr1_") {
            return get_expr1_info(&child, source);
        }
    }
    None
}

/// Get instruction info from an expr1_* node
fn get_expr1_info(expr1: &Node, source: &str) -> Option<(String, usize)> {
    let mut expr_cursor = expr1.walk();
    let explicit_operands = expr1
        .children(&mut expr_cursor)
        .filter(|c| {
            #[cfg(feature = "native")]
            let kind = c.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let kind = c.kind();
            kind == "expr"
        })
        .count();

    let mut cursor = expr1.walk();
    for child in expr1.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "instr_plain" {
            if let Some(name) = get_instruction_name(&child, source) {
                return Some((name, explicit_operands));
            }
        }
        if kind == "call_indirect" {
            return Some(("call_indirect".to_string(), explicit_operands));
        }
        if kind == "return_call_indirect" {
            return Some(("return_call_indirect".to_string(), explicit_operands));
        }
        if kind == "instr_call" {
            let text = &source[child.byte_range()];
            let first_token = text.split_whitespace().next().unwrap_or("");
            if !first_token.is_empty() {
                return Some((first_token.to_string(), explicit_operands));
            }
        }
    }
    None
}

/// Get the expected operand count for an instruction (fixed or dynamic)
pub fn get_expected_operands_by_name(
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
    expr: &Node,
) -> usize {
    if let Some(arity) = lookup_instruction_arity(instr_name) {
        match arity.operand_mode {
            OperandMode::Fixed(n) => n,
            OperandMode::Dynamic => {
                get_dynamic_operand_count_from_expr(expr, instr_name, symbols, source)
            }
        }
    } else {
        0
    }
}

/// Get dynamic operand count by finding the instr_plain inside an expr
fn get_dynamic_operand_count_from_expr(
    expr: &Node,
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> usize {
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind.starts_with("expr1_") {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "instr_plain" {
                    return get_dynamic_operand_count(&inner_child, instr_name, symbols, source);
                }
            }
        }
    }
    0
}

/// Process a folded expression: consume implicit stack operands, produce results
fn process_folded_expr(
    expr: &Node,
    source: &str,
    symbols: &SymbolTable,
    checker: &mut TypeChecker,
) {
    if let Some((instr_name, explicit_operands)) = get_folded_expr_info(expr, source) {
        // Handle tail call instructions
        if matches!(
            instr_name.as_str(),
            "return_call" | "return_call_ref" | "return_call_indirect"
        ) {
            let expected = get_expected_operands_by_name(&instr_name, symbols, source, expr);
            let from_stack = expected.saturating_sub(explicit_operands);
            if from_stack > 0 {
                let consumed = vec![ValueType::Unknown; from_stack];
                checker.pop_vals_for_instr(&consumed, expr, &instr_name);
            }
            if let Some(diag) =
                validate_tail_call_in_folded_expr(expr, &instr_name, symbols, source)
            {
                checker.diagnostics.push(diag);
            }
            checker.mark_unreachable();
            return;
        }

        // Handle other terminating instructions
        if is_terminating_instruction(&instr_name) {
            checker.mark_unreachable();
            return;
        }

        // Calculate operands needed from the stack
        let expected = get_expected_operands_by_name(&instr_name, symbols, source, expr);
        let from_stack = expected.saturating_sub(explicit_operands);

        if from_stack > 0 {
            let consumed = vec![ValueType::Unknown; from_stack];
            checker.pop_vals_for_instr(&consumed, expr, &instr_name);
        }
    }

    // For block expressions, consume param types from outer stack
    let block_params = get_block_param_types_from_expr(expr, source);
    if !block_params.is_empty() {
        checker.pop_vals(&block_params, expr);
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

/// Get result types from a folded expression
pub fn get_expr_result_types(expr: &Node, source: &str, symbols: &SymbolTable) -> Vec<ValueType> {
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind.starts_with("expr1_") {
            return get_expr1_result_types(&child, source, symbols);
        }
        if kind == "expr1" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

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
    #[cfg(feature = "native")]
    let kind = expr1.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = expr1.kind();

    match kind.as_ref() {
        "expr1_plain" => {
            let mut cursor = expr1.walk();
            for child in expr1.children(&mut cursor) {
                #[cfg(feature = "native")]
                let child_kind = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let child_kind = child.kind();

                if child_kind == "instr_plain" {
                    if let Some(instr_name) = get_instruction_name(&child, source) {
                        return infer_instruction_result_types(
                            &instr_name,
                            &child,
                            symbols,
                            source,
                        );
                    }
                }
            }
            vec![ValueType::Unknown]
        }
        "expr1_block" | "expr1_loop" => get_block_result_types(expr1, source),
        "expr1_if" => get_block_result_types(expr1, source),
        "expr1_try" | "expr1_try_table" => get_block_result_types(expr1, source),
        "expr1_call" => {
            let mut cursor = expr1.walk();
            for child in expr1.children(&mut cursor) {
                #[cfg(feature = "native")]
                let child_kind = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let child_kind = child.kind();

                if child_kind == "call_indirect" || child_kind == "return_call_indirect" {
                    return get_call_indirect_result_types(expr1, symbols, source);
                }
            }
            if let Some(func_ref) = get_index_from_expr1_call(expr1, source) {
                if let Some(func) = symbols.get_function_by_name(&func_ref) {
                    return func.results.clone();
                } else if let Ok(idx) = func_ref.parse::<usize>() {
                    if let Some(func) = symbols.get_function_by_index(idx) {
                        return func.results.clone();
                    }
                }
            }
            vec![]
        }
        _ => vec![ValueType::Unknown],
    }
}

/// Get function reference from expr1_call node
pub fn get_index_from_expr1_call(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "index" || kind == "identifier" {
            return Some(source[child.byte_range()].trim().to_string());
        }
    }
    None
}

/// Get result types from a block/loop/if node
pub fn get_block_result_types(block_node: &Node, source: &str) -> Vec<ValueType> {
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "func_type_results" {
            return parse_func_type_results(&child, source);
        }
        if kind == "block_type" {
            return parse_result_types(&child, source);
        }
        if kind == "block_block" || kind == "loop_block" || kind == "if_block" || kind == "block_if"
        {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "func_type_results" {
                    return parse_func_type_results(&inner_child, source);
                }
            }
        }
    }
    vec![]
}

/// Get block param types from a folded expression (expr node)
fn get_block_param_types_from_expr(expr: &Node, source: &str) -> Vec<ValueType> {
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "expr1_block" || kind == "expr1_loop" || kind == "expr1_if" {
            return get_block_param_types(&child, source);
        }
        if kind == "expr1" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "expr1_block"
                    || inner_kind == "expr1_loop"
                    || inner_kind == "expr1_if"
                {
                    return get_block_param_types(&inner_child, source);
                }
            }
        }
    }
    vec![]
}

/// Get param types from a block/loop/if node
pub fn get_block_param_types(block_node: &Node, source: &str) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "func_type_params_many" {
            types.extend(parse_func_type_results(&child, source));
        }
        if kind == "block_block" || kind == "loop_block" || kind == "if_block" || kind == "block_if"
        {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "func_type_params_many" {
                    types.extend(parse_func_type_results(&inner_child, source));
                }
            }
        }
    }
    types
}

/// Parse result types from a func_type_results node
pub fn parse_func_type_results(results_node: &Node, source: &str) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut cursor = results_node.walk();
    for child in results_node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "value_type" || kind == "ref_type" {
            let type_text = &source[child.byte_range()];
            if let Some(t) = ValueType::try_parse(type_text.trim()) {
                types.push(t);
            } else {
                types.push(ValueType::Unknown);
            }
        }
    }
    types
}

/// Parse result types from a block_type node
pub fn parse_result_types(block_type: &Node, source: &str) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut cursor = block_type.walk();

    for child in block_type.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind == "func_type_results" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind == "value_type" || inner_kind == "ref_type" {
                    let type_text = &source[inner_child.byte_range()];
                    if let Some(t) = ValueType::try_parse(type_text.trim()) {
                        types.push(t);
                    } else {
                        types.push(ValueType::Unknown);
                    }
                }
            }
        }
        if kind == "value_type" || kind == "ref_type" {
            let type_text = &source[child.byte_range()];
            if let Some(t) = ValueType::try_parse(type_text.trim()) {
                types.push(t);
            } else {
                types.push(ValueType::Unknown);
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
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind.starts_with("expr1_") {
            if kind == "expr1_call" {
                return validate_tail_call_return_types(&child, instr_name, symbols, source);
            }
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

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
                #[cfg(feature = "native")]
                let mid_kind = mid_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let mid_kind = mid_child.kind();

                if mid_kind == "expr1_call" {
                    return validate_tail_call_return_types(
                        &mid_child, instr_name, symbols, source,
                    );
                }
                if mid_kind.starts_with("expr1_") {
                    let mut inner_cursor = mid_child.walk();
                    for inner_child in mid_child.children(&mut inner_cursor) {
                        #[cfg(feature = "native")]
                        let inner_kind = inner_child.kind();
                        #[cfg(all(feature = "wasm", not(feature = "native")))]
                        let inner_kind = inner_child.kind();

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
    let inner: Vec<_> = types.iter().map(|t| t.to_string()).collect();
    format!("({})", inner.join(", "))
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
    let enclosing_results = &enclosing_func.results;

    let callee_results = match instr_name {
        "return_call" => get_call_result_types(node, symbols, source),
        "return_call_ref" => get_call_ref_result_types(node, symbols, source),
        "return_call_indirect" => get_call_indirect_result_types(node, symbols, source),
        _ => return None,
    };

    if callee_results.is_empty() || callee_results.contains(&ValueType::Unknown) {
        return None;
    }

    if callee_results != *enclosing_results {
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
                    "Tail call return type mismatch: '{}' returns {} but enclosing function returns {}",
                    label,
                    format_types(&callee_results),
                    format_types(enclosing_results),
                ),
            )
            .with_code("tail-call-type-mismatch"),
        );
    }

    None
}

/// Create a diagnostic for stack underflow
pub fn create_stack_underflow_diagnostic(
    node: &Node,
    instr_name: &str,
    needed: usize,
    available: usize,
) -> Diagnostic {
    let range = node_to_range(node);
    let value_word = if needed == 1 { "value" } else { "values" };

    Diagnostic::error(
        range,
        format!(
            "Stack underflow: '{}' requires {} {} but only {} available on stack",
            instr_name, needed, value_word, available
        ),
    )
    .with_code("stack-underflow")
}
