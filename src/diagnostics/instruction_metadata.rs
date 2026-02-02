use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperandMode {
    Fixed(usize),
    Dynamic, // Depends on type definition or other context
}

/// Represents the expected parameter count for a WAT instruction
#[derive(Debug, Clone)]
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

    pub fn is_valid(&self, param_count: usize) -> bool {
        param_count >= self.min_params && param_count <= self.max_params
    }

    #[cfg(test)]
    pub fn is_valid_operands(&self, operand_count: usize) -> bool {
        match self.operand_mode {
            OperandMode::Fixed(n) => operand_count == n,
            OperandMode::Dynamic => true, // Validated separately
        }
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

    pub fn expected_operands_message(&self) -> String {
        match self.operand_mode {
            OperandMode::Fixed(n) => match n {
                0 => "0 operands".to_string(),
                1 => "1 operand".to_string(),
                _ => format!("{} operands", n),
            },
            OperandMode::Dynamic => "variable operands".to_string(),
        }
    }
}

/// Returns a map of instruction names to their expected parameter counts
pub fn get_instruction_arity_map() -> HashMap<&'static str, InstructionArity> {
    let mut map = HashMap::new();

    // Local variable instructions (index-based, produce values)
    map.insert("local.get", InstructionArity::index("index"));
    map.insert("local.set", InstructionArity::index_set("index")); // consumes 1 value
    map.insert("local.tee", InstructionArity::index_tee("index")); // consumes 1 value, produces 1

    // Global variable instructions
    map.insert("global.get", InstructionArity::index("index"));
    map.insert("global.set", InstructionArity::index_set("index")); // consumes 1 value

    // Control flow instructions
    // br and br_if can pass values to blocks with result types (multi-value)
    map.insert("br", InstructionArity::dynamic(1, 1, "label index", 0)); // terminates
    map.insert("br_if", InstructionArity::dynamic(1, 1, "label index", 0)); // consumes condition + optional values
    map.insert("call", InstructionArity::dynamic(1, 1, "function index", 0)); // produces depend on function signature (set to 0, handled dynamically)
                                                                              // return can pass values for functions with result types
    map.insert("return", InstructionArity::dynamic(0, 0, "", 0)); // terminates
    map.insert("unreachable", InstructionArity::nullary()); // terminates
    map.insert("nop", InstructionArity::nullary());

    // Constant instructions (produce values)
    map.insert("i32.const", InstructionArity::constant("literal value"));
    map.insert("i64.const", InstructionArity::constant("literal value"));
    map.insert("f32.const", InstructionArity::constant("literal value"));
    map.insert("f64.const", InstructionArity::constant("literal value"));

    // Stack manipulation
    map.insert("drop", InstructionArity::drop_op()); // consumes 1 value, produces 0
    map.insert("select", InstructionArity::exact(0, "", 3, 1)); // consumes 3 values, produces 1

    // i32 arithmetic operations (binary)
    map.insert("i32.add", InstructionArity::binary_op());
    map.insert("i32.sub", InstructionArity::binary_op());
    map.insert("i32.mul", InstructionArity::binary_op());
    map.insert("i32.div_s", InstructionArity::binary_op());
    map.insert("i32.div_u", InstructionArity::binary_op());
    map.insert("i32.rem_s", InstructionArity::binary_op());
    map.insert("i32.rem_u", InstructionArity::binary_op());
    map.insert("i32.and", InstructionArity::binary_op());
    map.insert("i32.or", InstructionArity::binary_op());
    map.insert("i32.xor", InstructionArity::binary_op());
    map.insert("i32.shl", InstructionArity::binary_op());
    map.insert("i32.shr_s", InstructionArity::binary_op());
    map.insert("i32.shr_u", InstructionArity::binary_op());
    map.insert("i32.rotl", InstructionArity::binary_op());
    map.insert("i32.rotr", InstructionArity::binary_op());

    // i32 comparison operations (binary)
    map.insert("i32.eq", InstructionArity::binary_op());
    map.insert("i32.ne", InstructionArity::binary_op());
    map.insert("i32.lt_s", InstructionArity::binary_op());
    map.insert("i32.lt_u", InstructionArity::binary_op());
    map.insert("i32.le_s", InstructionArity::binary_op());
    map.insert("i32.le_u", InstructionArity::binary_op());
    map.insert("i32.gt_s", InstructionArity::binary_op());
    map.insert("i32.gt_u", InstructionArity::binary_op());
    map.insert("i32.ge_s", InstructionArity::binary_op());
    map.insert("i32.ge_u", InstructionArity::binary_op());

    // i32 unary operations
    map.insert("i32.clz", InstructionArity::unary_op());
    map.insert("i32.ctz", InstructionArity::unary_op());
    map.insert("i32.popcnt", InstructionArity::unary_op());
    map.insert("i32.eqz", InstructionArity::unary_op());

    // i64 arithmetic operations (binary)
    map.insert("i64.add", InstructionArity::binary_op());
    map.insert("i64.sub", InstructionArity::binary_op());
    map.insert("i64.mul", InstructionArity::binary_op());
    map.insert("i64.div_s", InstructionArity::binary_op());
    map.insert("i64.div_u", InstructionArity::binary_op());
    map.insert("i64.rem_s", InstructionArity::binary_op());
    map.insert("i64.rem_u", InstructionArity::binary_op());
    map.insert("i64.and", InstructionArity::binary_op());
    map.insert("i64.or", InstructionArity::binary_op());
    map.insert("i64.xor", InstructionArity::binary_op());
    map.insert("i64.shl", InstructionArity::binary_op());
    map.insert("i64.shr_s", InstructionArity::binary_op());
    map.insert("i64.shr_u", InstructionArity::binary_op());
    map.insert("i64.rotl", InstructionArity::binary_op());
    map.insert("i64.rotr", InstructionArity::binary_op());

    // i64 comparison operations (binary)
    map.insert("i64.eq", InstructionArity::binary_op());
    map.insert("i64.ne", InstructionArity::binary_op());
    map.insert("i64.lt_s", InstructionArity::binary_op());
    map.insert("i64.lt_u", InstructionArity::binary_op());
    map.insert("i64.le_s", InstructionArity::binary_op());
    map.insert("i64.le_u", InstructionArity::binary_op());
    map.insert("i64.gt_s", InstructionArity::binary_op());
    map.insert("i64.gt_u", InstructionArity::binary_op());
    map.insert("i64.ge_s", InstructionArity::binary_op());
    map.insert("i64.ge_u", InstructionArity::binary_op());

    // i64 unary operations
    map.insert("i64.clz", InstructionArity::unary_op());
    map.insert("i64.ctz", InstructionArity::unary_op());
    map.insert("i64.popcnt", InstructionArity::unary_op());
    map.insert("i64.eqz", InstructionArity::unary_op());

    // f32 arithmetic operations (binary)
    map.insert("f32.add", InstructionArity::binary_op());
    map.insert("f32.sub", InstructionArity::binary_op());
    map.insert("f32.mul", InstructionArity::binary_op());
    map.insert("f32.div", InstructionArity::binary_op());
    map.insert("f32.min", InstructionArity::binary_op());
    map.insert("f32.max", InstructionArity::binary_op());
    map.insert("f32.copysign", InstructionArity::binary_op());

    // f32 comparison operations (binary)
    map.insert("f32.eq", InstructionArity::binary_op());
    map.insert("f32.ne", InstructionArity::binary_op());
    map.insert("f32.lt", InstructionArity::binary_op());
    map.insert("f32.le", InstructionArity::binary_op());
    map.insert("f32.gt", InstructionArity::binary_op());
    map.insert("f32.ge", InstructionArity::binary_op());

    // f32 unary operations
    map.insert("f32.abs", InstructionArity::unary_op());
    map.insert("f32.neg", InstructionArity::unary_op());
    map.insert("f32.ceil", InstructionArity::unary_op());
    map.insert("f32.floor", InstructionArity::unary_op());
    map.insert("f32.trunc", InstructionArity::unary_op());
    map.insert("f32.nearest", InstructionArity::unary_op());
    map.insert("f32.sqrt", InstructionArity::unary_op());

    // f64 arithmetic operations (binary)
    map.insert("f64.add", InstructionArity::binary_op());
    map.insert("f64.sub", InstructionArity::binary_op());
    map.insert("f64.mul", InstructionArity::binary_op());
    map.insert("f64.div", InstructionArity::binary_op());
    map.insert("f64.min", InstructionArity::binary_op());
    map.insert("f64.max", InstructionArity::binary_op());
    map.insert("f64.copysign", InstructionArity::binary_op());

    // f64 comparison operations (binary)
    map.insert("f64.eq", InstructionArity::binary_op());
    map.insert("f64.ne", InstructionArity::binary_op());
    map.insert("f64.lt", InstructionArity::binary_op());
    map.insert("f64.le", InstructionArity::binary_op());
    map.insert("f64.gt", InstructionArity::binary_op());
    map.insert("f64.ge", InstructionArity::binary_op());

    // f64 unary operations
    map.insert("f64.abs", InstructionArity::unary_op());
    map.insert("f64.neg", InstructionArity::unary_op());
    map.insert("f64.ceil", InstructionArity::unary_op());
    map.insert("f64.floor", InstructionArity::unary_op());
    map.insert("f64.trunc", InstructionArity::unary_op());
    map.insert("f64.nearest", InstructionArity::unary_op());
    map.insert("f64.sqrt", InstructionArity::unary_op());

    // Conversion operations (unary)
    map.insert("i32.wrap_i64", InstructionArity::unary_op());
    map.insert("i64.extend_i32_s", InstructionArity::unary_op());
    map.insert("i64.extend_i32_u", InstructionArity::unary_op());
    map.insert("i32.trunc_f32_s", InstructionArity::unary_op());
    map.insert("i32.trunc_f32_u", InstructionArity::unary_op());
    map.insert("i32.trunc_f64_s", InstructionArity::unary_op());
    map.insert("i32.trunc_f64_u", InstructionArity::unary_op());
    map.insert("i64.trunc_f32_s", InstructionArity::unary_op());
    map.insert("i64.trunc_f32_u", InstructionArity::unary_op());
    map.insert("i64.trunc_f64_s", InstructionArity::unary_op());
    map.insert("i64.trunc_f64_u", InstructionArity::unary_op());
    map.insert("f32.convert_i32_s", InstructionArity::unary_op());
    map.insert("f32.convert_i32_u", InstructionArity::unary_op());
    map.insert("f32.convert_i64_s", InstructionArity::unary_op());
    map.insert("f32.convert_i64_u", InstructionArity::unary_op());
    map.insert("f32.demote_f64", InstructionArity::unary_op());
    map.insert("f64.convert_i32_s", InstructionArity::unary_op());
    map.insert("f64.convert_i32_u", InstructionArity::unary_op());
    map.insert("f64.convert_i64_s", InstructionArity::unary_op());
    map.insert("f64.convert_i64_u", InstructionArity::unary_op());
    map.insert("f64.promote_f32", InstructionArity::unary_op());
    map.insert("i32.reinterpret_f32", InstructionArity::unary_op());
    map.insert("i64.reinterpret_f64", InstructionArity::unary_op());
    map.insert("f32.reinterpret_i32", InstructionArity::unary_op());
    map.insert("f64.reinterpret_i64", InstructionArity::unary_op());

    // Memory load instructions (optional memory index, consume address)
    map.insert("i32.load", InstructionArity::mem_load());
    map.insert("i64.load", InstructionArity::mem_load());
    map.insert("f32.load", InstructionArity::mem_load());
    map.insert("f64.load", InstructionArity::mem_load());
    map.insert("i32.load8_s", InstructionArity::mem_load());
    map.insert("i32.load8_u", InstructionArity::mem_load());
    map.insert("i32.load16_s", InstructionArity::mem_load());
    map.insert("i32.load16_u", InstructionArity::mem_load());
    map.insert("i64.load8_s", InstructionArity::mem_load());
    map.insert("i64.load8_u", InstructionArity::mem_load());
    map.insert("i64.load16_s", InstructionArity::mem_load());
    map.insert("i64.load16_u", InstructionArity::mem_load());
    map.insert("i64.load32_s", InstructionArity::mem_load());
    map.insert("i64.load32_u", InstructionArity::mem_load());

    // Memory store instructions (optional memory index, consume address and value)
    map.insert("i32.store", InstructionArity::mem_store());
    map.insert("i64.store", InstructionArity::mem_store());
    map.insert("f32.store", InstructionArity::mem_store());
    map.insert("f64.store", InstructionArity::mem_store());
    map.insert("i32.store8", InstructionArity::mem_store());
    map.insert("i32.store16", InstructionArity::mem_store());
    map.insert("i64.store8", InstructionArity::mem_store());
    map.insert("i64.store16", InstructionArity::mem_store());
    map.insert("i64.store32", InstructionArity::mem_store());

    // Memory management - support optional memory index (multi-memory proposal)
    map.insert(
        "memory.size",
        InstructionArity {
            min_params: 0,
            max_params: 1,
            param_description: "optional memory index",
            operand_mode: OperandMode::Fixed(0),
            produces: 1, // produces i32 (current size)
        },
    );
    map.insert(
        "memory.grow",
        InstructionArity {
            min_params: 0,
            max_params: 1,
            param_description: "optional memory index",
            operand_mode: OperandMode::Fixed(1), // consumes delta
            produces: 1,                         // produces i32 (previous size or -1)
        },
    );

    // WasmGC Instructions
    // Structs
    map.insert(
        "struct.new",
        InstructionArity::dynamic(1, 1, "type index", 1),
    ); // produces structref
    map.insert(
        "struct.new_default",
        InstructionArity::exact(1, "type index", 0, 1), // produces structref
    );
    map.insert(
        "struct.get",
        InstructionArity::exact(2, "type and field index", 1, 1), // consumes structref, produces value
    );
    map.insert(
        "struct.get_s",
        InstructionArity::exact(2, "type and field index", 1, 1),
    );
    map.insert(
        "struct.get_u",
        InstructionArity::exact(2, "type and field index", 1, 1),
    );
    map.insert(
        "struct.set",
        InstructionArity::exact(2, "type and field index", 2, 0), // consumes structref + value, produces nothing
    );

    // Arrays
    map.insert("array.new", InstructionArity::exact(1, "type index", 2, 1)); // value, len -> arrayref
    map.insert(
        "array.new_default",
        InstructionArity::exact(1, "type index", 1, 1), // len -> arrayref
    );
    map.insert(
        "array.new_fixed",
        InstructionArity::dynamic(2, 2, "type index and length", 1), // -> arrayref
    );
    map.insert(
        "array.new_data",
        InstructionArity::exact(2, "type and data index", 2, 1), // offset, len -> arrayref
    );
    map.insert(
        "array.new_elem",
        InstructionArity::exact(2, "type and elem index", 2, 1), // offset, len -> arrayref
    );
    map.insert("array.get", InstructionArity::exact(1, "type index", 2, 1)); // arrayref, index -> value
    map.insert(
        "array.get_s",
        InstructionArity::exact(1, "type index", 2, 1),
    );
    map.insert(
        "array.get_u",
        InstructionArity::exact(1, "type index", 2, 1),
    );
    map.insert("array.set", InstructionArity::exact(1, "type index", 3, 0)); // arrayref, index, value -> nothing
    map.insert("array.len", InstructionArity::unary_op()); // arrayref -> i32
    map.insert("array.fill", InstructionArity::exact(1, "type index", 4, 0)); // arrayref, index, value, len -> nothing
    map.insert(
        "array.copy",
        InstructionArity::exact(2, "dest and src type index", 5, 0), // dest, dest_idx, src, src_idx, len -> nothing
    );

    // I31
    map.insert("ref.i31", InstructionArity::unary_op()); // i32 -> i31ref
    map.insert("i31.get_s", InstructionArity::unary_op()); // i31ref -> i32
    map.insert("i31.get_u", InstructionArity::unary_op()); // i31ref -> i32

    // Casts
    map.insert("ref.test", InstructionArity::exact(1, "type index", 1, 1)); // ref -> i32
    map.insert("ref.cast", InstructionArity::exact(1, "type index", 1, 1)); // ref -> ref
    map.insert(
        "ref.cast_null",
        InstructionArity::exact(1, "type index", 1, 1),
    ); // ref -> ref
    map.insert(
        "br_on_cast",
        InstructionArity::exact(2, "label and type index", 1, 0), // ref -> (branches or continues)
    );
    map.insert(
        "br_on_cast_fail",
        InstructionArity::exact(2, "label and type index", 1, 0), // ref -> (branches or continues)
    );

    // Exceptions
    map.insert("throw", InstructionArity::dynamic(1, 1, "tag index", 0)); // terminates
    map.insert("throw_ref", InstructionArity::exact(0, "", 1, 0)); // exnref -> terminates (unary but not returning)
    map.insert("rethrow", InstructionArity::exact(1, "label index", 0, 0)); // terminates

    // Typed function references
    map.insert("call_ref", InstructionArity::dynamic(1, 1, "type index", 0)); // args + funcref -> dynamic results
    map.insert(
        "return_call_ref",
        InstructionArity::dynamic(1, 1, "type index", 0), // args + funcref -> terminates (tail call)
    );

    // Null-checking branches
    map.insert(
        "br_on_null",
        InstructionArity::exact(1, "label index", 1, 1),
    ); // ref -> non-null ref (or branches)
    map.insert(
        "br_on_non_null",
        InstructionArity::exact(1, "label index", 1, 0), // ref -> (branches with non-null or continues with nothing)
    );

    // Reference equality
    map.insert("ref.eq", InstructionArity::binary_op()); // eqref, eqref -> i32

    // Reference conversions
    map.insert("any.convert_extern", InstructionArity::unary_op()); // externref -> anyref
    map.insert("extern.convert_any", InstructionArity::unary_op()); // anyref -> externref

    // Array initialization from segments
    map.insert(
        "array.init_data",
        InstructionArity::exact(2, "type and data index", 4, 0), // array, dst, src, len -> nothing
    );
    map.insert(
        "array.init_elem",
        InstructionArity::exact(2, "type and elem index", 4, 0), // array, dst, src, len -> nothing
    );

    // Bulk memory operations - support optional memory indices (multi-memory proposal)
    map.insert(
        "memory.copy",
        InstructionArity {
            min_params: 0,
            max_params: 2,
            param_description: "optional dest and src memory indices",
            operand_mode: OperandMode::Fixed(3), // dest offset, src offset, len
            produces: 0,
        },
    );
    map.insert(
        "memory.fill",
        InstructionArity {
            min_params: 0,
            max_params: 1,
            param_description: "optional memory index",
            operand_mode: OperandMode::Fixed(3), // dest, val, len
            produces: 0,
        },
    );
    map.insert(
        "memory.init",
        InstructionArity {
            min_params: 1,
            max_params: 2,
            param_description: "data index and optional memory index",
            operand_mode: OperandMode::Fixed(3), // dest, offset, len
            produces: 0,
        },
    );
    map.insert("data.drop", InstructionArity::exact(1, "data index", 0, 0));

    // Table operations
    map.insert("table.get", InstructionArity::exact(1, "table index", 1, 1)); // index -> ref
    map.insert("table.set", InstructionArity::exact(1, "table index", 2, 0)); // index, value -> nothing
    map.insert(
        "table.size",
        InstructionArity::exact(1, "table index", 0, 1),
    ); // -> i32
    map.insert(
        "table.grow",
        InstructionArity::exact(1, "table index", 2, 1),
    ); // init, delta -> i32
    map.insert(
        "table.fill",
        InstructionArity::exact(1, "table index", 3, 0),
    ); // index, value, len -> nothing
    map.insert(
        "table.copy",
        InstructionArity::exact(2, "dest and src table index", 3, 0), // dest, src, len -> nothing
    );
    map.insert(
        "table.init",
        InstructionArity::exact(2, "table and elem index", 3, 0), // dest, offset, len -> nothing
    );
    map.insert("elem.drop", InstructionArity::exact(1, "elem index", 0, 0));

    // Reference operations
    map.insert("ref.null", InstructionArity::exact(1, "type", 0, 1)); // -> null ref
    map.insert(
        "ref.func",
        InstructionArity::exact(1, "function index", 0, 1),
    ); // -> funcref
    map.insert("ref.is_null", InstructionArity::unary_op()); // ref -> i32
    map.insert("ref.as_non_null", InstructionArity::unary_op()); // nullable ref -> non-null ref

    // Saturating truncation operations
    map.insert("i32.trunc_sat_f32_s", InstructionArity::unary_op());
    map.insert("i32.trunc_sat_f32_u", InstructionArity::unary_op());
    map.insert("i32.trunc_sat_f64_s", InstructionArity::unary_op());
    map.insert("i32.trunc_sat_f64_u", InstructionArity::unary_op());
    map.insert("i64.trunc_sat_f32_s", InstructionArity::unary_op());
    map.insert("i64.trunc_sat_f32_u", InstructionArity::unary_op());
    map.insert("i64.trunc_sat_f64_s", InstructionArity::unary_op());
    map.insert("i64.trunc_sat_f64_u", InstructionArity::unary_op());

    // Sign extension operations
    map.insert("i32.extend8_s", InstructionArity::unary_op());
    map.insert("i32.extend16_s", InstructionArity::unary_op());
    map.insert("i64.extend8_s", InstructionArity::unary_op());
    map.insert("i64.extend16_s", InstructionArity::unary_op());
    map.insert("i64.extend32_s", InstructionArity::unary_op());

    map
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
    fn test_expected_message() {
        let arity_exact = InstructionArity::exact(1, "index", 0, 1);
        assert_eq!(arity_exact.expected_message(), "1 (index)");

        let arity_exact_no_desc = InstructionArity::exact(0, "", 2, 1);
        assert_eq!(arity_exact_no_desc.expected_message(), "0");
    }

    #[test]
    fn test_binary_op() {
        let arity = InstructionArity::binary_op();
        assert_eq!(arity.operand_mode, OperandMode::Fixed(2));
        assert_eq!(arity.produces, 1);
        assert!(arity.is_valid_operands(2));
        assert!(!arity.is_valid_operands(1));
        assert!(!arity.is_valid_operands(3));
    }

    #[test]
    fn test_unary_op() {
        let arity = InstructionArity::unary_op();
        assert_eq!(arity.operand_mode, OperandMode::Fixed(1));
        assert_eq!(arity.produces, 1);
        assert!(arity.is_valid_operands(1));
        assert!(!arity.is_valid_operands(0));
        assert!(!arity.is_valid_operands(2));
    }

    #[test]
    fn test_drop_op() {
        let arity = InstructionArity::drop_op();
        assert_eq!(arity.operand_mode, OperandMode::Fixed(1));
        assert_eq!(arity.produces, 0);
    }

    #[test]
    fn test_produces_field() {
        let map = get_instruction_arity_map();

        // Constants produce 1 value
        assert_eq!(map.get("i32.const").unwrap().produces, 1);
        assert_eq!(map.get("f64.const").unwrap().produces, 1);

        // Binary ops consume 2, produce 1
        assert_eq!(map.get("i32.add").unwrap().produces, 1);

        // drop consumes 1, produces 0
        assert_eq!(map.get("drop").unwrap().produces, 0);

        // local.get produces 1
        assert_eq!(map.get("local.get").unwrap().produces, 1);

        // local.set produces 0
        assert_eq!(map.get("local.set").unwrap().produces, 0);

        // local.tee produces 1
        assert_eq!(map.get("local.tee").unwrap().produces, 1);

        // Memory load produces 1
        assert_eq!(map.get("i32.load").unwrap().produces, 1);

        // Memory store produces 0
        assert_eq!(map.get("i32.store").unwrap().produces, 0);

        // select produces 1
        assert_eq!(map.get("select").unwrap().produces, 1);
    }

    #[test]
    fn test_instruction_map_contains_common_instructions() {
        let map = get_instruction_arity_map();

        // Local instructions
        assert!(map.contains_key("local.get"));
        assert!(map.contains_key("local.set"));
        assert!(map.contains_key("local.tee"));

        // Global instructions
        assert!(map.contains_key("global.get"));
        assert!(map.contains_key("global.set"));

        // Constants
        assert!(map.contains_key("i32.const"));
        assert!(map.contains_key("f32.const"));

        // Control flow
        assert!(map.contains_key("br"));
        assert!(map.contains_key("call"));
        assert!(map.contains_key("return"));
    }

    #[test]
    fn test_local_set_expects_one_param() {
        let map = get_instruction_arity_map();
        let arity = map.get("local.set").unwrap();
        assert_eq!(arity.min_params, 1);
        assert_eq!(arity.max_params, 1);
        assert_eq!(arity.param_description, "index");
    }
}
