//! Instruction metadata for stack tracking.
//!
//! This module is shared between native and WASM builds to provide
//! instruction arity information for stack underflow detection.
//!
//! Uses a sorted array with binary search for O(log n) lookups,
//! matching the pattern used by `docs.rs`. Eliminates HashMap
//! overhead and runtime allocation in WASM builds.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperandMode {
    Fixed(usize),
    Dynamic, // Depends on type definition or other context
}

/// Represents the expected parameter count for a WAT instruction
#[derive(Debug, Clone, Copy)]
pub struct InstructionArity {
    pub min_params: usize,
    pub max_params: usize,
    pub param_description: &'static str,
    /// Number of operands this instruction consumes from the stack (for folded expressions)
    pub operand_mode: OperandMode,
    /// Number of values this instruction pushes to the stack
    pub produces: usize,
}

impl InstructionArity {
    const fn exact(
        count: usize,
        description: &'static str,
        stack_operands: usize,
        produces: usize,
    ) -> Self {
        Self {
            min_params: count,
            max_params: count,
            param_description: description,
            operand_mode: OperandMode::Fixed(stack_operands),
            produces,
        }
    }

    // For instructions with dynamic arity (like struct.new)
    const fn dynamic(
        min_params: usize,
        max_params: usize,
        description: &'static str,
        produces: usize,
    ) -> Self {
        Self {
            min_params,
            max_params,
            param_description: description,
            operand_mode: OperandMode::Dynamic,
            produces,
        }
    }

    /// Index-based instruction (local.get, global.get, etc.) - produces a value
    const fn index(description: &'static str) -> Self {
        Self::exact(1, description, 0, 1)
    }

    /// Index-based instruction that consumes a value (local.set, global.set)
    const fn index_set(description: &'static str) -> Self {
        Self::exact(1, description, 1, 0)
    }

    /// Index-based instruction that consumes and produces a value (local.tee)
    const fn index_tee(description: &'static str) -> Self {
        Self::exact(1, description, 1, 1)
    }

    /// Constant instruction (i32.const, f64.const) - produces a value
    const fn constant(description: &'static str) -> Self {
        Self::exact(1, description, 0, 1)
    }

    /// Binary operator (i32.add, i32.mul, etc.) - consumes 2 values, produces 1
    const fn binary_op() -> Self {
        Self::exact(0, "", 2, 1)
    }

    /// Unary operator (i32.eqz, i32.clz, etc.) - consumes 1 value, produces 1
    const fn unary_op() -> Self {
        Self::exact(0, "", 1, 1)
    }

    /// Nullary instruction with no params or operands and no production (nop)
    const fn nullary() -> Self {
        Self::exact(0, "", 0, 0)
    }

    /// Instruction that consumes 1 value and produces nothing (drop)
    const fn drop_op() -> Self {
        Self::exact(0, "", 1, 0)
    }

    /// Memory load operation - can take optional memory index (multi-memory proposal)
    /// Consumes address, produces value
    const fn mem_load() -> Self {
        Self {
            min_params: 0,
            max_params: 1,
            param_description: "optional memory index",
            operand_mode: OperandMode::Fixed(1), // consumes address
            produces: 1,
        }
    }

    /// Memory store operation - can take optional memory index (multi-memory proposal)
    /// Consumes address and value, produces nothing
    const fn mem_store() -> Self {
        Self {
            min_params: 0,
            max_params: 1,
            param_description: "optional memory index",
            operand_mode: OperandMode::Fixed(2), // consumes address and value
            produces: 0,
        }
    }

    /// Atomic RMW operation - addr + operand → old value
    const fn mem_rmw() -> Self {
        Self {
            min_params: 0,
            max_params: 1,
            param_description: "optional memory index",
            operand_mode: OperandMode::Fixed(2), // consumes address and operand
            produces: 1,
        }
    }

    /// Atomic cmpxchg operation - addr + expected + replacement → old value
    const fn mem_cmpxchg() -> Self {
        Self {
            min_params: 0,
            max_params: 1,
            param_description: "optional memory index",
            operand_mode: OperandMode::Fixed(3), // consumes address, expected, replacement
            produces: 1,
        }
    }

    pub fn is_valid(&self, param_count: usize) -> bool {
        param_count >= self.min_params && param_count <= self.max_params
    }

    pub fn expected_message(&self) -> String {
        if self.min_params == self.max_params {
            if self.param_description.is_empty() {
                format!("{}", self.min_params)
            } else {
                format!("{} ({})", self.min_params, self.param_description)
            }
        } else if self.param_description.is_empty() {
            format!("{}-{}", self.min_params, self.max_params)
        } else {
            format!(
                "{}-{} ({})",
                self.min_params, self.max_params, self.param_description
            )
        }
    }
}

// ============================================================================
// Sorted arity table — single OnceLock, binary-search lookup
// ============================================================================

