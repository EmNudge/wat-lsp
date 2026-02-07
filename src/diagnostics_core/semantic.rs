//! Core semantic diagnostic functions shared between native and WASM builds.
//!
//! This module provides platform-agnostic functions for:
//! - Stack tracking through instruction lists
//! - Type inference for instructions
//! - Return type validation

// Allow useless_asref because kind.as_ref() is needed for WASM (String -> &str)
// but is a no-op for native (&str -> &str)
#![allow(clippy::useless_asref)]

use crate::core::types::Diagnostic;
use crate::instruction_metadata::{
    get_instruction_arity_map, infer_simd_instruction_arity, is_terminating_instruction,
    InstructionArity, OperandMode,
};
use crate::symbols::{SymbolTable, TypeKind, ValueType};
use crate::utils::node_to_range;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::{sequence_always_terminates, StackState};

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

// Lazy static initialization for instruction arity map
static INSTRUCTION_ARITY: OnceLock<HashMap<&'static str, InstructionArity>> = OnceLock::new();

pub fn get_arity_map() -> &'static HashMap<&'static str, InstructionArity> {
    INSTRUCTION_ARITY.get_or_init(get_instruction_arity_map)
}

/// Track stack state through an instruction list and report underflow/type errors
/// If expected_results is None, skip return type validation (e.g., when function uses type reference)
/// Returns diagnostics as core::types::Diagnostic
pub fn track_stack_in_instr_list(
    instr_list: &Node,
    source: &str,
    symbols: &SymbolTable,
    expected_results: Option<&[ValueType]>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut stack = StackState::new();
    let arity_map = get_arity_map();

    let mut cursor = instr_list.walk();
    for child in instr_list.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        match kind.as_ref() {
            "instr" => {
                // instr wraps instr_plain, instr_block, etc.
                process_instr_node(
                    &child,
                    source,
                    symbols,
                    arity_map,
                    &mut stack,
                    &mut diagnostics,
                );
            }
            "instr_plain" => {
                // Direct instr_plain (shouldn't happen but handle anyway)
                if let Some(instr_name) = get_instruction_name(&child, source) {
                    process_instruction(
                        &child,
                        &instr_name,
                        &mut stack,
                        symbols,
                        source,
                        arity_map,
                        &mut diagnostics,
                    );
                }
            }
            "expr" => {
                // Folded expression - consume stack operands and produce results
                process_folded_expr(
                    &child,
                    source,
                    symbols,
                    arity_map,
                    &mut stack,
                    &mut diagnostics,
                );
            }
            "instr_block" | "instr_loop" => {
                // Block instructions create a new stack frame
                // After a block completes, it pushes its result values
                // But only if the block can actually fall through
                // Check if this is a block_if (linear if format: if ... end)
                // which needs to consume a condition value from the stack
                if contains_block_if(&child) {
                    if let Err(available) = stack.consume(1) {
                        let diagnostic =
                            create_stack_underflow_diagnostic(&child, "if", 1, available);
                        diagnostics.push(diagnostic);
                    }
                }
                // Consume block param types from the outer stack (multi-value proposal)
                let params = get_block_param_types(&child, source);
                if !params.is_empty() {
                    if let Err(available) = stack.consume(params.len()) {
                        let block_name = if contains_block_if(&child) {
                            "if"
                        } else {
                            "block"
                        };
                        let diagnostic = create_stack_underflow_diagnostic(
                            &child,
                            block_name,
                            params.len(),
                            available,
                        );
                        diagnostics.push(diagnostic);
                    }
                }
                if !sequence_always_terminates(&child, source) {
                    let results = get_block_result_types(&child, source);
                    stack.produce(results);
                } else {
                    stack.mark_uncertain();
                }
            }
            "instr_if" => {
                // if consumes condition (1 value) and produces result values
                if let Err(available) = stack.consume(1) {
                    let diagnostic = create_stack_underflow_diagnostic(&child, "if", 1, available);
                    diagnostics.push(diagnostic);
                }
                // Consume block param types from the outer stack (multi-value proposal)
                let params = get_block_param_types(&child, source);
                if !params.is_empty() {
                    if let Err(available) = stack.consume(params.len()) {
                        let diagnostic = create_stack_underflow_diagnostic(
                            &child,
                            "if",
                            params.len(),
                            available,
                        );
                        diagnostics.push(diagnostic);
                    }
                }
                // Only produce results if the if can fall through
                if !sequence_always_terminates(&child, source) {
                    let results = get_block_result_types(&child, source);
                    stack.produce(results);
                } else {
                    stack.mark_uncertain();
                }
            }
            "instr_call" => {
                // Folded call: (call $fn args...) - consume stack operands and produce results
                process_folded_expr(
                    &child,
                    source,
                    symbols,
                    arity_map,
                    &mut stack,
                    &mut diagnostics,
                );
            }
            _ => {
                // Other node types (comments, etc.) - ignore
            }
        }
    }

    // Check return types at end of function (only if we know the expected results)
    if !stack.is_uncertain() {
        if let Some(expected) = expected_results {
            check_return_types(&stack, expected, instr_list, &mut diagnostics);
        }
    }

    diagnostics
}

