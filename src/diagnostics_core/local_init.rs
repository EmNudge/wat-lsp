//! Uninitialized local detection for non-nullable reference types.
//!
//! Per the Wasm spec (GC proposal), locals with non-nullable reference types
//! (e.g., `ref $t`, `ref func`, `ref extern`) must be explicitly initialized
//! via `local.set` or `local.tee` before being read with `local.get`.
//!
//! Initialization tracking rules:
//! - Parameters are always initialized
//! - Locals with defaultable types (numeric, nullable ref) are always initialized
//! - After a block/loop, initialization state reverts to before the block
//! - After an if/else, only locals initialized in BOTH branches are considered initialized

use std::collections::HashSet;

use crate::core::types::Diagnostic;
use crate::symbols::{Function, SymbolTable};
use crate::utils::node_to_range;

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// Check for uses of uninitialized locals with non-defaultable types.
pub(crate) fn check_uninitialized_locals(
    func_node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let func_line = func_node.start_position().row as u32;
    let func = match symbols.find_function_containing_line(func_line) {
        Some(f) => f,
        None => return vec![],
    };

    // Scan the AST to find which locals have non-defaultable (non-nullable ref) types
    let non_defaultable = find_non_defaultable_locals(func_node, source, func);
    if non_defaultable.is_empty() {
        return vec![];
    }

    // Initialize tracking: params are always initialized, defaultable locals are initialized
    let num_params = func.parameters.len();
    let total = num_params + func.locals.len();
    let mut initialized: HashSet<usize> = (0..total)
        .filter(|i| !non_defaultable.contains(i))
        .collect();

    let mut diagnostics = Vec::new();

    // Find and walk the instr_list
    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = &*kind;
        if kind == "instr_list" {
            walk_body(
                &child,
                source,
                func,
                &non_defaultable,
                &mut initialized,
                &mut diagnostics,
            );
            break;
        }
    }

    diagnostics
}

/// Scan function AST for local declarations with non-nullable reference types.
/// Returns the set of absolute local indices (params.len() + local_index) that are non-defaultable.
fn find_non_defaultable_locals(func_node: &Node, source: &str, func: &Function) -> HashSet<usize> {
    let mut result = HashSet::new();
    let num_params = func.parameters.len();
    let mut local_counter = 0;

    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        let kind = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let kind = &*kind;

        if kind == "func_locals" || kind == "func_locals_one" || kind == "func_locals_many" {
            scan_locals_node(&child, source, num_params, &mut local_counter, &mut result);
        }
    }

    result
}

/// Recursively scan a locals node (func_locals, func_locals_one, func_locals_many)
/// for value_type children and check if they are non-nullable ref types.
fn scan_locals_node(
    node: &Node,
    source: &str,
    num_params: usize,
    local_counter: &mut usize,
    result: &mut HashSet<usize>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = &*kind;

    match kind {
        "value_type" | "value_type_ref_type" | "value_type_num_type" => {
            if is_non_nullable_ref_in_ast(node, source) {
                result.insert(num_params + *local_counter);
            }
            *local_counter += 1;
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                scan_locals_node(&child, source, num_params, local_counter, result);
            }
        }
    }
}

/// Check if a `value_type` AST node represents a non-nullable reference type.
/// Walks down through `value_type → value_type_ref_type → ref_type → ref_type_ref/ref_type_concrete`
/// and checks for the absence of `null`.
fn is_non_nullable_ref_in_ast(node: &Node, source: &str) -> bool {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = &*kind;

    match kind {
        "ref_type_ref" | "ref_type_concrete" => {
            // (ref null? ...) — non-nullable if no "null" child
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if &source[child.byte_range()] == "null" {
                    return false;
                }
            }
            true
        }
        // Shorthand types are always nullable
        "ref_type_funcref" | "ref_type_externref" => false,
        _ => {
            let text = source[node.byte_range()].trim();
            if matches!(
                text,
                "funcref"
                    | "externref"
                    | "anyref"
                    | "eqref"
                    | "i31ref"
                    | "structref"
                    | "arrayref"
                    | "nullref"
                    | "nullfuncref"
                    | "nullexternref"
                    | "exnref"
                    | "nullexnref"
            ) {
                return false;
            }
            // Recurse into children
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if is_non_nullable_ref_in_ast(&child, source) {
                    return true;
                }
            }
            false
        }
    }
}

/// Walk a block body (instr_list, block_block, etc.) tracking local initialization.
fn walk_body(
    node: &Node,
    source: &str,
    func: &Function,
    non_defaultable: &HashSet<usize>,
    initialized: &mut HashSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(
            &child,
            source,
            func,
            non_defaultable,
            initialized,
            diagnostics,
        );
    }
}

