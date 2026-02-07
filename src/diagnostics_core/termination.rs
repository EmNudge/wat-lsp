//! Control flow termination analysis for both native and WASM builds.
//!
//! This module provides platform-agnostic functions to determine if a sequence
//! of instructions always terminates (never falls through).

// Allow useless_asref because kind.as_ref() is needed for WASM (String -> &str)
// but is a no-op for native (&str -> &str)
#![allow(clippy::useless_asref)]

use crate::instruction_metadata::is_terminating_instruction;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check if a sequence of instructions always terminates (never falls through).
/// This is used to determine whether a block's declared result types should be
/// pushed onto the stack - if the block always terminates, it doesn't produce
/// values via fall-through.
pub fn sequence_always_terminates(node: &Node, source: &str) -> bool {
    #[cfg(feature = "native")]
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = node.kind();

    match kind.as_ref() {
        // An instruction list terminates if ANY child instruction terminates
        "instr_list" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if sequence_always_terminates(&child, source) {
                    return true;
                }
            }
            false
        }

        // Check if this is a terminating instruction
        "instr" => {
            // instr can contain expr (folded), instr_plain, instr_block, etc.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if sequence_always_terminates(&child, source) {
                    return true;
                }
            }
            false
        }

        "instr_plain" => {
            if let Some(name) = get_instruction_name_from_node(node, source) {
                is_terminating_instruction(&name)
            } else {
                false
            }
        }

        // expr1, expr1_plain contain the actual instruction content
        "expr1" | "expr1_plain" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if sequence_always_terminates(&child, source) {
                    return true;
                }
            }
            false
        }

        // Block: check if the block's instruction list terminates
        "instr_block" | "block_block" => block_body_always_terminates(node, source),

        // Loop: check if the body terminates (loops can run forever but body might terminate)
        "instr_loop" | "loop_block" => block_body_always_terminates(node, source),

        // If: terminates only if BOTH then AND else branches exist AND both terminate
        // block_if is the linear format (if ... else ... end)
        "instr_if" | "if_block" | "block_if" => if_always_terminates(node, source),

        // Expr (folded expression): check inner content
        "expr" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if sequence_always_terminates(&child, source) {
                    return true;
                }
            }
            false
        }

        // For expr1_block, expr1_loop (folded block/loop syntax)
        "expr1_block" | "expr1_loop" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                #[cfg(feature = "native")]
                let child_kind = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let child_kind = child.kind();

                // Check the body expressions/instructions
                if (child_kind == "expr" || child_kind == "instr" || child_kind == "instr_list")
                    && sequence_always_terminates(&child, source)
                {
                    return true;
                }
            }
            false
        }

        // For expr1_if (folded if syntax)
        "expr1_if" => if_always_terminates(node, source),

        // For try_table (folded and linear formats)
        // A try_table with catch clauses can exit via catch branches to outer labels,
        // so it doesn't "always terminate" in the control flow sense.
        // Even if the body terminates (return/unreachable), catch clauses provide
        // alternative paths that branch to outer labels.
        // See: https://github.com/EmNudge/wat-lsp/issues/108
        "expr1_try_table" | "block_try_table" => try_table_always_terminates(node, source),

        _ => false,
    }
}

/// Extract the instruction name from a node (handles instr, instr_plain, etc.)
fn get_instruction_name_from_node(node: &Node, source: &str) -> Option<String> {
    #[cfg(feature = "native")]
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = node.kind();

    if kind == "instr" {
        // instr wraps other instruction types
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(name) = get_instruction_name_from_node(&child, source) {
                return Some(name);
            }
        }
        return None;
    }

    if kind == "instr_plain" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            #[cfg(feature = "native")]
            let child_kind = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let child_kind = child.kind();

            if child_kind.starts_with("op_") {
                let text = &source[child.byte_range()];
                return text.split_whitespace().next().map(|s| s.to_string());
            }
        }
        // Fallback
        let text = &source[node.byte_range()];
        return text.split_whitespace().next().map(|s| s.to_string());
    }

    None
}

