//! Module-level structural validations for WAT modules.
//!
//! This module provides platform-agnostic checks for:
//! - Memory limits exceeding 65536 pages (or 2^48 for memory64)
//! - Min > max for memories and tables
//! - Multiple start sections
//! - Duplicate export names
//! - Import ordering (imports must precede definitions)
//! - Duplicate identifiers within the same index space
//! - Duplicate local/parameter names within functions
//! - Inline function type mismatches
//! - Constant expression validation
//! - Block label mismatches

#![allow(clippy::needless_borrow, clippy::borrow_deref_ref)]

use std::collections::{HashMap, HashSet};

use crate::core::types::{Diagnostic, Range};
use crate::symbols::{SymbolTable, TypeKind};
use crate::utils::node_to_range;

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

// ============================================================================
// Main entry point
// ============================================================================

/// Validate module-level structure and return diagnostics.
pub fn validate_module_structure(
    root: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_memory_limits(symbols, &mut diagnostics);
    check_min_gt_max(symbols, &mut diagnostics);
    check_duplicate_locals(symbols, &mut diagnostics);

    // Find the module node for AST-based checks
    if let Some(module_node) = find_module_node(root) {
        check_multiple_starts(&module_node, source, &mut diagnostics);
        check_start_signature(&module_node, source, symbols, &mut diagnostics);
        check_duplicate_exports(&module_node, source, &mut diagnostics);
        check_import_ordering(&module_node, source, &mut diagnostics);
        check_duplicate_identifiers(&module_node, source, &mut diagnostics);
        check_inline_type_mismatches(&module_node, source, symbols, &mut diagnostics);
        check_constant_expressions(&module_node, source, symbols, &mut diagnostics);
        check_data_segment_memory_indices(&module_node, source, symbols, &mut diagnostics);
        check_elem_segment_table_indices(&module_node, source, symbols, &mut diagnostics);
        check_ref_func_declarations(&module_node, source, symbols, &mut diagnostics);
    }

    diagnostics
}

// ============================================================================
// Helper: find the module node
// ============================================================================

#[cfg(feature = "native")]
fn find_module_node<'a>(root: &Node<'a>) -> Option<Node<'a>> {
    if root.kind() == "module" {
        return Some(*root);
    }
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            return Some(child);
        }
        if child.kind() == "module_field" {
            return Some(*root);
        }
    }
    None
}

#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_module_node(root: &Node) -> Option<Node> {
    let kind = root.kind();
    if kind == "module" {
        return Some(root.clone());
    }
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            return Some(child);
        }
        if child.kind() == "module_field" {
            return Some(root.clone());
        }
    }
    None
}

/// Helper: iterate module_field children of a module or root node.
/// Handles both `(module ...)` form and bare module fields.
fn for_each_module_field(module: &Node, mut f: impl FnMut(&Node)) {
    let mut cursor = module.walk();
    for child in module.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "module_field" {
            // Each module_field wraps one actual field
            let mut fc = child.walk();
            for field_child in child.children(&mut fc) {
                f(&field_child);
            }
        }
    }
}

/// Helper: extract text from a node
fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

// ============================================================================
// 1. Memory limits > 65536 pages
// ============================================================================

fn check_memory_limits(symbols: &SymbolTable, diagnostics: &mut Vec<Diagnostic>) {
    const MAX_MEMORY32_PAGES: u64 = 65536;
    const MAX_MEMORY64_PAGES: u64 = 1 << 48;

    for memory in &symbols.memories {
        let max_pages = if memory.is_memory64 {
            MAX_MEMORY64_PAGES
        } else {
            MAX_MEMORY32_PAGES
        };

        let range = memory
            .range
            .unwrap_or_else(|| Range::from_coords(memory.line, 0, memory.line, 0));

        if memory.limits.0 > max_pages {
            diagnostics.push(Diagnostic::error(
                range,
                format!(
                    "Memory size must be at most {} pages (2^{} bytes)",
                    max_pages,
                    if memory.is_memory64 { 64 } else { 32 }
                ),
            ));
        }

        if let Some(max) = memory.limits.1 {
            if max > max_pages {
                diagnostics.push(Diagnostic::error(
                    range,
                    format!(
                        "Memory size must be at most {} pages (2^{} bytes)",
                        max_pages,
                        if memory.is_memory64 { 64 } else { 32 }
                    ),
                ));
            }
        }
    }
}

// ============================================================================
// 2. Min > max for memories and tables
// ============================================================================

fn check_min_gt_max(symbols: &SymbolTable, diagnostics: &mut Vec<Diagnostic>) {
    for memory in &symbols.memories {
        if let Some(max) = memory.limits.1 {
            if memory.limits.0 > max {
                let range = memory
                    .range
                    .unwrap_or_else(|| Range::from_coords(memory.line, 0, memory.line, 0));
                diagnostics.push(Diagnostic::error(
                    range,
                    "Size minimum must not be greater than maximum",
                ));
            }
        }
    }

    for table in &symbols.tables {
        if let Some(max) = table.limits.1 {
            if table.limits.0 > max {
                let range = table
                    .range
                    .unwrap_or_else(|| Range::from_coords(table.line, 0, table.line, 0));
                diagnostics.push(Diagnostic::error(
                    range,
                    "Size minimum must not be greater than maximum",
                ));
            }
        }
    }
}

// ============================================================================
// 3. Multiple start sections
// ============================================================================