/// Process a single AST node for local initialization tracking.
fn walk_node(
    node: &Node,
    source: &str,
    func: &Function,
    non_defaultable: &HashSet<usize>,
    initialized: &mut HashSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = &*kind;

    match kind {
        "instr_plain" => {
            check_local_instruction(
                node,
                source,
                func,
                non_defaultable,
                initialized,
                diagnostics,
            );
        }

        // Folded expression: process operand sub-exprs first, then the instruction
        "expr1_plain" => {
            let mut cursor = node.walk();
            let mut instr_plain = None;
            for child in node.children(&mut cursor) {
                let ck = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ck = &*ck;
                if ck == "expr" {
                    walk_node(
                        &child,
                        source,
                        func,
                        non_defaultable,
                        initialized,
                        diagnostics,
                    );
                } else if ck == "instr_plain" {
                    // Save for processing after operands
                    instr_plain = Some(child);
                }
            }
            if let Some(ref ip) = instr_plain {
                check_local_instruction(
                    ip,
                    source,
                    func,
                    non_defaultable,
                    initialized,
                    diagnostics,
                );
            }
        }

        // Block/loop: save state, walk body, restore state
        "instr_block" | "instr_loop" => {
            let saved = initialized.clone();
            if is_block_if(node) {
                walk_if_body(
                    node,
                    source,
                    func,
                    non_defaultable,
                    initialized,
                    diagnostics,
                    &saved,
                );
            } else {
                walk_body(
                    node,
                    source,
                    func,
                    non_defaultable,
                    initialized,
                    diagnostics,
                );
            }
            *initialized = saved;
        }
        "block_block" | "loop_block" | "block_try_table" | "block_try" => {
            let saved = initialized.clone();
            walk_body(
                node,
                source,
                func,
                non_defaultable,
                initialized,
                diagnostics,
            );
            *initialized = saved;
        }

        // Folded block/loop
        "expr1_block" | "expr1_loop" | "expr1_try_table" => {
            let saved = initialized.clone();
            walk_body(
                node,
                source,
                func,
                non_defaultable,
                initialized,
                diagnostics,
            );
            *initialized = saved;
        }

        // If: save/restore same as block (control flow doesn't contribute to init)
        "instr_if" | "expr1_if" => {
            let saved = initialized.clone();
            walk_if_body(
                node,
                source,
                func,
                non_defaultable,
                initialized,
                diagnostics,
                &saved,
            );
            *initialized = saved;
        }
        "block_if" | "if_block" => {
            let saved = initialized.clone();
            walk_if_body(
                node,
                source,
                func,
                non_defaultable,
                initialized,
                diagnostics,
                &saved,
            );
            *initialized = saved;
        }

        // Default: recurse into children in order
        _ => {
            walk_body(
                node,
                source,
                func,
                non_defaultable,
                initialized,
                diagnostics,
            );
        }
    }
}

/// Walk if body, resetting init state at the else boundary.
/// Each branch sees the pre-if initialization state.
fn walk_if_body(
    node: &Node,
    source: &str,
    func: &Function,
    non_defaultable: &HashSet<usize>,
    initialized: &mut HashSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
    saved: &HashSet<usize>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = &*ck;

        match ck {
            // Linear format: else branch is in a separate node
            "instr_else" => {
                *initialized = saved.clone();
                walk_body(
                    &child,
                    source,
                    func,
                    non_defaultable,
                    initialized,
                    diagnostics,
                );
            }
            // Folded format: "else" keyword resets state
            "else" => {
                *initialized = saved.clone();
            }
            // if_block / block_if contain the then/else structure — recurse
            "if_block" | "block_if" => {
                walk_if_body(
                    &child,
                    source,
                    func,
                    non_defaultable,
                    initialized,
                    diagnostics,
                    saved,
                );
            }
            _ => {
                walk_node(
                    &child,
                    source,
                    func,
                    non_defaultable,
                    initialized,
                    diagnostics,
                );
            }
        }
    }
}

/// Check if a block node is actually a linear-format if (contains block_if child).
fn is_block_if(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = &*ck;
        if ck == "block_if" {
            return true;
        }
    }
    false
}

/// Check if an instr_plain is a local.get/set/tee and update initialization tracking.
fn check_local_instruction(
    node: &Node,
    source: &str,
    func: &Function,
    non_defaultable: &HashSet<usize>,
    initialized: &mut HashSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let text = &source[node.byte_range()];
    let first_token = text.split_whitespace().next().unwrap_or("");

    match first_token {
        "local.get" => {
            if let Some(idx) = resolve_local_index_from_node(node, source, func) {
                if non_defaultable.contains(&idx) && !initialized.contains(&idx) {
                    diagnostics.push(Diagnostic::error(
                        node_to_range(node),
                        "uninitialized local".to_string(),
                    ));
                }
            }
        }
        "local.set" | "local.tee" => {
            if let Some(idx) = resolve_local_index_from_node(node, source, func) {
                initialized.insert(idx);
            }
        }
        _ => {}
    }
}