/// Check if a block/loop body always terminates
fn block_body_always_terminates(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let child_kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let child_kind = child.kind();

        if child_kind == "instr_list" {
            return sequence_always_terminates(&child, source);
        }
        // For linear format blocks like block_block, loop_block
        if (child_kind == "block_block" || child_kind == "loop_block")
            && block_body_always_terminates(&child, source)
        {
            return true;
        }
        // block_if and if_block use if-specific termination (need both branches)
        if (child_kind == "block_if" || child_kind == "if_block")
            && if_always_terminates(&child, source)
        {
            return true;
        }
        // For folded expressions, check each expr/instr child
        if (child_kind == "expr" || child_kind == "instr" || child_kind == "instr_plain")
            && sequence_always_terminates(&child, source)
        {
            return true;
        }
    }
    false
}

/// Check if a try_table always terminates.
///
/// A try_table with catch clauses NEVER "always terminates" in the control flow sense,
/// because catch clauses can branch to outer labels, providing alternative exit paths.
/// Even if the try_table body always terminates (return/unreachable), the catch clause
/// can still branch to an outer block, allowing code after that block to be reached.
///
/// For example:
/// ```wat
/// (block $outer (result i32)
///   (try_table (result i32) (catch $tag $outer)
///     (return)))  ;; body terminates, but catch branches to $outer
/// (i32.const -1)  ;; reachable via catch branch to $outer
/// ```
fn try_table_always_terminates(node: &Node, source: &str) -> bool {
    // Check if this try_table has any catch clauses
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let child_kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let child_kind = child.kind();

        // If we find any catch clause, the try_table doesn't "always terminate"
        // because the catch can branch to an outer label
        if child_kind == "catch_clause" {
            return false;
        }
    }

    // No catch clauses found - check if the body terminates
    // (This is an edge case; try_table without catch clauses is unusual)
    block_body_always_terminates(node, source)
}

/// Check if an if/if_block always terminates (both branches must exist and terminate)
fn if_always_terminates(node: &Node, source: &str) -> bool {
    #[cfg(feature = "native")]
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = node.kind();

    // For folded if (expr1_if), we need to find then and else expressions
    if kind == "expr1_if" {
        let mut then_terminates = false;
        let mut else_terminates = false;
        let mut has_else = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            #[cfg(feature = "native")]
            let child_kind = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let child_kind = child.kind();

            // In folded if, the structure is:
            // (if (result ...) condition (then ...) (else ...))
            // We need to find the then/else parts
            if child_kind == "expr1_then" {
                then_terminates = sequence_always_terminates(&child, source);
            } else if child_kind == "expr1_else" {
                has_else = true;
                else_terminates = sequence_always_terminates(&child, source);
            }
        }

        // If without else does not terminate unconditionally
        return has_else && then_terminates && else_terminates;
    }

    // For linear format if (instr_if or if_block)
    let mut then_terminates = false;
    let mut else_terminates = false;
    let mut has_else = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        #[cfg(feature = "native")]
        let child_kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let child_kind = child.kind();

        if child_kind == "if_block" {
            // Recurse into if_block
            return if_always_terminates(&child, source);
        }

        if child_kind == "instr_list" {
            // First instr_list is the then branch
            if !then_terminates {
                then_terminates = sequence_always_terminates(&child, source);
            } else {
                // Second instr_list would be in else (but typically else has its own structure)
                has_else = true;
                else_terminates = sequence_always_terminates(&child, source);
            }
        }

        // Handle explicit else
        if child_kind == "else" || child_kind == "instr_else" {
            has_else = true;
            // Find the instr_list inside the else
            let mut else_cursor = child.walk();
            for else_child in child.children(&mut else_cursor) {
                #[cfg(feature = "native")]
                let else_kind = else_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let else_kind = else_child.kind();

                if else_kind == "instr_list" {
                    else_terminates = sequence_always_terminates(&else_child, source);
                    break;
                }
            }
        }
    }

    // If without else does not terminate unconditionally
    has_else && then_terminates && else_terminates
}