fn check_multiple_starts(module: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let _ = source;
    let mut seen_start = false;
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk == "module_field_start" {
            if seen_start {
                diagnostics.push(Diagnostic::error(
                    node_to_range(field),
                    "Multiple start sections",
                ));
            } else {
                seen_start = true;
            }
        }
    });
}

// ============================================================================
// 3b. Start function signature must be [] -> []
// ============================================================================

fn check_start_signature(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk != "module_field_start" {
            return;
        }

        // Find the function index referenced by start
        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            let ck = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let ck = ck.as_str();
            if ck == "index" || ck == "identifier" {
                let text = node_text(&child, source);
                let func = if text.starts_with('$') {
                    symbols.get_function_by_name(text)
                } else if let Ok(idx) = text.parse::<usize>() {
                    symbols.get_function_by_index(idx)
                } else {
                    // Might be an index node wrapping an identifier/nat
                    let mut ic = child.walk();
                    let mut found = None;
                    for idx_child in child.children(&mut ic) {
                        let ik = idx_child.kind();
                        #[cfg(all(feature = "wasm", not(feature = "native")))]
                        let ik = ik.as_str();
                        if ik == "identifier" {
                            let id_text = node_text(&idx_child, source);
                            found = symbols.get_function_by_name(id_text);
                        } else if ik == "nat" {
                            let nat_text = node_text(&idx_child, source);
                            if let Ok(idx) = nat_text.parse::<usize>() {
                                found = symbols.get_function_by_index(idx);
                            }
                        }
                    }
                    found
                };

                if let Some(func) = func {
                    if !func.parameters.is_empty() || !func.results.is_empty() {
                        diagnostics.push(Diagnostic::error(node_to_range(field), "start function"));
                    }
                }
                return;
            }
        }
    });
}

// ============================================================================
// 4. Duplicate export names
// ============================================================================

fn check_duplicate_exports(module: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen_exports: HashMap<String, Range> = HashMap::new();

    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();

        match fk {
            // Standalone export: (export "name" (func $idx))
            "module_field_export" => {
                if let Some(name) = extract_export_name(field, source) {
                    check_export_name(name, field, &mut seen_exports, diagnostics);
                }
            }
            // Inline exports on func/global/table/memory/tag
            "module_field_func"
            | "module_field_global"
            | "module_field_table"
            | "module_field_memory"
            | "module_field_tag" => {
                let mut cursor = field.walk();
                for child in field.children(&mut cursor) {
                    let ck = child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let ck = ck.as_str();
                    if ck == "export" {
                        if let Some(name) = extract_name_child(&child, source) {
                            check_export_name(name, &child, &mut seen_exports, diagnostics);
                        }
                    }
                }
            }
            _ => {}
        }
    });
}

fn extract_export_name<'a>(node: &Node, source: &'a str) -> Option<&'a str> {
    extract_name_child(node, source)
}

fn extract_name_child<'a>(node: &Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "name" {
            let text = node_text(&child, source);
            // name node wraps a string node; strip outer quotes
            if text.len() >= 2 && text.starts_with('"') {
                return Some(&text[1..text.len() - 1]);
            }
            return Some(text);
        }
    }
    None
}

fn check_export_name(
    name: &str,
    node: &Node,
    seen: &mut HashMap<String, Range>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let range = node_to_range(node);
    if let Some(_first) = seen.get(name) {
        diagnostics.push(Diagnostic::error(
            range,
            format!("Duplicate export name \"{}\"", name),
        ));
    } else {
        seen.insert(name.to_string(), range);
    }
}

// ============================================================================
// 5. Import ordering
// ============================================================================

fn check_import_ordering(module: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let _ = source;
    let mut seen_non_import = false;

    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();

        match fk {
            "module_field_import" => {
                if seen_non_import {
                    diagnostics.push(Diagnostic::error(
                        node_to_range(field),
                        "Imports must occur before all non-import definitions",
                    ));
                }
            }
            // Inline imports on func/global/table/memory/tag
            "module_field_func"
            | "module_field_global"
            | "module_field_table"
            | "module_field_memory"
            | "module_field_tag" => {
                if has_inline_import(field) {
                    if seen_non_import {
                        diagnostics.push(Diagnostic::error(
                            node_to_range(field),
                            "Imports must occur before all non-import definitions",
                        ));
                    }
                } else {
                    seen_non_import = true;
                }
            }
            // Types, exports, start, data, elem, rec don't count as non-import definitions
            "module_field_type"
            | "module_field_export"
            | "module_field_start"
            | "module_field_data"
            | "module_field_elem"
            | "module_field_rec" => {}
            _ => {}
        }
    });
}

fn has_inline_import(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "import" {
            return true;
        }
        // Check inside memory_fields_type and table_fields_type
        if ck == "memory_fields_type" || ck == "table_fields_type" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                let gck = gc.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let gck = gck.as_str();
                if gck == "import" {
                    return true;
                }
            }
        }
    }
    false
}

// ============================================================================
// 6. Duplicate identifiers
// ============================================================================