static ARITY_TABLE: OnceLock<Vec<(&'static str, InstructionArity)>> = OnceLock::new();

fn init_arity_table() -> Vec<(&'static str, InstructionArity)> {
    let mut entries: Vec<(&'static str, InstructionArity)> = vec![
        // Local variable instructions
        ("local.get", InstructionArity::index("index")),
        ("local.set", InstructionArity::index_set("index")),
        ("local.tee", InstructionArity::index_tee("index")),
        // Global variable instructions
        ("global.get", InstructionArity::index("index")),
        ("global.set", InstructionArity::index_set("index")),
        // Control flow
        ("br", InstructionArity::dynamic(1, 1, "label index", 0)),
        ("br_if", InstructionArity::dynamic(1, 1, "label index", 0)),
        (
            "br_table",
            InstructionArity::dynamic(1, usize::MAX, "label indices", 0),
        ),
        ("call", InstructionArity::dynamic(1, 1, "function index", 0)),
        (
            "return_call",
            InstructionArity::dynamic(1, 1, "function index", 0),
        ),
        (
            "call_indirect",
            InstructionArity::dynamic(1, 1, "type index", 0),
        ),
        (
            "return_call_indirect",
            InstructionArity::dynamic(1, 1, "type index", 0),
        ),
        ("return", InstructionArity::dynamic(0, 0, "", 0)),
        ("unreachable", InstructionArity::nullary()),
        ("nop", InstructionArity::nullary()),
        // Constants
        ("i32.const", InstructionArity::constant("literal value")),
        ("i64.const", InstructionArity::constant("literal value")),
        ("f32.const", InstructionArity::constant("literal value")),
        ("f64.const", InstructionArity::constant("literal value")),
        // Stack manipulation
        ("drop", InstructionArity::drop_op()),
        ("select", InstructionArity::exact(0, "", 3, 1)),
        // i32 arithmetic (binary)
        ("i32.add", InstructionArity::binary_op()),
        ("i32.sub", InstructionArity::binary_op()),
        ("i32.mul", InstructionArity::binary_op()),
        ("i32.div_s", InstructionArity::binary_op()),
        ("i32.div_u", InstructionArity::binary_op()),
        ("i32.rem_s", InstructionArity::binary_op()),
        ("i32.rem_u", InstructionArity::binary_op()),
        ("i32.and", InstructionArity::binary_op()),
        ("i32.or", InstructionArity::binary_op()),
        ("i32.xor", InstructionArity::binary_op()),
        ("i32.shl", InstructionArity::binary_op()),
        ("i32.shr_s", InstructionArity::binary_op()),
        ("i32.shr_u", InstructionArity::binary_op()),
        ("i32.rotl", InstructionArity::binary_op()),
        ("i32.rotr", InstructionArity::binary_op()),
        // i32 comparison (binary)
        ("i32.eq", InstructionArity::binary_op()),
        ("i32.ne", InstructionArity::binary_op()),
        ("i32.lt_s", InstructionArity::binary_op()),
        ("i32.lt_u", InstructionArity::binary_op()),
        ("i32.le_s", InstructionArity::binary_op()),
        ("i32.le_u", InstructionArity::binary_op()),
        ("i32.gt_s", InstructionArity::binary_op()),
        ("i32.gt_u", InstructionArity::binary_op()),
        ("i32.ge_s", InstructionArity::binary_op()),
        ("i32.ge_u", InstructionArity::binary_op()),
        // i32 unary
        ("i32.clz", InstructionArity::unary_op()),
        ("i32.ctz", InstructionArity::unary_op()),
        ("i32.popcnt", InstructionArity::unary_op()),
        ("i32.eqz", InstructionArity::unary_op()),
        // i64 arithmetic (binary)
        ("i64.add", InstructionArity::binary_op()),
        ("i64.sub", InstructionArity::binary_op()),
        ("i64.mul", InstructionArity::binary_op()),
        ("i64.div_s", InstructionArity::binary_op()),
        ("i64.div_u", InstructionArity::binary_op()),
        ("i64.rem_s", InstructionArity::binary_op()),
        ("i64.rem_u", InstructionArity::binary_op()),
        ("i64.and", InstructionArity::binary_op()),
        ("i64.or", InstructionArity::binary_op()),
        ("i64.xor", InstructionArity::binary_op()),
        ("i64.shl", InstructionArity::binary_op()),
        ("i64.shr_s", InstructionArity::binary_op()),
        ("i64.shr_u", InstructionArity::binary_op()),
        ("i64.rotl", InstructionArity::binary_op()),
        ("i64.rotr", InstructionArity::binary_op()),
        // i64 comparison (binary)
        ("i64.eq", InstructionArity::binary_op()),
        ("i64.ne", InstructionArity::binary_op()),
        ("i64.lt_s", InstructionArity::binary_op()),
        ("i64.lt_u", InstructionArity::binary_op()),
        ("i64.le_s", InstructionArity::binary_op()),
        ("i64.le_u", InstructionArity::binary_op()),
        ("i64.gt_s", InstructionArity::binary_op()),
        ("i64.gt_u", InstructionArity::binary_op()),
        ("i64.ge_s", InstructionArity::binary_op()),
        ("i64.ge_u", InstructionArity::binary_op()),
        // i64 unary
        ("i64.clz", InstructionArity::unary_op()),
        ("i64.ctz", InstructionArity::unary_op()),
        ("i64.popcnt", InstructionArity::unary_op()),
        ("i64.eqz", InstructionArity::unary_op()),
        // f32 arithmetic (binary)
        ("f32.add", InstructionArity::binary_op()),
        ("f32.sub", InstructionArity::binary_op()),
        ("f32.mul", InstructionArity::binary_op()),
        ("f32.div", InstructionArity::binary_op()),
        ("f32.min", InstructionArity::binary_op()),
        ("f32.max", InstructionArity::binary_op()),
        ("f32.copysign", InstructionArity::binary_op()),
        // f32 comparison (binary)
        ("f32.eq", InstructionArity::binary_op()),
        ("f32.ne", InstructionArity::binary_op()),
        ("f32.lt", InstructionArity::binary_op()),
        ("f32.le", InstructionArity::binary_op()),
        ("f32.gt", InstructionArity::binary_op()),
        ("f32.ge", InstructionArity::binary_op()),
        // f32 unary
        ("f32.abs", InstructionArity::unary_op()),
        ("f32.neg", InstructionArity::unary_op()),
        ("f32.ceil", InstructionArity::unary_op()),
        ("f32.floor", InstructionArity::unary_op()),
        ("f32.trunc", InstructionArity::unary_op()),
        ("f32.nearest", InstructionArity::unary_op()),
        ("f32.sqrt", InstructionArity::unary_op()),
        // f64 arithmetic (binary)
        ("f64.add", InstructionArity::binary_op()),
        ("f64.sub", InstructionArity::binary_op()),
        ("f64.mul", InstructionArity::binary_op()),
        ("f64.div", InstructionArity::binary_op()),
        ("f64.min", InstructionArity::binary_op()),
        ("f64.max", InstructionArity::binary_op()),
        ("f64.copysign", InstructionArity::binary_op()),
        // f64 comparison (binary)
        ("f64.eq", InstructionArity::binary_op()),
        ("f64.ne", InstructionArity::binary_op()),
        ("f64.lt", InstructionArity::binary_op()),
        ("f64.le", InstructionArity::binary_op()),
        ("f64.gt", InstructionArity::binary_op()),
        ("f64.ge", InstructionArity::binary_op()),
        // f64 unary
        ("f64.abs", InstructionArity::unary_op()),
        ("f64.neg", InstructionArity::unary_op()),
        ("f64.ceil", InstructionArity::unary_op()),
        ("f64.floor", InstructionArity::unary_op()),
        ("f64.trunc", InstructionArity::unary_op()),
        ("f64.nearest", InstructionArity::unary_op()),
        ("f64.sqrt", InstructionArity::unary_op()),
        // Conversion operations (unary)
        ("i32.wrap_i64", InstructionArity::unary_op()),
        ("i64.extend_i32_s", InstructionArity::unary_op()),
        ("i64.extend_i32_u", InstructionArity::unary_op()),
        ("i32.trunc_f32_s", InstructionArity::unary_op()),
        ("i32.trunc_f32_u", InstructionArity::unary_op()),
        ("i32.trunc_f64_s", InstructionArity::unary_op()),
        ("i32.trunc_f64_u", InstructionArity::unary_op()),
        ("i64.trunc_f32_s", InstructionArity::unary_op()),
        ("i64.trunc_f32_u", InstructionArity::unary_op()),
        ("i64.trunc_f64_s", InstructionArity::unary_op()),
        ("i64.trunc_f64_u", InstructionArity::unary_op()),
        ("f32.convert_i32_s", InstructionArity::unary_op()),
        ("f32.convert_i32_u", InstructionArity::unary_op()),
        ("f32.convert_i64_s", InstructionArity::unary_op()),
        ("f32.convert_i64_u", InstructionArity::unary_op()),
        ("f32.demote_f64", InstructionArity::unary_op()),
        ("f64.convert_i32_s", InstructionArity::unary_op()),
        ("f64.convert_i32_u", InstructionArity::unary_op()),
        ("f64.convert_i64_s", InstructionArity::unary_op()),
        ("f64.convert_i64_u", InstructionArity::unary_op()),
        ("f64.promote_f32", InstructionArity::unary_op()),
        ("i32.reinterpret_f32", InstructionArity::unary_op()),
        ("i64.reinterpret_f64", InstructionArity::unary_op()),
        ("f32.reinterpret_i32", InstructionArity::unary_op()),
        ("f64.reinterpret_i64", InstructionArity::unary_op()),
        // Memory load (optional memory index, consume address)
        ("i32.load", InstructionArity::mem_load()),
        ("i64.load", InstructionArity::mem_load()),
        ("f32.load", InstructionArity::mem_load()),
        ("f64.load", InstructionArity::mem_load()),
        ("i32.load8_s", InstructionArity::mem_load()),
        ("i32.load8_u", InstructionArity::mem_load()),
        ("i32.load16_s", InstructionArity::mem_load()),
        ("i32.load16_u", InstructionArity::mem_load()),
        ("i64.load8_s", InstructionArity::mem_load()),
        ("i64.load8_u", InstructionArity::mem_load()),
        ("i64.load16_s", InstructionArity::mem_load()),
        ("i64.load16_u", InstructionArity::mem_load()),
        ("i64.load32_s", InstructionArity::mem_load()),
        ("i64.load32_u", InstructionArity::mem_load()),
        // Memory store (optional memory index, consume address + value)
        ("i32.store", InstructionArity::mem_store()),
        ("i64.store", InstructionArity::mem_store()),
        ("f32.store", InstructionArity::mem_store()),
        ("f64.store", InstructionArity::mem_store()),
        ("i32.store8", InstructionArity::mem_store()),
        ("i32.store16", InstructionArity::mem_store()),
        ("i64.store8", InstructionArity::mem_store()),
        ("i64.store16", InstructionArity::mem_store()),
        ("i64.store32", InstructionArity::mem_store()),
        // Memory management (optional memory index)
        (
            "memory.size",
            InstructionArity {
                min_params: 0,
                max_params: 1,
                param_description: "optional memory index",
                operand_mode: OperandMode::Fixed(0),
                produces: 1,
            },
        ),
        (
            "memory.grow",
            InstructionArity {
                min_params: 0,
                max_params: 1,
                param_description: "optional memory index",
                operand_mode: OperandMode::Fixed(1),
                produces: 1,
            },
        ),
        // WasmGC — Structs
        (
            "struct.new",
            InstructionArity::dynamic(1, 1, "type index", 1),
        ),
        (
            "struct.new_default",
            InstructionArity::exact(1, "type index", 0, 1),
        ),
        (
            "struct.get",
            InstructionArity::exact(2, "type and field index", 1, 1),
        ),
        (
            "struct.get_s",
            InstructionArity::exact(2, "type and field index", 1, 1),
        ),
        (
            "struct.get_u",
            InstructionArity::exact(2, "type and field index", 1, 1),
        ),
        (
            "struct.set",
            InstructionArity::exact(2, "type and field index", 2, 0),
        ),
        // WasmGC — Arrays
        ("array.new", InstructionArity::exact(1, "type index", 2, 1)),
        (
            "array.new_default",
            InstructionArity::exact(1, "type index", 1, 1),
        ),
        (
            "array.new_fixed",
            InstructionArity::dynamic(2, 2, "type index and length", 1),
        ),
        (
            "array.new_data",
            InstructionArity::exact(2, "type and data index", 2, 1),
        ),
        (
            "array.new_elem",
            InstructionArity::exact(2, "type and elem index", 2, 1),
        ),
        ("array.get", InstructionArity::exact(1, "type index", 2, 1)),
        (
            "array.get_s",
            InstructionArity::exact(1, "type index", 2, 1),
        ),
        (
            "array.get_u",
            InstructionArity::exact(1, "type index", 2, 1),
        ),
        ("array.set", InstructionArity::exact(1, "type index", 3, 0)),
        ("array.len", InstructionArity::unary_op()),
        ("array.fill", InstructionArity::exact(1, "type index", 4, 0)),
        (
            "array.copy",
            InstructionArity::exact(2, "dest and src type index", 5, 0),
        ),
        (
            "array.init_data",
            InstructionArity::exact(2, "type and data index", 4, 0),
        ),
        (
            "array.init_elem",
            InstructionArity::exact(2, "type and elem index", 4, 0),
        ),
        // WasmGC — I31
        ("ref.i31", InstructionArity::unary_op()),
        ("i31.get_s", InstructionArity::unary_op()),
        ("i31.get_u", InstructionArity::unary_op()),
        // WasmGC — Casts
        ("ref.test", InstructionArity::exact(1, "type index", 1, 1)),
        ("ref.cast", InstructionArity::exact(1, "type index", 1, 1)),
        (
            "ref.cast_null",
            InstructionArity::exact(1, "type index", 1, 1),
        ),
        (
            "br_on_cast",
            InstructionArity::exact(3, "label, source and target type", 1, 0),
        ),
        (
            "br_on_cast_fail",
            InstructionArity::exact(3, "label, source and target type", 1, 0),
        ),
        // Exceptions
        ("throw", InstructionArity::dynamic(1, 1, "tag index", 0)),
        ("throw_ref", InstructionArity::exact(0, "", 1, 0)),
        ("rethrow", InstructionArity::exact(1, "label index", 0, 0)),
        // Typed function references
        ("call_ref", InstructionArity::dynamic(1, 1, "type index", 0)),
        (
            "return_call_ref",
            InstructionArity::dynamic(1, 1, "type index", 0),
        ),
        // Null-checking branches
        (
            "br_on_null",
            InstructionArity::dynamic(1, 1, "label index", 1),
        ),
        (
            "br_on_non_null",
            InstructionArity::dynamic(1, 1, "label index", 0),
        ),
        // Reference equality
        ("ref.eq", InstructionArity::binary_op()),
        // Reference conversions
        ("any.convert_extern", InstructionArity::unary_op()),
        ("extern.convert_any", InstructionArity::unary_op()),
        // Bulk memory (optional memory indices)
        (
            "memory.copy",
            InstructionArity {
                min_params: 0,
                max_params: 2,
                param_description: "optional dest and src memory indices",
                operand_mode: OperandMode::Fixed(3),
                produces: 0,
            },
        ),
        (
            "memory.fill",
            InstructionArity {
                min_params: 0,
                max_params: 1,
                param_description: "optional memory index",
                operand_mode: OperandMode::Fixed(3),
                produces: 0,
            },
        ),
        (
            "memory.init",
            InstructionArity {
                min_params: 1,
                max_params: 2,
                param_description: "data index and optional memory index",
                operand_mode: OperandMode::Fixed(3),
                produces: 0,
            },
        ),
        ("data.drop", InstructionArity::exact(1, "data index", 0, 0)),
        // Table operations
        ("table.get", InstructionArity::exact(1, "table index", 1, 1)),
        ("table.set", InstructionArity::exact(1, "table index", 2, 0)),
        (
            "table.size",
            InstructionArity::exact(1, "table index", 0, 1),
        ),
        (
            "table.grow",
            InstructionArity::exact(1, "table index", 2, 1),
        ),
        (
            "table.fill",
            InstructionArity::exact(1, "table index", 3, 0),
        ),
        (
            "table.copy",
            InstructionArity::exact(2, "dest and src table index", 3, 0),
        ),
        (
            "table.init",
            InstructionArity::exact(2, "table and elem index", 3, 0),
        ),
        ("elem.drop", InstructionArity::exact(1, "elem index", 0, 0)),
        // Reference operations
        ("ref.null", InstructionArity::exact(1, "type", 0, 1)),
        (
            "ref.func",
            InstructionArity::exact(1, "function index", 0, 1),
        ),
        (
            "ref.extern",
            InstructionArity::exact(1, "externref index", 0, 1),
        ),
        ("ref.is_null", InstructionArity::unary_op()),
        ("ref.as_non_null", InstructionArity::unary_op()),
        // Saturating truncation
        ("i32.trunc_sat_f32_s", InstructionArity::unary_op()),
        ("i32.trunc_sat_f32_u", InstructionArity::unary_op()),
        ("i32.trunc_sat_f64_s", InstructionArity::unary_op()),
        ("i32.trunc_sat_f64_u", InstructionArity::unary_op()),
        ("i64.trunc_sat_f32_s", InstructionArity::unary_op()),
        ("i64.trunc_sat_f32_u", InstructionArity::unary_op()),
        ("i64.trunc_sat_f64_s", InstructionArity::unary_op()),
        ("i64.trunc_sat_f64_u", InstructionArity::unary_op()),
        // Sign extension
        ("i32.extend8_s", InstructionArity::unary_op()),
        ("i32.extend16_s", InstructionArity::unary_op()),
        ("i64.extend8_s", InstructionArity::unary_op()),
        ("i64.extend16_s", InstructionArity::unary_op()),
        ("i64.extend32_s", InstructionArity::unary_op()),
        // Atomic operations
        ("atomic.fence", InstructionArity::nullary()),
        // Atomic loads
        ("i32.atomic.load", InstructionArity::mem_load()),
        ("i64.atomic.load", InstructionArity::mem_load()),
        ("i32.atomic.load8_u", InstructionArity::mem_load()),
        ("i32.atomic.load16_u", InstructionArity::mem_load()),
        ("i64.atomic.load8_u", InstructionArity::mem_load()),
        ("i64.atomic.load16_u", InstructionArity::mem_load()),
        ("i64.atomic.load32_u", InstructionArity::mem_load()),
        // Atomic stores
        ("i32.atomic.store", InstructionArity::mem_store()),
        ("i64.atomic.store", InstructionArity::mem_store()),
        ("i32.atomic.store8", InstructionArity::mem_store()),
        ("i32.atomic.store16", InstructionArity::mem_store()),
        ("i64.atomic.store8", InstructionArity::mem_store()),
        ("i64.atomic.store16", InstructionArity::mem_store()),
        ("i64.atomic.store32", InstructionArity::mem_store()),
        // Atomic RMW i32 full-width
        ("i32.atomic.rmw.add", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw.sub", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw.and", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw.or", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw.xor", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw.xchg", InstructionArity::mem_rmw()),
        // Atomic RMW i32 narrow
        ("i32.atomic.rmw8.add_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw8.sub_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw8.and_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw8.or_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw8.xor_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw8.xchg_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw16.add_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw16.sub_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw16.and_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw16.or_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw16.xor_u", InstructionArity::mem_rmw()),
        ("i32.atomic.rmw16.xchg_u", InstructionArity::mem_rmw()),
        // Atomic RMW i64 full-width
        ("i64.atomic.rmw.add", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw.sub", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw.and", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw.or", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw.xor", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw.xchg", InstructionArity::mem_rmw()),
        // Atomic RMW i64 narrow
        ("i64.atomic.rmw8.add_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw8.sub_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw8.and_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw8.or_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw8.xor_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw8.xchg_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw16.add_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw16.sub_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw16.and_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw16.or_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw16.xor_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw16.xchg_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw32.add_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw32.sub_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw32.and_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw32.or_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw32.xor_u", InstructionArity::mem_rmw()),
        ("i64.atomic.rmw32.xchg_u", InstructionArity::mem_rmw()),
        // Atomic cmpxchg
        ("i32.atomic.rmw.cmpxchg", InstructionArity::mem_cmpxchg()),
        ("i64.atomic.rmw.cmpxchg", InstructionArity::mem_cmpxchg()),
        ("i32.atomic.rmw8.cmpxchg_u", InstructionArity::mem_cmpxchg()),
        (
            "i32.atomic.rmw16.cmpxchg_u",
            InstructionArity::mem_cmpxchg(),
        ),
        ("i64.atomic.rmw8.cmpxchg_u", InstructionArity::mem_cmpxchg()),
        (
            "i64.atomic.rmw16.cmpxchg_u",
            InstructionArity::mem_cmpxchg(),
        ),
        (
            "i64.atomic.rmw32.cmpxchg_u",
            InstructionArity::mem_cmpxchg(),
        ),
        // Wait/notify
        ("memory.atomic.wait32", InstructionArity::exact(0, "", 3, 1)),
        ("memory.atomic.wait64", InstructionArity::exact(0, "", 3, 1)),
        ("memory.atomic.notify", InstructionArity::exact(0, "", 2, 1)),
    ];
    entries.sort_by_key(|(name, _)| *name);
    entries
}

/// Look up instruction arity by name using binary search on the sorted table.
pub fn lookup_instruction_arity(name: &str) -> Option<&'static InstructionArity> {
    let table = ARITY_TABLE.get_or_init(init_arity_table);
    table
        .binary_search_by_key(&name, |(k, _)| k)
        .ok()
        .map(|i| &table[i].1)
}

/// Returns a map of instruction names to their expected parameter counts (test-only).
#[cfg(test)]
pub fn get_instruction_arity_map() -> std::collections::HashMap<&'static str, InstructionArity> {
    init_arity_table().into_iter().collect()
}

/// Infer the stack arity of a SIMD instruction from its name pattern.
/// Returns `Some((consumes, produces))` for recognized SIMD patterns, or `None`.
///
/// This covers v128.* operations and lane-typed instructions (i8x16, i16x8, i32x4,
/// i64x2, f32x4, f64x2) without enumerating every individual opcode.
pub fn infer_simd_instruction_arity(name: &str) -> Option<(usize, usize)> {
    // ── v128.* prefix operations ──────────────────────────────────────
    if name.starts_with("v128.") {
        return match name {
            "v128.const" => Some((0, 1)),
            "v128.not" | "v128.any_true" => Some((1, 1)),
            "v128.and" | "v128.or" | "v128.xor" | "v128.andnot" => Some((2, 1)),
            "v128.bitselect" => Some((3, 1)),
            n if n.starts_with("v128.load") => {
                if n.contains("_lane") {
                    Some((2, 1)) // address + v128 → v128
                } else {
                    Some((1, 1)) // address → v128  (includes splat/zero/extend variants)
                }
            }
            _ if name.starts_with("v128.store") => Some((2, 0)), // address + v128 → ∅
            _ => None,
        };
    }

    // ── Lane-typed operations (i8x16.*, i16x8.*, …, f64x2.*) ─────────
    let is_simd_lane = name.starts_with("i8x16.")
        || name.starts_with("i16x8.")
        || name.starts_with("i32x4.")
        || name.starts_with("i64x2.")
        || name.starts_with("f32x4.")
        || name.starts_with("f64x2.");

    if !is_simd_lane {
        return None;
    }

    // Splat: scalar → v128
    if name.ends_with(".splat") {
        return Some((1, 1));
    }

    // Extract lane: v128 (+ imm) → scalar
    if name.contains(".extract_lane") {
        return Some((1, 1));
    }

    // Replace lane: v128 + scalar (+ imm) → v128
    if name.contains(".replace_lane") {
        return Some((2, 1));
    }

    // Reductions: v128 → i32
    if name.ends_with(".all_true") || name.ends_with(".bitmask") {
        return Some((1, 1));
    }

    // ── Relaxed SIMD (explicit handling for all instructions) ───────
    if name.contains(".relaxed_") {
        // Ternary: 3 v128 → v128
        if name.contains(".relaxed_madd")
            || name.contains(".relaxed_nmadd")
            || name.contains(".relaxed_laneselect")
            || name.contains(".relaxed_dot_i8x16_i7x16_add")
        {
            return Some((3, 1));
        }
        // Unary: 1 v128 → v128
        if name.contains(".relaxed_trunc") {
            return Some((1, 1));
        }
        // Binary: 2 v128 → v128 (swizzle, min, max, q15mulr)
        return Some((2, 1));
    }

    // Unary ops: 1 v128 → 1 v128
    let suffix = name.rsplit('.').next().unwrap_or("");
    let is_unary = matches!(
        suffix,
        "neg"
            | "abs"
            | "popcnt"
            | "sqrt"
            | "ceil"
            | "floor"
            | "trunc"
            | "nearest"
            | "not"
            | "trunc_sat_f32x4_s"
            | "trunc_sat_f32x4_u"
            | "trunc_sat_f64x2_s"
            | "trunc_sat_f64x2_u"
    ) || suffix.starts_with("extend_low")
        || suffix.starts_with("extend_high")
        || suffix.starts_with("extadd_pairwise")
        || suffix.starts_with("promote_low")
        || suffix.starts_with("demote")
        || suffix.starts_with("convert");

    if is_unary {
        return Some((1, 1));
    }

    // Default for remaining lane ops: binary (2 → 1).
    // This covers add, sub, mul, min, max, eq, ne, lt, gt, le, ge,
    // shl, shr, avgr, swizzle, shuffle, narrow, extmul, dot, pmin, pmax, …
    Some((2, 1))
}

/// Check if an instruction terminates control flow (makes subsequent code unreachable).
/// These instructions mark the stack as "uncertain" because code after them is dead.
/// This is used by both native and WASM builds for consistent stack tracking.
pub fn is_terminating_instruction(name: &str) -> bool {
    matches!(
        name,
        "unreachable"
            | "return"
            | "br"
            | "br_table"
            | "throw"
            | "throw_ref"
            | "rethrow"
            | "return_call"
            | "return_call_indirect"
            | "return_call_ref"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_arity_exact() {
        let arity = InstructionArity::exact(1, "index", 0, 1);
        assert_eq!(arity.min_params, 1);
        assert_eq!(arity.max_params, 1);
        assert_eq!(arity.operand_mode, OperandMode::Fixed(0));
        assert_eq!(arity.produces, 1);
        assert!(arity.is_valid(1));
        assert!(!arity.is_valid(0));
        assert!(!arity.is_valid(2));
    }

    #[test]
    fn test_binary_op() {
        let arity = InstructionArity::binary_op();
        assert_eq!(arity.operand_mode, OperandMode::Fixed(2));
        assert_eq!(arity.produces, 1);
    }

    #[test]
    fn test_unary_op() {
        let arity = InstructionArity::unary_op();
        assert_eq!(arity.operand_mode, OperandMode::Fixed(1));
        assert_eq!(arity.produces, 1);
    }

    #[test]
    fn test_arity_map_contains_common_instructions() {
        let map = get_instruction_arity_map();
        assert!(map.contains_key("i32.add"));
        assert!(map.contains_key("local.get"));
        assert!(map.contains_key("call"));
        assert!(map.contains_key("i32.const"));
    }

    #[test]
    fn test_simd_arity_binary_ops() {
        // Binary lane ops: 2 v128 → 1 v128
        for instr in &[
            "i32x4.add",
            "i16x8.sub",
            "f32x4.mul",
            "i64x2.eq",
            "i8x16.shuffle",
            "i32x4.shl",
            "i16x8.narrow_i32x4_s",
            "i32x4.extmul_low_i16x8_s",
            "f32x4.pmin",
            "i8x16.swizzle",
        ] {
            let arity = infer_simd_instruction_arity(instr);
            assert_eq!(arity, Some((2, 1)), "expected binary arity for {instr}");
        }
    }

    #[test]
    fn test_simd_arity_unary_ops() {
        // Unary lane ops: 1 v128 → 1 v128
        for instr in &[
            "i32x4.neg",
            "f32x4.abs",
            "f64x2.sqrt",
            "f32x4.ceil",
            "i8x16.popcnt",
            "i32x4.extend_low_i16x8_s",
            "i16x8.extadd_pairwise_i8x16_s",
            "f64x2.promote_low_f32x4",
            "f32x4.demote_f64x2_zero",
            "i32x4.trunc_sat_f32x4_s",
            "f32x4.convert_i32x4_s",
        ] {
            let arity = infer_simd_instruction_arity(instr);
            assert_eq!(arity, Some((1, 1)), "expected unary arity for {instr}");
        }
    }

    #[test]
    fn test_simd_arity_special_ops() {
        // Splat: 1 scalar → 1 v128
        assert_eq!(infer_simd_instruction_arity("i32x4.splat"), Some((1, 1)));
        assert_eq!(infer_simd_instruction_arity("f64x2.splat"), Some((1, 1)));

        // Extract lane: 1 v128 → 1 scalar
        assert_eq!(
            infer_simd_instruction_arity("i32x4.extract_lane"),
            Some((1, 1))
        );
        assert_eq!(
            infer_simd_instruction_arity("i8x16.extract_lane_s"),
            Some((1, 1))
        );

        // Replace lane: v128 + scalar → v128
        assert_eq!(
            infer_simd_instruction_arity("i32x4.replace_lane"),
            Some((2, 1))
        );

        // Reductions: v128 → i32
        assert_eq!(infer_simd_instruction_arity("i32x4.all_true"), Some((1, 1)));
        assert_eq!(infer_simd_instruction_arity("i16x8.bitmask"), Some((1, 1)));
    }

    #[test]
    fn test_simd_arity_v128_ops() {
        // v128 bitwise
        assert_eq!(infer_simd_instruction_arity("v128.and"), Some((2, 1)));
        assert_eq!(infer_simd_instruction_arity("v128.or"), Some((2, 1)));
        assert_eq!(infer_simd_instruction_arity("v128.not"), Some((1, 1)));
        assert_eq!(infer_simd_instruction_arity("v128.bitselect"), Some((3, 1)));
        assert_eq!(infer_simd_instruction_arity("v128.any_true"), Some((1, 1)));

        // v128 const
        assert_eq!(infer_simd_instruction_arity("v128.const"), Some((0, 1)));

        // v128 memory ops
        assert_eq!(infer_simd_instruction_arity("v128.load"), Some((1, 1)));
        assert_eq!(
            infer_simd_instruction_arity("v128.load32_splat"),
            Some((1, 1))
        );
        assert_eq!(
            infer_simd_instruction_arity("v128.load64_zero"),
            Some((1, 1))
        );
        assert_eq!(
            infer_simd_instruction_arity("v128.load32_lane"),
            Some((2, 1))
        );
        assert_eq!(infer_simd_instruction_arity("v128.store"), Some((2, 0)));
        assert_eq!(
            infer_simd_instruction_arity("v128.store32_lane"),
            Some((2, 0))
        );
    }

    #[test]
    fn test_relaxed_simd_full_coverage() {
        // Ternary: madd, nmadd, laneselect (3 operands → 1 result)
        for instr in [
            "f32x4.relaxed_madd",
            "f32x4.relaxed_nmadd",
            "f64x2.relaxed_madd",
            "f64x2.relaxed_nmadd",
            "i8x16.relaxed_laneselect",
            "i16x8.relaxed_laneselect",
            "i32x4.relaxed_laneselect",
            "i64x2.relaxed_laneselect",
            "i32x4.relaxed_dot_i8x16_i7x16_add_s",
        ] {
            let arity = infer_simd_instruction_arity(instr);
            assert_eq!(
                arity,
                Some((3, 1)),
                "{instr} should be ternary (3 operands → 1 result)"
            );
        }

        // Binary: swizzle, min, max, trunc, q15mulr (2 operands → 1 result)
        for instr in [
            "i8x16.relaxed_swizzle",
            "f32x4.relaxed_min",
            "f32x4.relaxed_max",
            "f64x2.relaxed_min",
            "f64x2.relaxed_max",
            "i16x8.relaxed_q15mulr_s",
        ] {
            let arity = infer_simd_instruction_arity(instr);
            assert_eq!(
                arity,
                Some((2, 1)),
                "{instr} should be binary (2 operands → 1 result)"
            );
        }

        // Unary: trunc (1 operand → 1 result)
        for instr in [
            "i32x4.relaxed_trunc_f32x4_s",
            "i32x4.relaxed_trunc_f32x4_u",
            "i32x4.relaxed_trunc_f64x2_s_zero",
            "i32x4.relaxed_trunc_f64x2_u_zero",
        ] {
            let arity = infer_simd_instruction_arity(instr);
            assert_eq!(
                arity,
                Some((1, 1)),
                "{instr} should be unary (1 operand → 1 result)"
            );
        }
    }

    #[test]
    fn test_simd_arity_non_simd_returns_none() {
        // Non-SIMD instructions should return None
        assert_eq!(infer_simd_instruction_arity("i32.add"), None);
        assert_eq!(infer_simd_instruction_arity("local.get"), None);
        assert_eq!(infer_simd_instruction_arity("call"), None);
    }

    #[test]
    fn test_atomic_instruction_arity() {
        let map = get_instruction_arity_map();

        // fence: no operands, no results
        let fence = &map["atomic.fence"];
        assert_eq!(fence.operand_mode, OperandMode::Fixed(0));
        assert_eq!(fence.produces, 0);

        // Atomic loads: addr → value (same shape as mem_load)
        for instr in &[
            "i32.atomic.load",
            "i64.atomic.load",
            "i32.atomic.load8_u",
            "i32.atomic.load16_u",
            "i64.atomic.load8_u",
            "i64.atomic.load16_u",
            "i64.atomic.load32_u",
        ] {
            let arity = &map[*instr];
            assert_eq!(
                arity.operand_mode,
                OperandMode::Fixed(1),
                "{instr} operands"
            );
            assert_eq!(arity.produces, 1, "{instr} produces");
        }

        // Atomic stores: addr + value → ∅ (same shape as mem_store)
        for instr in &[
            "i32.atomic.store",
            "i64.atomic.store",
            "i32.atomic.store8",
            "i32.atomic.store16",
            "i64.atomic.store8",
            "i64.atomic.store16",
            "i64.atomic.store32",
        ] {
            let arity = &map[*instr];
            assert_eq!(
                arity.operand_mode,
                OperandMode::Fixed(2),
                "{instr} operands"
            );
            assert_eq!(arity.produces, 0, "{instr} produces");
        }

        // Atomic RMW: addr + operand → old value
        for instr in &[
            "i32.atomic.rmw.add",
            "i64.atomic.rmw.sub",
            "i32.atomic.rmw8.xor_u",
            "i64.atomic.rmw16.xchg_u",
            "i64.atomic.rmw32.and_u",
        ] {
            let arity = &map[*instr];
            assert_eq!(
                arity.operand_mode,
                OperandMode::Fixed(2),
                "{instr} operands"
            );
            assert_eq!(arity.produces, 1, "{instr} produces");
        }

        // Atomic cmpxchg: addr + expected + replacement → old value
        for instr in &[
            "i32.atomic.rmw.cmpxchg",
            "i64.atomic.rmw.cmpxchg",
            "i32.atomic.rmw8.cmpxchg_u",
            "i64.atomic.rmw32.cmpxchg_u",
        ] {
            let arity = &map[*instr];
            assert_eq!(
                arity.operand_mode,
                OperandMode::Fixed(3),
                "{instr} operands"
            );
            assert_eq!(arity.produces, 1, "{instr} produces");
        }

        // Wait/notify
        let wait32 = &map["memory.atomic.wait32"];
        assert_eq!(wait32.operand_mode, OperandMode::Fixed(3));
        assert_eq!(wait32.produces, 1);

        let wait64 = &map["memory.atomic.wait64"];
        assert_eq!(wait64.operand_mode, OperandMode::Fixed(3));
        assert_eq!(wait64.produces, 1);

        let notify = &map["memory.atomic.notify"];
        assert_eq!(notify.operand_mode, OperandMode::Fixed(2));
        assert_eq!(notify.produces, 1);
    }
}