/// Resolve the local index referenced by a local.get/set/tee instruction node.
fn resolve_local_index_from_node(node: &Node, source: &str, func: &Function) -> Option<usize> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = &*ck;

        if ck == "index" || ck == "identifier" || ck == "nat" {
            let text = source[child.byte_range()].trim();
            if let Some(idx) = resolve_local_text(text, func) {
                return Some(idx);
            }
            // Check inside index node for identifier/nat children
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                let ik = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ik = &*ik;
                if ik == "identifier" || ik == "nat" {
                    let id_text = source[idx_child.byte_range()].trim();
                    if let Some(idx) = resolve_local_text(id_text, func) {
                        return Some(idx);
                    }
                }
            }
        }
    }
    None
}

/// Resolve a text reference (name or numeric index) to an absolute local index.
fn resolve_local_text(text: &str, func: &Function) -> Option<usize> {
    if text.starts_with('$') {
        for param in &func.parameters {
            if param.name.as_deref() == Some(text) {
                return Some(param.index);
            }
        }
        for local in &func.locals {
            if local.name.as_deref() == Some(text) {
                return Some(func.parameters.len() + local.index);
            }
        }
    } else if let Ok(idx) = text.parse::<usize>() {
        return Some(idx);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_diagnostics(source: &str) -> Vec<String> {
        let mut parser = crate::tree_sitter_bindings::create_parser();
        let tree = parser.parse(source, None).unwrap();
        let symbols = crate::parser::parse_document(source).expect("Failed to extract symbols");
        let root = tree.root_node();

        let mut diagnostics = Vec::new();
        collect_local_init_diags(root, source, &symbols, &mut diagnostics);
        diagnostics.into_iter().map(|d| d.message).collect()
    }

    fn collect_local_init_diags(
        node: tree_sitter::Node,
        source: &str,
        symbols: &SymbolTable,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if node.kind() == "module_field_func" {
            diagnostics.extend(check_uninitialized_locals(&node, source, symbols));
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_local_init_diags(child, source, symbols, diagnostics);
        }
    }

    #[test]
    fn test_simple_uninit() {
        let src = r#"(module
            (type $t (func))
            (func (local $x (ref $t)) (drop (local.get $x)))
        )"#;
        let diags = get_diagnostics(src);
        assert!(
            diags.iter().any(|d| d.contains("uninitialized local")),
            "Expected uninitialized local error, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_init_before_get() {
        let src = r#"(module
            (type $t (func))
            (func (param $p (ref $t)) (local $x (ref $t))
                (local.set $x (local.get $p))
                (drop (local.get $x))
            )
        )"#;
        let diags = get_diagnostics(src);
        assert!(diags.is_empty(), "Expected no errors, got: {:?}", diags);
    }

    #[test]
    fn test_uninit_after_block() {
        let src = r#"(module
            (func (param $p (ref extern)) (local $x (ref extern))
                (block (local.set $x (local.get $p)) (drop (local.tee $x (local.get $p))))
                (drop (local.get $x))
            )
        )"#;
        let diags = get_diagnostics(src);
        assert!(
            diags.iter().any(|d| d.contains("uninitialized local")),
            "Expected uninitialized local error after block, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_uninit_in_else() {
        let src = r#"(module
            (func (param $p (ref extern)) (local $x (ref extern))
                (if (i32.const 0)
                    (then (local.set $x (local.get $p)))
                    (else (drop (local.get $x)))
                )
            )
        )"#;
        let diags = get_diagnostics(src);
        assert!(
            diags.iter().any(|d| d.contains("uninitialized local")),
            "Expected uninitialized local error in else, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_defaultable_local_ok() {
        // Nullable ref types are defaultable — no error expected
        let src = r#"(module
            (func (local $x funcref) (drop (local.get $x)))
        )"#;
        let diags = get_diagnostics(src);
        assert!(
            diags.is_empty(),
            "Expected no errors for funcref local, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_numeric_local_ok() {
        let src = r#"(module
            (func (local $x i32) (drop (local.get $x)))
        )"#;
        let diags = get_diagnostics(src);
        assert!(
            diags.is_empty(),
            "Expected no errors for i32 local, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_param_always_init() {
        let src = r#"(module
            (type $t (func))
            (func (param $p (ref $t)) (drop (local.get $p)))
        )"#;
        let diags = get_diagnostics(src);
        assert!(
            diags.is_empty(),
            "Expected no errors for param, got: {:?}",
            diags
        );
    }
}