fn check_duplicate_identifiers(module: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen_func: HashMap<String, Range> = HashMap::new();
    let mut seen_global: HashMap<String, Range> = HashMap::new();
    let mut seen_table: HashMap<String, Range> = HashMap::new();
    let mut seen_memory: HashMap<String, Range> = HashMap::new();
    let mut seen_type: HashMap<String, Range> = HashMap::new();
    let mut seen_tag: HashMap<String, Range> = HashMap::new();

    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();

        match fk {
            "module_field_func" => {
                check_identifier_dup(field, source, &mut seen_func, "func", diagnostics);
            }
            "module_field_global" => {
                check_identifier_dup(field, source, &mut seen_global, "global", diagnostics);
            }
            "module_field_table" => {
                check_identifier_dup(field, source, &mut seen_table, "table", diagnostics);
            }
            "module_field_memory" => {
                check_identifier_dup(field, source, &mut seen_memory, "memory", diagnostics);
            }
            "module_field_type" => {
                check_identifier_dup(field, source, &mut seen_type, "type", diagnostics);
            }
            "module_field_tag" => {
                check_identifier_dup(field, source, &mut seen_tag, "tag", diagnostics);
            }
            "module_field_rec" => {
                // Types inside rec groups share the type index space
                let mut cursor = field.walk();
                for child in field.children(&mut cursor) {
                    // Inside rec, look for type definitions. The grammar defines rec as:
                    // (rec (type $id? type_field) ...)
                    // But the inner nodes are anonymous sequences, so we look for identifier
                    // children or children that look like type definitions.
                    let ck = child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let ck = ck.as_str();
                    if ck == "module_field_type" {
                        check_identifier_dup(&child, source, &mut seen_type, "type", diagnostics);
                    }
                }
            }
            "module_field_import" => {
                // Import descriptors declare identifiers in their respective namespaces
                check_import_identifier_dup(
                    field,
                    source,
                    &mut seen_func,
                    &mut seen_global,
                    &mut seen_table,
                    &mut seen_memory,
                    &mut seen_tag,
                    diagnostics,
                );
            }
            _ => {}
        }
    });
}

fn check_identifier_dup(
    node: &Node,
    source: &str,
    seen: &mut HashMap<String, Range>,
    kind_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(id_node) = find_identifier_child(node) {
        let name = node_text(&id_node, source);
        if name.starts_with('$') {
            let range = node_to_range(&id_node);
            if let Some(_first) = seen.get(name) {
                diagnostics.push(Diagnostic::error(
                    range,
                    format!("Duplicate {} identifier {}", kind_name, name),
                ));
            } else {
                seen.insert(name.to_string(), range);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_import_identifier_dup(
    import_node: &Node,
    source: &str,
    seen_func: &mut HashMap<String, Range>,
    seen_global: &mut HashMap<String, Range>,
    seen_table: &mut HashMap<String, Range>,
    seen_memory: &mut HashMap<String, Range>,
    seen_tag: &mut HashMap<String, Range>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = import_node.walk();
    for child in import_node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "import_desc" {
            let mut dc = child.walk();
            for desc in child.children(&mut dc) {
                let dk = desc.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let dk = dk.as_str();
                match dk {
                    "import_desc_func_type" | "import_desc_type_use" => {
                        check_identifier_dup(&desc, source, seen_func, "func", diagnostics);
                    }
                    "import_desc_global_type" => {
                        check_identifier_dup(&desc, source, seen_global, "global", diagnostics);
                    }
                    "import_desc_table_type" => {
                        check_identifier_dup(&desc, source, seen_table, "table", diagnostics);
                    }
                    "import_desc_memory_type" => {
                        check_identifier_dup(&desc, source, seen_memory, "memory", diagnostics);
                    }
                    "import_desc_tag_type" => {
                        check_identifier_dup(&desc, source, seen_tag, "tag", diagnostics);
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(feature = "native")]
fn find_identifier_child<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    #[allow(clippy::manual_find)]
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(child);
        }
    }
    None
}

#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_identifier_child(node: &Node) -> Option<Node> {
    let mut cursor = node.walk();
    #[allow(clippy::manual_find)]
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(child);
        }
    }
    None
}

// ============================================================================
// 7. Inline function type mismatches
// ============================================================================

fn check_inline_type_mismatches(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();

        if fk != "module_field_func" {
            return;
        }

        // Check if this function has both type_use and inline params/results
        let mut type_use_node = None;
        let mut has_inline_sig = false;
        let mut inline_params: Vec<String> = Vec::new();
        let mut inline_results: Vec<String> = Vec::new();

        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            let ck = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let ck = ck.as_str();
            match ck {
                "type_use" => {
                    #[cfg(feature = "native")]
                    {
                        type_use_node = Some(child);
                    }
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    {
                        type_use_node = Some(child.clone());
                    }
                }
                "func_type_params" => {
                    has_inline_sig = true;
                    extract_param_types(&child, source, &mut inline_params);
                }
                "func_type_results" => {
                    has_inline_sig = true;
                    extract_result_types(&child, source, &mut inline_results);
                }
                _ => {}
            }
        }

        // Only check if both type_use and inline signature are present
        let type_use = match type_use_node {
            Some(n) if has_inline_sig => n,
            _ => return,
        };

        // Resolve the type from the type_use
        if let Some(type_def) = resolve_type_use(&type_use, source, symbols) {
            if let TypeKind::Func { params, results } = &type_def.kind {
                let type_params: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                let type_results: Vec<String> = results.iter().map(|r| r.to_string()).collect();

                if inline_params != type_params || inline_results != type_results {
                    diagnostics.push(Diagnostic::error(
                        node_to_range(&type_use),
                        "Inline function type does not match the type reference",
                    ));
                }
            }
        }
    });
}