/// Process an 'instr' wrapper node
fn process_instr_node(
    instr_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    arity_map: &HashMap<&'static str, InstructionArity>,
    stack: &mut StackState,
    diagnostics: &mut Vec<Diagnostic>,
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
                    process_instruction(
                        &child,
                        &instr_name,
                        stack,
                        symbols,
                        source,
                        arity_map,
                        diagnostics,
                    );
                }
            }
            "expr" => {
                // Folded expression - consume stack operands and produce results
                process_folded_expr(&child, source, symbols, arity_map, stack, diagnostics);
            }
            "instr_block" | "instr_loop" => {
                // Check if this is a block_if (linear if format: if ... end)
                if contains_block_if(&child) {
                    if let Err(available) = stack.consume(1) {
                        let diagnostic =
                            create_stack_underflow_diagnostic(&child, "if", 1, available);
                        diagnostics.push(diagnostic);
                    }
                }
                // Consume block param types from the outer stack (multi-value proposal)
                let params = get_block_param_types(&child, source);
                if !params.is_empty() {
                    if let Err(available) = stack.consume(params.len()) {
                        let block_name = if contains_block_if(&child) {
                            "if"
                        } else {
                            "block"
                        };
                        let diagnostic = create_stack_underflow_diagnostic(
                            &child,
                            block_name,
                            params.len(),
                            available,
                        );
                        diagnostics.push(diagnostic);
                    }
                }
                if !sequence_always_terminates(&child, source) {
                    let results = get_block_result_types(&child, source);
                    stack.produce(results);
                } else {
                    stack.mark_uncertain();
                }
            }
            "instr_if" => {
                if let Err(available) = stack.consume(1) {
                    let diagnostic = create_stack_underflow_diagnostic(&child, "if", 1, available);
                    diagnostics.push(diagnostic);
                }
                // Consume block param types from the outer stack (multi-value proposal)
                let params = get_block_param_types(&child, source);
                if !params.is_empty() {
                    if let Err(available) = stack.consume(params.len()) {
                        let diagnostic = create_stack_underflow_diagnostic(
                            &child,
                            "if",
                            params.len(),
                            available,
                        );
                        diagnostics.push(diagnostic);
                    }
                }
                if !sequence_always_terminates(&child, source) {
                    let results = get_block_result_types(&child, source);
                    stack.produce(results);
                } else {
                    stack.mark_uncertain();
                }
            }
            "instr_call" => {
                // Folded call - consume stack operands and produce results
                process_folded_expr(&child, source, symbols, arity_map, stack, diagnostics);
            }
            _ => {}
        }
    }
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

