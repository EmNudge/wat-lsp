//! Control flow termination analysis for both native and WASM builds.
//!
//! This module provides platform-agnostic functions to determine if a sequence
//! of instructions always terminates (never falls through).

// Allow useless_asref because kind.as_ref() is needed for WASM (String -> &str)
// but is a no-op for native (&str -> &str)
#![allow(clippy::useless_asref)]

use crate::instruction_metadata::is_terminating_instruction;

use super::semantic::get_instruction_name;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check if a sequence of instructions always terminates (never falls through).
/// This is used to determine whether a block's declared result types should be
/// pushed onto the stack - if the block always terminates, it doesn't produce
/// values via fall-through.
pub(crate) fn sequence_always_terminates(node: &Node, source: &str) -> bool {
    node_kind!(kind = node);

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
            if let Some(name) = get_instruction_name(node, source) {
                is_terminating_instruction(name)
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
        "instr_loop" | "block_loop" => block_body_always_terminates(node, source),

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
                node_kind!(child_kind = child);

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

/// Check if a block/loop body always terminates
fn block_body_always_terminates(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(child_kind = child);

        if child_kind == "instr_list" {
            return sequence_always_terminates(&child, source);
        }
        // For linear format blocks like block_block, block_loop
        if (child_kind == "block_block" || child_kind == "block_loop")
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
        node_kind!(child_kind = child);

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
    node_kind!(kind = node);

    // For folded if (expr1_if), we need to find then and else expressions
    if kind == "expr1_if" {
        let mut then_terminates = false;
        let mut else_terminates = false;
        let mut has_else = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            node_kind!(child_kind = child);

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
        node_kind!(child_kind = child);

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
                node_kind!(else_kind = else_child);

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

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::sequence_always_terminates;
    use crate::tree_sitter_bindings::create_parser;
    use tree_sitter::{Node, Tree};

    /// Depth-first search for the first node of `kind` in the tree.
    fn find_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_kind(child, kind) {
                return Some(found);
            }
        }
        None
    }

    /// Parse `src` and evaluate `sequence_always_terminates` on the first
    /// function-body `instr_list`.
    fn body_terminates(src: &str) -> bool {
        let mut parser = create_parser();
        let tree: Tree = parser.parse(src, None).unwrap();
        // The func body is the top-level `instr_list` inside the function.
        let list = find_kind(tree.root_node(), "instr_list")
            .expect("expected an instr_list in the parsed function");
        sequence_always_terminates(&list, src)
    }

    #[test]
    fn plain_sequence_does_not_terminate() {
        assert!(!body_terminates(
            "(func $f (result i32) (i32.const 1) (i32.const 2) (i32.add))"
        ));
    }

    #[test]
    fn return_terminates() {
        assert!(body_terminates(
            "(func $f (result i32) (return (i32.const 1)))"
        ));
    }

    #[test]
    fn unreachable_terminates() {
        assert!(body_terminates("(func $f (result i32) unreachable)"));
    }

    #[test]
    fn br_terminates() {
        assert!(body_terminates(
            "(func $f (result i32) (block (result i32) (br 0 (i32.const 1))))"
        ));
    }

    #[test]
    fn linear_if_terminates_only_with_both_branches() {
        // Linear if with only a `then` branch that returns: not unconditional.
        assert!(!body_terminates(
            "(func $f (result i32) i32.const 1 if (result i32) \
             i32.const 1 return end)"
        ));
        // Both branches return: the whole if always terminates.
        assert!(body_terminates(
            "(func $f (result i32) i32.const 1 if (result i32) \
             i32.const 1 return else i32.const 2 return end)"
        ));
    }

    #[test]
    fn if_without_else_does_not_terminate() {
        // A folded/linear if lacking an else branch never terminates
        // unconditionally, regardless of what the then branch does.
        assert!(!body_terminates(
            "(func $f (result i32) (if (i32.const 1) (then (return (i32.const 1)))))"
        ));
    }

    #[test]
    fn try_table_with_catch_does_not_terminate() {
        // A catch clause can branch to an outer label, so the try_table does not
        // always terminate even though its body returns. Guards issue #108.
        let src = "(module (tag $e) (func $f (result i32) \
                   (block $out (result i32) \
                   (try_table (result i32) (catch $e $out) (return (i32.const 1))))))";
        let mut parser = create_parser();
        let tree = parser.parse(src, None).unwrap();
        let try_table = find_kind(tree.root_node(), "block_try_table")
            .or_else(|| find_kind(tree.root_node(), "expr1_try_table"));
        if let Some(node) = try_table {
            assert!(!sequence_always_terminates(&node, src));
        }
    }

    #[test]
    fn terminates_within_recursion_depth_and_halts() {
        // Deeply nested folded blocks must terminate the recursion (no infinite
        // loop / stack blowup) and report a stable answer.
        let mut src = String::from("(func $f (result i32) ");
        let depth = 200;
        for _ in 0..depth {
            src.push_str("(block (result i32) ");
        }
        src.push_str("(return (i32.const 1))");
        for _ in 0..depth {
            src.push(')');
        }
        src.push(')');
        // Should return (halt) without overflowing; a return inside terminates.
        assert!(body_terminates(&src));
    }
}