fn extract_param_types(node: &Node, source: &str, out: &mut Vec<String>) {
    // func_type_params wraps func_type_params_one or func_type_params_many
    // which contain value_type children
    collect_value_types_recursive(node, source, out);
}

fn extract_result_types(node: &Node, source: &str, out: &mut Vec<String>) {
    // func_type_results: (result value_type*)
    collect_value_types_recursive(node, source, out);
}

fn collect_value_types_recursive(node: &Node, source: &str, out: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "value_type" || ck == "ref_type" {
            out.push(node_text(&child, source).to_string());
        } else {
            collect_value_types_recursive(&child, source, out);
        }
    }
}

fn resolve_type_use<'a>(
    type_use: &Node,
    source: &str,
    symbols: &'a SymbolTable,
) -> Option<&'a crate::symbols::TypeDef> {
    // type_use: (type $idx) or (type 0)
    let mut cursor = type_use.walk();
    for child in type_use.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "index" {
            let text = node_text(&child, source);
            // Try as name reference
            if text.starts_with('$') {
                return symbols.get_type_by_name(text);
            }
            // Try as numeric index
            if let Ok(idx) = text.parse::<usize>() {
                return symbols.get_type_by_index(idx);
            }
            // Index may contain an identifier child
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                let ik = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ik = ik.as_str();
                if ik == "identifier" {
                    let id_text = node_text(&idx_child, source);
                    return symbols.get_type_by_name(id_text);
                }
                if ik == "nat" {
                    let nat_text = node_text(&idx_child, source);
                    if let Ok(idx) = nat_text.parse::<usize>() {
                        return symbols.get_type_by_index(idx);
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// 8. Constant expression validation
// ============================================================================

const CONST_INSTRUCTIONS: &[&str] = &[
    "i32.const",
    "i64.const",
    "f32.const",
    "f64.const",
    "v128.const",
    "ref.null",
    "ref.func",
    "global.get",
    // GC proposal const instructions
    "struct.new",
    "struct.new_default",
    "array.new",
    "array.new_default",
    "array.new_fixed",
    "any.convert_extern",
    "extern.convert_any",
    "ref.i31",
    "i31.get_s",
    "i31.get_u",
    // Extended constant expressions proposal
    "i32.add",
    "i32.sub",
    "i32.mul",
    "i64.add",
    "i64.sub",
    "i64.mul",
];

fn check_constant_expressions(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();

        match fk {
            "module_field_global" => {
                // Check instructions after global_type
                check_global_init_expr(field, source, symbols, diagnostics);
            }
            "module_field_data" => {
                // Check offset expression
                check_offset_expr(field, source, symbols, diagnostics);
            }
            "module_field_elem" => {
                // Check offset expression and elem expressions
                check_offset_expr(field, source, symbols, diagnostics);
                check_elem_exprs(field, source, symbols, diagnostics);
            }
            "module_field_table" => {
                // Check table init expression (if any)
                check_table_init_expr(field, source, symbols, diagnostics);
            }
            _ => {}
        }
    });
}

fn check_global_init_expr(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Global: (global $id? type init_expr...)
    // init instructions appear after global_type as instr or expr nodes
    let mut past_global_type = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "global_type" {
            past_global_type = true;
            continue;
        }
        if past_global_type {
            check_const_instruction_tree(&child, source, symbols, diagnostics);
        }
    }
}

fn check_offset_expr(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "offset" {
            check_const_instruction_tree(&child, source, symbols, diagnostics);
        }
    }
}

fn check_elem_exprs(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "elem_expr" {
            check_const_instruction_tree(&child, source, symbols, diagnostics);
        }
    }
}

fn check_table_init_expr(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        // table_fields_type may contain an expr child
        if ck == "table_fields_type" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                let gk = gc.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let gk = gk.as_str();
                if gk == "expr" {
                    check_const_instruction_tree(&gc, source, symbols, diagnostics);
                }
            }
        }
    }
}

/// Recursively check that all instructions in a tree are constant expressions.
fn check_const_instruction_tree(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    match kind {
        "instr_plain" => {
            // Linear-form instruction: check the opcode directly
            let text = node_text(node, source);
            let first_token = text.split_whitespace().next().unwrap_or("");
            if !CONST_INSTRUCTIONS.contains(&first_token) {
                diagnostics.push(Diagnostic::error(
                    node_to_range(node),
                    format!("Constant expression required, but found '{}'", first_token),
                ));
            } else if first_token == "global.get" {
                check_global_get_in_const(node, source, symbols, diagnostics);
            }
            return;
        }
        "expr1_plain" => {
            // Folded expression: first child is instr_plain, check its opcode
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let ck = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ck = ck.as_str();
                if ck == "instr_plain" {
                    let text = node_text(&child, source);
                    let first_token = text.split_whitespace().next().unwrap_or("");
                    if !CONST_INSTRUCTIONS.contains(&first_token) {
                        diagnostics.push(Diagnostic::error(
                            node_to_range(node),
                            format!("Constant expression required, but found '{}'", first_token),
                        ));
                    } else if first_token == "global.get" {
                        check_global_get_in_const(&child, source, symbols, diagnostics);
                    }
                } else if ck == "expr" {
                    // Check nested expressions for constness too
                    check_const_instruction_tree(&child, source, symbols, diagnostics);
                }
            }
            return;
        }
        // block/if/loop instructions are never valid in const expressions
        "instr_block" | "block_block" | "block_loop" | "block_if" => {
            diagnostics.push(Diagnostic::error(
                node_to_range(node),
                "Constant expression required, but found block instruction",
            ));
            return;
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_const_instruction_tree(&child, source, symbols, diagnostics);
    }
}