/// Process an instruction: consume operands, check for underflow, produce results
fn process_instruction(
    node: &Node,
    instr_name: &str,
    stack: &mut StackState,
    symbols: &SymbolTable,
    source: &str,
    arity_map: &HashMap<&'static str, InstructionArity>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Handle control flow that makes subsequent code dead
    if is_terminating_instruction(instr_name) {
        stack.mark_uncertain();
        return;
    }

    // Look up instruction arity (explicit map, then SIMD pattern fallback)
    let (consumes, produces) = if let Some(arity) = arity_map.get(instr_name) {
        let c = match arity.operand_mode {
            OperandMode::Fixed(n) => n,
            OperandMode::Dynamic => get_dynamic_operand_count(node, instr_name, symbols, source),
        };
        (c, arity.produces)
    } else if let Some((c, p)) = infer_simd_instruction_arity(instr_name) {
        (c, p)
    } else {
        // Unknown instruction - skip (might be a newer instruction we don't know about)
        return;
    };

    // Try to consume operands
    if let Err(available) = stack.consume(consumes) {
        let diagnostic = create_stack_underflow_diagnostic(node, instr_name, consumes, available);
        diagnostics.push(diagnostic);
    }

    // Produce results with actual types
    let result_types = infer_instruction_result_types(instr_name, node, symbols, source);
    // If type inference returned fewer types than `produces`, pad with Unknown
    let mut types = result_types;
    while types.len() < produces {
        types.push(ValueType::Unknown);
    }
    stack.produce(types);
}

