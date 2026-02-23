//! Shared memory-related diagnostic checks for both native and WASM builds.
//!
//! Provides checks for:
//! - Atomic operations on non-shared memory
//! - Memory64 type hints for load/store/size/grow operations

use crate::core::types::Diagnostic;
use crate::symbols::SymbolTable;
use crate::utils::node_to_range;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check if an atomic operation is used on non-shared memory
pub(crate) fn check_atomic_operation(
    node: &Node,
    first_token: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !is_atomic_memory_operation(first_token) {
        return;
    }

    diagnostics.push(Diagnostic::warning(
        node_to_range(node),
        format!(
            "Atomic operation '{}' requires shared memory. Declare memory with 'shared' keyword: (memory 1 1 shared)",
            first_token
        ),
    ));
}

/// Check memory64 hints for a specific instruction
pub(crate) fn check_memory64_for_instruction(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    first_token: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Check memory.size and memory.grow
    if first_token == "memory.size" || first_token == "memory.grow" {
        if let Some(memory) = get_memory_for_instruction(node, source, symbols) {
            if memory.is_memory64 {
                let param_info = if first_token == "memory.grow" {
                    " and takes i64 delta parameter"
                } else {
                    ""
                };
                diagnostics.push(Diagnostic::hint(
                    node_to_range(node),
                    format!("'{}' on memory64 returns i64{}", first_token, param_info),
                ));
            }
        }
    }

    // Check load/store operations
    if is_memory_load_store(first_token) {
        if let Some(memory) = get_memory_for_instruction(node, source, symbols) {
            if memory.is_memory64 {
                diagnostics.push(Diagnostic::hint(
                    node_to_range(node),
                    format!("'{}' on memory64 requires i64 address operand", first_token),
                ));
            }
        }
    }

    // Check bulk memory operations
    if first_token == "memory.copy" || first_token == "memory.fill" {
        if let Some(memory) = get_memory_for_instruction(node, source, symbols) {
            if memory.is_memory64 {
                diagnostics.push(Diagnostic::hint(
                    node_to_range(node),
                    format!(
                        "'{}' on memory64 requires i64 address operands",
                        first_token
                    ),
                ));
            }
        }
    }
}

/// Check if an instruction is an atomic memory operation that requires shared memory
pub(crate) fn is_atomic_memory_operation(instr: &str) -> bool {
    if instr == "atomic.fence" {
        return false;
    }
    instr.contains(".atomic.") || instr.starts_with("memory.atomic.")
}

/// Get the memory referenced by an instruction
/// Returns the memory at index 0 if no explicit memory index is specified
fn get_memory_for_instruction<'a>(
    node: &Node,
    source: &str,
    symbols: &'a SymbolTable,
) -> Option<&'a crate::symbols::Memory> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind_ref = child);

        if kind_ref == "index" {
            let index_text = &source[child.byte_range()];
            if index_text.starts_with('$') {
                return symbols.get_memory_by_name(index_text);
            }
            if let Ok(idx) = index_text.parse::<usize>() {
                return symbols.memories.get(idx);
            }
        }
    }
    symbols.memories.first()
}

/// Check if an instruction is a memory load or store operation
pub(crate) fn is_memory_load_store(instr: &str) -> bool {
    if instr.ends_with(".load")
        || instr.contains(".load8_")
        || instr.contains(".load16_")
        || instr.contains(".load32_")
    {
        return true;
    }

    if instr.ends_with(".store")
        || instr.contains(".store8")
        || instr.contains(".store16")
        || instr.contains(".store32")
    {
        return true;
    }

    if instr.starts_with("v128.load") || instr.starts_with("v128.store") {
        return true;
    }

    false
}