/// Check that global.get in a constant expression references an immutable imported global.
fn check_global_get_in_const(
    instr_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the index/identifier child
    let mut cursor = instr_node.walk();
    for child in instr_node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "index" || ck == "identifier" {
            let text = node_text(&child, source);
            let global = if text.starts_with('$') {
                symbols.get_global_by_name(text)
            } else if let Ok(idx) = text.parse::<usize>() {
                symbols.get_global_by_index(idx)
            } else {
                // Check for identifier/nat child inside index node
                let mut ic = child.walk();
                let mut found = None;
                for idx_child in child.children(&mut ic) {
                    let ik = idx_child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let ik = ik.as_str();
                    if ik == "identifier" {
                        found = symbols.get_global_by_name(node_text(&idx_child, source));
                    } else if ik == "nat" {
                        if let Ok(idx) = node_text(&idx_child, source).parse::<usize>() {
                            found = symbols.get_global_by_index(idx);
                        }
                    }
                }
                found
            };

            if let Some(global) = global {
                if global.is_mutable {
                    diagnostics.push(Diagnostic::error(
                        node_to_range(instr_node),
                        "constant expression required: global.get of mutable global",
                    ));
                } else if global.index >= symbols.num_imported_globals {
                    diagnostics.push(Diagnostic::error(
                        node_to_range(instr_node),
                        "constant expression required: global.get of non-imported global",
                    ));
                }
            }
            return;
        }
    }
}

// ============================================================================
// Data segment memory index validation
// ============================================================================

fn check_data_segment_memory_indices(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk != "module_field_data" {
            return;
        }

        // Look for memory_use child (e.g., (memory 1)) or an index before offset
        // Active data segments have an offset expression
        let mut has_offset = false;
        let mut memory_idx: Option<usize> = None;
        let mut memory_node_range = None;

        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            let ck = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let ck = ck.as_str();
            if ck == "offset" {
                has_offset = true;
            }
            if ck == "memory_use" {
                // (memory $idx) or (memory N)
                let mut mc = child.walk();
                for mc_child in child.children(&mut mc) {
                    let mk = mc_child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let mk = mk.as_str();
                    if mk == "index" || mk == "identifier" || mk == "nat" {
                        let text = node_text(&mc_child, source);
                        memory_node_range = Some(node_to_range(&child));
                        if text.starts_with('$') {
                            if let Some(mem) = symbols.get_memory_by_name(text) {
                                memory_idx = Some(mem.index);
                            } else {
                                // Unknown memory — reference check will handle this
                                return;
                            }
                        } else if let Ok(idx) = text.parse::<usize>() {
                            memory_idx = Some(idx);
                        }
                    }
                }
            }
        }

        if !has_offset {
            return; // passive segment — no memory index needed
        }

        // Default memory index is 0
        let idx = memory_idx.unwrap_or(0);
        if idx >= symbols.memories.len() {
            let range = memory_node_range.unwrap_or_else(|| node_to_range(field));
            diagnostics.push(Diagnostic::error(range, format!("unknown memory {}", idx)));
        }
    });
}

// ============================================================================
// Element segment table index validation
// ============================================================================

fn check_elem_segment_table_indices(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk != "module_field_elem" {
            return;
        }

        // Look for table_use child or active segment with offset
        let mut has_offset = false;
        let mut table_idx: Option<usize> = None;
        let mut table_node_range = None;

        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            let ck = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let ck = ck.as_str();
            if ck == "offset" {
                has_offset = true;
            }
            if ck == "table_use" {
                let mut tc = child.walk();
                for tc_child in child.children(&mut tc) {
                    let tk = tc_child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let tk = tk.as_str();
                    if tk == "index" || tk == "identifier" || tk == "nat" {
                        let text = node_text(&tc_child, source);
                        table_node_range = Some(node_to_range(&child));
                        if text.starts_with('$') {
                            if let Some(tbl) = symbols.get_table_by_name(text) {
                                table_idx = Some(tbl.index);
                            } else {
                                return; // unknown table — reference check handles
                            }
                        } else if let Ok(idx) = text.parse::<usize>() {
                            table_idx = Some(idx);
                        }
                    }
                }
            }
        }

        if !has_offset {
            return; // passive or declarative segment
        }

        let idx = table_idx.unwrap_or(0);
        if idx >= symbols.tables.len() {
            let range = table_node_range.unwrap_or_else(|| node_to_range(field));
            diagnostics.push(Diagnostic::error(range, format!("unknown table {}", idx)));
        }
    });
}

// ============================================================================
// ref.func declaration check
// ============================================================================