/// Get the operand count for a dynamic instruction
pub fn get_dynamic_operand_count(
    node: &Node,
    instr_name: &str,
    symbols: &SymbolTable,
    source: &str,
) -> usize {
    match instr_name {
        "call" => {
            // Get function index/name and look up parameter count
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
            // Get type index and look up parameter count + 1 for funcref
            if let Some(type_ref) = get_index_from_node(node, source) {
                if let Some(type_def) = symbols.get_type_by_name(&type_ref) {
                    if let TypeKind::Func { params, .. } = &type_def.kind {
                        return params.len() + 1; // params + funcref
                    }
                } else if let Ok(idx) = type_ref.parse::<usize>() {
                    if let Some(type_def) = symbols.get_type_by_index(idx) {
                        if let TypeKind::Func { params, .. } = &type_def.kind {
                            return params.len() + 1;
                        }
                    }
                }
            }
            1 // At least the funcref
        }
        "struct.new" => {
            // Get type and look up field count
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
            // Get tag and look up parameter count
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
            // br_if consumes condition (1) + potential multi-value args
            // For simplicity, just handle the condition for br_if
            if instr_name == "br_if" {
                1 // condition
            } else {
                0 // br itself doesn't consume extra (but terminates)
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
        // Also check inside type_use nodes (e.g. call_ref (type $t))
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
        // Also check inside op_index nodes
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
    // First check the instruction metadata for the production count
    // This handles many instructions that produce 0 values
    // BUT skip instructions with dynamic result counts (call, call_ref, etc.)
    // which have produces=0 in metadata but actually look up results dynamically
    let arity_map = get_arity_map();
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
        if let Some(arity) = arity_map.get(instr_name) {
            if arity.produces == 0 {
                return vec![];
            }
        }
    }

    // Extract type from instruction prefix (i32.*, i64.*, f32.*, f64.*, v128.*)
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

    // Check if instruction is a scalar comparison (returns i32)
    // Excludes SIMD comparisons which return v128 masks
    let is_scalar_comparison = |name: &str| -> bool {
        // SIMD comparisons contain x2, x4, x8, x16 (e.g., i32x4.eq, f64x2.lt)
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
        // Constants - type from name
        "i32.const" => vec![ValueType::I32],
        "i64.const" => vec![ValueType::I64],
        "f32.const" => vec![ValueType::F32],
        "f64.const" => vec![ValueType::F64],
        "v128.const" => vec![ValueType::V128],

        // Scalar comparisons always produce i32
        name if is_scalar_comparison(name) => vec![ValueType::I32],

        // Conversions - result type varies
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

        // Sign extension (result type matches prefix)
        "i32.extend8_s" | "i32.extend16_s" => vec![ValueType::I32],

        // Memory operations - result type from prefix for loads
        name if name.contains(".load") => {
            type_from_prefix(name).map(|t| vec![t]).unwrap_or_default()
        }

        // Memory stores produce nothing
        name if name.contains(".store") => vec![],

        // Memory size/grow
        "memory.size" | "memory.grow" => vec![ValueType::I32],

        // Local/global operations - need symbol lookup
        "local.get" | "local.tee" => get_local_type_from_node(node, symbols, source)
            .map(|t| vec![t])
            .unwrap_or_else(|| vec![ValueType::Unknown]),

        "global.get" => get_global_type_from_node(node, symbols, source)
            .map(|t| vec![t])
            .unwrap_or_else(|| vec![ValueType::Unknown]),

        // Call instructions - need function signature lookup
        "call" => get_call_result_types(node, symbols, source),
        "call_ref" | "return_call_ref" => get_call_ref_result_types(node, symbols, source),
        "call_indirect" | "return_call_indirect" => {
            get_call_indirect_result_types(node, symbols, source)
        }

        // These produce nothing
        "drop" | "local.set" | "global.set" | "return" | "br" | "unreachable" | "nop" => vec![],

        // Reference instructions
        "ref.null" => vec![ValueType::Unknown], // Type depends on argument
        "ref.func" => vec![ValueType::Funcref],
        "ref.is_null" => vec![ValueType::I32],

        // Select - we don't track consumed types, so use Unknown
        "select" => vec![ValueType::Unknown],

        // br_if consumes condition but may pass through values
        "br_if" => vec![],

        // SIMD reduction operations that return i32
        "v128.any_true" => vec![ValueType::I32],
        name if name.ends_with(".all_true") || name.ends_with(".bitmask") => vec![ValueType::I32],

        // SIMD lane extract operations that return the lane type
        name if name.ends_with(".extract_lane_s")
            || name.ends_with(".extract_lane_u")
            || name.ends_with(".extract_lane") =>
        {
            // Extract type from the prefix (e.g., i32x4.extract_lane -> i32)
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

        // Arithmetic/bitwise - type from prefix
        name if type_from_prefix(name).is_some() => {
            // Most operations with type prefix produce that type
            type_from_prefix(name).map(|t| vec![t]).unwrap_or_default()
        }

        // Unknown instruction - use Unknown type
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

    // Try as name first (with or without $)
    let name_to_check = index.strip_prefix('$').unwrap_or(&index);

    // Check parameters
    for param in &func.parameters {
        if let Some(param_name) = &param.name {
            let param_name_stripped = param_name.strip_prefix('$').unwrap_or(param_name);
            if param_name_stripped == name_to_check || param_name == &index {
                return Some(param.param_type.clone());
            }
        }
    }

    // Check locals
    for local in &func.locals {
        if let Some(local_name) = &local.name {
            let local_name_stripped = local_name.strip_prefix('$').unwrap_or(local_name);
            if local_name_stripped == name_to_check || local_name == &index {
                return Some(local.var_type.clone());
            }
        }
    }

    // Try as numeric index
    if let Ok(idx) = index.parse::<usize>() {
        // Parameters come first, then locals
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
/// call_indirect uses (type $t) syntax where $t is the function type
pub fn get_call_indirect_result_types(
    node: &Node,
    symbols: &SymbolTable,
    source: &str,
) -> Vec<ValueType> {
    // call_indirect has a type_use child: (call_indirect (type 0) ...)
    // We need to find the type index inside the type_use
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        // Look for type_use node which contains (type $idx)
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
        // Also check for direct index child (numeric type reference)
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
    // Fallback: return Unknown type if we can't determine
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

        // Handle both expr1 wrapper and direct expr1_* nodes
        if kind == "expr1" {
            // expr1 wraps expr1_plain, expr1_call, etc.
            return get_expr1_wrapper_info(&child, source);
        } else if kind.starts_with("expr1_") {
            return get_expr1_info(&child, source);
        }
    }
    None
}

/// Get instruction info from an expr1 wrapper node (which contains expr1_plain, etc.)
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

/// Get instruction info from an expr1_* node (expr1_plain, expr1_call, etc.)
fn get_expr1_info(expr1: &Node, source: &str) -> Option<(String, usize)> {
    // Count explicit expr children (these are operands provided inline)
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

    // Get instruction name from instr_plain child
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
        // Handle call_indirect and return_call_indirect directly
        if kind == "call_indirect" {
            return Some(("call_indirect".to_string(), explicit_operands));
        }
        if kind == "return_call_indirect" {
            return Some(("return_call_indirect".to_string(), explicit_operands));
        }
        // Handle instr_call which might contain call/call_indirect
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
    arity_map: &HashMap<&'static str, InstructionArity>,
    expr: &Node,
) -> usize {
    if let Some(arity) = arity_map.get(instr_name) {
        match arity.operand_mode {
            OperandMode::Fixed(n) => n,
            OperandMode::Dynamic => {
                // For dynamic instructions, we need to find the instr_plain to get param info
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
    // Navigate to find instr_plain inside expr > expr1_* > instr_plain
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

/// Process a folded expression: consume implicit stack operands, check for underflow, produce results
fn process_folded_expr(
    expr: &Node,
    source: &str,
    symbols: &SymbolTable,
    arity_map: &HashMap<&'static str, InstructionArity>,
    stack: &mut StackState,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Get instruction info from the expression
    if let Some((instr_name, explicit_operands)) = get_folded_expr_info(expr, source) {
        // Handle control flow that makes subsequent code dead
        if is_terminating_instruction(&instr_name) {
            stack.mark_uncertain();
            return;
        }

        // Calculate operands needed from the stack
        let expected = get_expected_operands_by_name(&instr_name, symbols, source, arity_map, expr);
        let from_stack = expected.saturating_sub(explicit_operands);

        // Consume operands from stack
        if from_stack > 0 {
            if let Err(available) = stack.consume(from_stack) {
                // Use the expr node for the diagnostic location
                let diagnostic =
                    create_stack_underflow_diagnostic(expr, &instr_name, from_stack, available);
                diagnostics.push(diagnostic);
            }
        }
    }

    // For block expressions (block, loop, if), consume param types from the outer stack
    // This handles multi-value block params per the Wasm 2.0 multi-value proposal
    let block_params = get_block_param_types_from_expr(expr, source);
    if !block_params.is_empty() {
        if let Err(available) = stack.consume(block_params.len()) {
            let diagnostic =
                create_stack_underflow_diagnostic(expr, "block", block_params.len(), available);
            diagnostics.push(diagnostic);
        }
    }

    // Check if the expression always terminates (e.g., a block where all paths return)
    // If so, mark uncertain and don't produce values
    if sequence_always_terminates(expr, source) {
        stack.mark_uncertain();
        return;
    }

    // Produce result values with actual types
    let result_types = get_expr_result_types(expr, source, symbols, arity_map);
    stack.produce(result_types);
}

/// Get result types from a folded expression
pub fn get_expr_result_types(
    expr: &Node,
    source: &str,
    symbols: &SymbolTable,
    arity_map: &HashMap<&'static str, InstructionArity>,
) -> Vec<ValueType> {
    // Find the instruction inside the expression
    // AST structure can be: expr > expr1_* or expr > expr1 > expr1_*
    let mut cursor = expr.walk();
    for child in expr.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        if kind.starts_with("expr1_") {
            return get_expr1_result_types(&child, source, symbols, arity_map);
        }
        // Handle wrapped expr1 node
        if kind == "expr1" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                #[cfg(feature = "native")]
                let inner_kind = inner_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let inner_kind = inner_child.kind();

                if inner_kind.starts_with("expr1_") {
                    return get_expr1_result_types(&inner_child, source, symbols, arity_map);
                }
            }
        }
    }
    // Default: assume 1 value of Unknown type
    vec![ValueType::Unknown]
}

/// Get result types for expr1_* nodes
fn get_expr1_result_types(
    expr1: &Node,
    source: &str,
    symbols: &SymbolTable,
    _arity_map: &HashMap<&'static str, InstructionArity>,
) -> Vec<ValueType> {
    #[cfg(feature = "native")]
    let kind = expr1.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = expr1.kind();

    match kind.as_ref() {
        "expr1_plain" => {
            // Get instruction name from instr_plain child
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
        "expr1_block" | "expr1_loop" => {
            // Block produces its result type values
            get_block_result_types(expr1, source)
        }
        "expr1_if" => {
            // If produces its result type values
            get_block_result_types(expr1, source)
        }
        "expr1_try" | "expr1_try_table" => {
            // Try blocks produce their result type values
            get_block_result_types(expr1, source)
        }
        "expr1_call" => {
            // Check if this is call_indirect or regular call
            let mut cursor = expr1.walk();
            for child in expr1.children(&mut cursor) {
                #[cfg(feature = "native")]
                let child_kind = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let child_kind = child.kind();

                if child_kind == "call_indirect" || child_kind == "return_call_indirect" {
                    // call_indirect - look up type for result types
                    // Pass expr1 since type_use is a sibling of call_indirect
                    return get_call_indirect_result_types(expr1, symbols, source);
                }
            }
            // Regular call - look up function result types
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

        // Handle func_type_results directly (folded format: expr1_block, expr1_loop)
        if kind == "func_type_results" {
            return parse_func_type_results(&child, source);
        }
        // Handle block_type if present
        if kind == "block_type" {
            return parse_result_types(&child, source);
        }
        // Handle block_block, loop_block (linear format), if_block/block_if (both formats)
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
/// Navigates expr > expr1_* or expr > expr1 > expr1_* to find block nodes
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

/// Get param types from a block/loop/if node (for multi-value block params)
/// Mirrors get_block_result_types but looks for func_type_params_many instead of func_type_results
pub fn get_block_param_types(block_node: &Node, source: &str) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut cursor = block_node.walk();
    for child in block_node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = child.kind();

        // Handle func_type_params_many directly (folded format: expr1_block, expr1_loop)
        if kind == "func_type_params_many" {
            types.extend(parse_func_type_results(&child, source));
        }
        // Handle block_block, loop_block (linear format), if_block/block_if (both formats)
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
            // Parse value types inside
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
        // Also handle inline result types
        if kind == "value_type" || kind == "ref_type" {
            let type_text = &source[child.byte_range()];
            if let Some(t) = ValueType::try_parse(type_text.trim()) {
                types.push(t);
            } else {
                types.push(ValueType::Unknown);
            }
        }
    }

    // Fallback: parse from text if we couldn't find structured types
    if types.is_empty() {
        let text = &source[block_type.byte_range()];
        if text.contains("result") {
            // Try to extract types from text
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

/// Check if final stack matches expected return types
pub fn check_return_types(
    stack: &StackState,
    expected: &[ValueType],
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let actual = stack.get_types();

    // Check count first
    if actual.len() != expected.len() {
        let range = node_to_range(node);
        let actual_word = if actual.len() == 1 { "value" } else { "values" };
        diagnostics.push(
            Diagnostic::error(
                range,
                format!(
                    "Type mismatch: function body leaves {} {} on stack but signature requires {}",
                    actual.len(),
                    actual_word,
                    expected.len()
                ),
            )
            .with_code("return-arity-mismatch"),
        );
        return;
    }

    // Check types match
    for (i, (actual_type, expected_type)) in actual.iter().zip(expected.iter()).enumerate() {
        // Skip Unknown types (we couldn't infer)
        if *actual_type == ValueType::Unknown {
            continue;
        }

        if actual_type != expected_type {
            let range = node_to_range(node);
            let position_msg = if expected.len() == 1 {
                String::new()
            } else {
                format!(" at position {}", i + 1)
            };
            diagnostics.push(
                Diagnostic::error(
                    range,
                    format!(
                        "Type mismatch{}: function returns {} but declared as {}",
                        position_msg, actual_type, expected_type
                    ),
                )
                .with_code("return-type-mismatch"),
            );
        }
    }
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