/// Check that all functions referenced by `ref.func` are declared in an element segment.
///
/// Per the WebAssembly spec, any function used with `ref.func` must appear in:
/// - A declarative element segment: `(elem declare func $f ...)`
/// - An active or passive element segment that lists the function
///
/// Functions used with `ref.func` but not declared in any element segment cause
/// an "undeclared function reference" validation error.
fn check_ref_func_declarations(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Step 1: Collect all functions declared in element segments
    let mut declared_funcs: HashSet<usize> = HashSet::new();
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk != "module_field_elem" {
            return;
        }
        collect_elem_declared_funcs(field, source, symbols, &mut declared_funcs);
    });

    // Also collect functions referenced in inline elem expressions on tables
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk != "module_field_table" {
            return;
        }
        collect_inline_table_elem_funcs(field, source, symbols, &mut declared_funcs);
    });

    // Step 2: Walk all function bodies to find ref.func usage
    collect_ref_func_errors(module, source, symbols, &declared_funcs, diagnostics);
}

/// Collect function indices declared in an element segment.
fn collect_elem_declared_funcs(
    elem_field: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &mut HashSet<usize>,
) {
    collect_func_refs_recursive(elem_field, source, symbols, declared);
}

/// Recursively collect function references from element segment nodes.
/// Walks the entire subtree, picking up any identifier/index/nat that resolves
/// to a function index.
fn collect_func_refs_recursive(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &mut HashSet<usize>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();

        if ck == "identifier" || ck == "index" {
            let text = node_text(&child, source).trim();
            if let Some(idx) = resolve_func_index(text, symbols) {
                declared.insert(idx);
            }
        } else if ck == "nat" {
            let text = node_text(&child, source).trim();
            if let Ok(idx) = text.parse::<usize>() {
                if idx < symbols.functions.len() {
                    declared.insert(idx);
                }
            }
        } else {
            // Recurse into all other nodes to find nested identifiers
            collect_func_refs_recursive(&child, source, symbols, declared);
        }
    }
}

/// Collect functions from inline table elem expressions.
fn collect_inline_table_elem_funcs(
    table_field: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &mut HashSet<usize>,
) {
    // Tables can have inline elem: (table funcref (elem $f1 $f2))
    let text = node_text(table_field, source);
    if !text.contains("elem") {
        return;
    }
    collect_func_refs_recursive(table_field, source, symbols, declared);
}

/// Resolve a function reference (name or numeric index) to an index.
fn resolve_func_index(text: &str, symbols: &SymbolTable) -> Option<usize> {
    if text.starts_with('$') {
        symbols.get_function_by_name(text).map(|f| f.index)
    } else {
        text.parse::<usize>()
            .ok()
            .filter(|&idx| idx < symbols.functions.len())
    }
}

/// Walk all function bodies to find ref.func instructions and check they're declared.
fn collect_ref_func_errors(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &HashSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Walk the entire module AST looking for ref.func instructions
    walk_for_ref_func(module, source, symbols, declared, diagnostics);
}

/// Recursively walk a node tree looking for ref.func instructions.
fn walk_for_ref_func(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &HashSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    // Check if this is a ref.func instruction
    if kind == "instr_plain" || kind == "op_index" || kind == "op_nullary" {
        let text = node_text(node, source).trim();
        if text.starts_with("ref.func") {
            // Find the function reference argument
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let ck = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ck = ck.as_str();

                // Look inside op_ nodes for the index
                if ck.starts_with("op_") {
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        let ik = inner.kind();
                        #[cfg(all(feature = "wasm", not(feature = "native")))]
                        let ik = ik.as_str();
                        if ik == "identifier" || ik == "index" || ik == "nat" {
                            check_ref_func_target(
                                &inner,
                                node_text(&inner, source).trim(),
                                symbols,
                                declared,
                                diagnostics,
                            );
                        }
                    }
                }
                if ck == "identifier" || ck == "index" || ck == "nat" {
                    check_ref_func_target(
                        &child,
                        node_text(&child, source).trim(),
                        symbols,
                        declared,
                        diagnostics,
                    );
                }
            }
        }
    }

    // Skip element segments (don't flag ref.func inside elem expressions)
    if kind == "module_field_elem" {
        return;
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_ref_func(&child, source, symbols, declared, diagnostics);
    }
}

/// Check a single ref.func target against the declared set.
fn check_ref_func_target(
    node: &Node,
    func_ref: &str,
    symbols: &SymbolTable,
    declared: &HashSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(idx) = resolve_func_index(func_ref, symbols) {
        if !declared.contains(&idx) {
            let range = node_to_range(node);
            diagnostics.push(
                Diagnostic::error(range, format!("undeclared function reference {}", func_ref))
                    .with_code("undeclared-func-ref"),
            );
        }
    }
}

// ============================================================================
// Block label mismatch
// ============================================================================

/// Check that end labels match opening labels on block/loop/if statements.
pub fn check_block_label_mismatch(node: &Node, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    if !matches!(kind, "block_block" | "block_loop" | "block_if") {
        return diagnostics;
    }

    let child_count = node.child_count();
    if child_count == 0 {
        return diagnostics;
    }

    // Find opening label: first identifier child (right after the keyword)
    // Find end label: identifier child that comes after "end" anonymous node
    // For if: also check identifier after "else" anonymous node
    let mut opening_label: Option<&str> = None;

    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    // First pass: find the opening label
    // Opening label is the first identifier that appears right after the block keyword
    for (i, child) in children.iter().enumerate() {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "identifier" {
            // Make sure this is not after "end" or "else"
            if i > 0 {
                let prev = &children[i - 1];
                let prev_text = node_text(prev, source);
                if prev_text == "end" || prev_text == "else" {
                    continue;
                }
            }
            opening_label = Some(node_text(child, source));
            break;
        }
    }

    // Second pass: find labels after "else" and "end"
    let mut after_end = false;
    let mut after_else = false;

    for child in &children {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();

        // Check for anonymous "end" and "else" tokens
        if child.is_named() {
            if ck == "identifier" {
                if after_end {
                    let end_label = node_text(child, source);
                    check_label_match(opening_label, end_label, child, "end", &mut diagnostics);
                    after_end = false;
                } else if after_else {
                    let else_label = node_text(child, source);
                    check_label_match(opening_label, else_label, child, "else", &mut diagnostics);
                    after_else = false;
                }
            } else {
                after_end = false;
                after_else = false;
            }
        } else {
            let text = node_text(child, source);
            if text == "end" {
                after_end = true;
                after_else = false;
            } else if text == "else" {
                after_else = true;
                after_end = false;
            } else {
                after_end = false;
                after_else = false;
            }
        }
    }

    diagnostics
}

fn check_label_match(
    opening_label: Option<&str>,
    end_label: &str,
    end_node: &Node,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match opening_label {
        Some(open) if open == end_label => {
            // Labels match — ok
        }
        Some(open) => {
            diagnostics.push(Diagnostic::error(
                node_to_range(end_node),
                format!(
                    "Mismatching label: {} label {} does not match opening label {}",
                    context, end_label, open
                ),
            ));
        }
        None => {
            diagnostics.push(Diagnostic::error(
                node_to_range(end_node),
                format!(
                    "Mismatching label: {} label {} specified but no opening label",
                    context, end_label
                ),
            ));
        }
    }
}

// ============================================================================
// Duplicate local/parameter names within functions
// ============================================================================

fn check_duplicate_locals(symbols: &SymbolTable, diagnostics: &mut Vec<Diagnostic>) {
    for func in &symbols.functions {
        let mut seen: HashMap<&str, Range> = HashMap::new();

        // Check parameters first
        for param in &func.parameters {
            if let Some(ref name) = param.name {
                if let Some(ref range) = param.range {
                    if let Some(_first) = seen.get(name.as_str()) {
                        diagnostics.push(Diagnostic::error(
                            *range,
                            format!("duplicate local {}", name),
                        ));
                    } else {
                        seen.insert(name, *range);
                    }
                }
            }
        }

        // Check locals (also conflicts with params)
        for local in &func.locals {
            if let Some(ref name) = local.name {
                if let Some(ref range) = local.range {
                    if let Some(_first) = seen.get(name.as_str()) {
                        diagnostics.push(Diagnostic::error(
                            *range,
                            format!("duplicate local {}", name),
                        ));
                    } else {
                        seen.insert(name, *range);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::parser::parse_document;

    fn parse_and_get_tree(source: &str) -> (tree_sitter::Tree, SymbolTable) {
        let mut parser = crate::tree_sitter_bindings::create_parser();
        let tree = parser.parse(source, None).expect("Failed to parse");
        let symbols = parse_document(source).expect("Failed to extract symbols");
        (tree, symbols)
    }

    fn get_diagnostics(source: &str) -> Vec<Diagnostic> {
        let (tree, symbols) = parse_and_get_tree(source);
        validate_module_structure(&tree.root_node(), source, &symbols)
    }

    fn get_block_diagnostics(source: &str) -> Vec<Diagnostic> {
        let mut parser = crate::tree_sitter_bindings::create_parser();
        let tree = parser.parse(source, None).expect("Failed to parse");
        let mut diags = Vec::new();
        collect_block_label_diags(tree.root_node(), source, &mut diags);
        diags
    }

    fn collect_block_label_diags(
        node: tree_sitter::Node,
        source: &str,
        diags: &mut Vec<Diagnostic>,
    ) {
        let kind = node.kind();
        if matches!(kind, "block_block" | "block_loop" | "block_if") {
            diags.extend(check_block_label_mismatch(&node, source));
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_block_label_diags(child, source, diags);
        }
    }

    #[test]
    fn test_memory_limits_ok() {
        let source = "(module (memory 1 100))";
        assert!(get_diagnostics(source).is_empty());
    }

    #[test]
    fn test_memory_limits_exceeds() {
        let source = "(module (memory 65537))";
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("65536"));
    }

    #[test]
    fn test_memory_limits_max_exceeds() {
        let source = "(module (memory 0 65537))";
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("65536"));
    }

    #[test]
    fn test_min_gt_max_memory() {
        let source = "(module (memory 10 5))";
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0]
            .message
            .contains("minimum must not be greater than maximum"));
    }

    #[test]
    fn test_min_gt_max_table() {
        let source = "(module (table 10 5 funcref))";
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0]
            .message
            .contains("minimum must not be greater than maximum"));
    }

    #[test]
    fn test_multiple_starts() {
        let source = "(module (func $f) (start $f) (start $f))";
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Multiple start"));
    }

    #[test]
    fn test_duplicate_export_names() {
        let source = r#"(module
            (func $f)
            (export "a" (func $f))
            (export "a" (func $f))
        )"#;
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Duplicate export name"));
    }

    #[test]
    fn test_duplicate_export_inline() {
        let source = r#"(module
            (func (export "a"))
            (func (export "a"))
        )"#;
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Duplicate export name"));
    }

    #[test]
    fn test_import_ordering() {
        let source = r#"(module
            (func $f)
            (import "m" "f" (func))
        )"#;
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Imports must occur before"));
    }

    #[test]
    fn test_import_ordering_ok() {
        let source = r#"(module
            (import "m" "f" (func))
            (func $f)
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_import_ordering_types_dont_count() {
        let source = r#"(module
            (type (func))
            (import "m" "f" (func))
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_duplicate_func_identifier() {
        let source = "(module (func $f) (func $f))";
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Duplicate func identifier"));
    }

    #[test]
    fn test_duplicate_type_identifier() {
        let source = "(module (type $t (func)) (type $t (func)))";
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Duplicate type identifier"));
    }

    #[test]
    fn test_block_label_mismatch() {
        let source = "(module (func block $l nop end $m))";
        let diags = get_block_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Mismatching label"));
    }

    #[test]
    fn test_block_label_match_ok() {
        let source = "(module (func block $l nop end $l))";
        let diags = get_block_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_block_label_end_without_opening() {
        let source = "(module (func block nop end $l))";
        let diags = get_block_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("no opening label"));
    }

    #[test]
    fn test_if_else_label_mismatch() {
        let source = "(module (func if $l nop else $m nop end $l))";
        let diags = get_block_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("else label"));
    }

    #[test]
    fn test_constant_expression_valid() {
        let source = "(module (global i32 i32.const 42))";
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_constant_expression_invalid() {
        let source = "(module (global i32 nop))";
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("Constant expression required"));
    }

    #[test]
    fn test_inline_type_mismatch() {
        let source = r#"(module
            (type $t (func (param i32) (result i32)))
            (func (type $t) (param i64) (result i32))
        )"#;
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0]
            .message
            .contains("Inline function type does not match"));
    }

    #[test]
    fn test_inline_type_match_ok() {
        let source = r#"(module
            (type $t (func (param i32) (result i32)))
            (func (type $t) (param i32) (result i32))
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    // ======================================================================
    // Duplicate local/parameter name tests
    // ======================================================================

    #[test]
    fn test_duplicate_param_names() {
        let source = r#"(module
            (func $f (param $x i32) (param $x i64))
        )"#;
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("duplicate local $x"));
    }

    #[test]
    fn test_duplicate_local_names() {
        let source = r#"(module
            (func $f
                (local $y i32)
                (local $y f64)
            )
        )"#;
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("duplicate local $y"));
    }

    #[test]
    fn test_local_shadows_param() {
        let source = r#"(module
            (func $f (param $x i32)
                (local $x i64)
            )
        )"#;
        let diags = get_diagnostics(source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("duplicate local $x"));
    }

    #[test]
    fn test_same_name_different_functions_ok() {
        let source = r#"(module
            (func $f (param $x i32))
            (func $g (param $x i32))
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_unnamed_params_no_conflict() {
        let source = r#"(module
            (func (param i32) (param i32))
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    // ======================================================================
    // Start function signature tests (Issue #191)
    // ======================================================================

    #[test]
    fn test_start_function_ok() {
        let source = "(module (func $f) (start $f))";
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_start_function_with_params() {
        let source = "(module (func $f (param i32)) (start $f))";
        let diags = get_diagnostics(source);
        assert!(
            !diags.is_empty(),
            "Expected diagnostic for start function with params"
        );
        assert!(diags.iter().any(|d| d.message.contains("start function")));
    }

    #[test]
    fn test_start_function_with_results() {
        let source = "(module (func $f (result i32) (i32.const 0)) (start $f))";
        let diags = get_diagnostics(source);
        assert!(
            !diags.is_empty(),
            "Expected diagnostic for start function with results"
        );
        assert!(diags.iter().any(|d| d.message.contains("start function")));
    }

    #[test]
    fn test_start_function_numeric_index_ok() {
        let source = "(module (func) (start 0))";
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_start_function_numeric_index_bad() {
        let source = "(module (func (param i32)) (start 0))";
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("start function")));
    }

    // ======================================================================
    // global.get in constant expressions (Issue #191)
    // ======================================================================

    #[test]
    fn test_global_get_const_imported_immutable_ok() {
        let source = r#"(module
            (import "env" "g" (global i32))
            (global i32 (global.get 0))
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_global_get_const_mutable_error() {
        let source = r#"(module
            (import "env" "g" (global (mut i32)))
            (global i32 (global.get 0))
        )"#;
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("mutable")));
    }

    #[test]
    fn test_global_get_const_non_imported_error() {
        let source = r#"(module
            (global $g i32 (i32.const 0))
            (global $h i32 (global.get $g))
        )"#;
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("non-imported")));
    }

    // ======================================================================
    // Data segment memory index validation (Issue #191)
    // ======================================================================

    #[test]
    fn test_data_segment_memory_ok() {
        let source = r#"(module
            (memory 1)
            (data (offset (i32.const 0)) "hello")
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_data_segment_no_memory() {
        let source = r#"(module
            (data (offset (i32.const 0)) "hello")
        )"#;
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("unknown memory")));
    }

    // ======================================================================
    // Element segment table index validation (Issue #191)
    // ======================================================================

    #[test]
    fn test_elem_segment_table_ok() {
        let source = r#"(module
            (table 1 funcref)
            (func $f)
            (elem (offset (i32.const 0)) $f)
        )"#;
        let diags = get_diagnostics(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_elem_segment_no_table() {
        let source = r#"(module
            (func $f)
            (elem (offset (i32.const 0)) $f)
        )"#;
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("unknown table")));
    }
}
