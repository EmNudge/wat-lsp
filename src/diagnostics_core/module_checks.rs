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
use crate::symbols::{SymbolTable, TypeKind, ValueType};
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
    check_table_limits(symbols, &mut diagnostics);
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
        check_constant_expression_types(&module_node, source, symbols, &mut diagnostics);
        check_global_forward_refs(&module_node, source, symbols, &mut diagnostics);
        check_data_segment_memory_indices(&module_node, source, symbols, &mut diagnostics);
        check_elem_segment_table_indices(&module_node, source, symbols, &mut diagnostics);
        check_ref_func_declarations(&module_node, source, symbols, &mut diagnostics);
        check_unknown_type_refs(&module_node, source, symbols, &mut diagnostics);
        check_block_type_use_mismatches(&module_node, source, symbols, &mut diagnostics);
        check_table_non_nullable_refs(&module_node, source, &mut diagnostics);
        check_implicit_memory_refs(&module_node, source, symbols, &mut diagnostics);
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
                }
                // Note: extended const expressions (now part of the spec) allow
                // global.get of any immutable global, not just imported ones.
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

    // Also collect functions referenced via ref.func in global/table init expressions
    // Per spec, ref.func in const init contexts counts as a function declaration
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk == "module_field_global" || fk == "module_field_table" {
            collect_ref_func_in_const_expr(field, source, symbols, &mut declared_funcs);
        }
    });

    // Step 2: Walk all function bodies to find ref.func usage
    collect_ref_func_errors(module, source, symbols, &declared_funcs, diagnostics);
}

/// Collect function indices referenced by ref.func in a global init expression.
fn collect_ref_func_in_const_expr(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &mut HashSet<usize>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    if kind == "instr_plain" {
        let text = node_text(node, source);
        if text.starts_with("ref.func") {
            // Find the function reference
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let ck = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ck = ck.as_str();
                if ck == "index" || ck == "identifier" || ck == "nat" {
                    let ref_text = node_text(&child, source).trim();
                    if let Some(idx) = resolve_func_index(ref_text, symbols) {
                        declared.insert(idx);
                    }
                }
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_ref_func_in_const_expr(&child, source, symbols, declared);
    }
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
// Unknown type references (heap type indices)
// ============================================================================

/// Validate that numeric type indices in ref types are within bounds.
/// Catches: `(ref N)`, `(ref null N)` where N >= number of types defined.
fn check_unknown_type_refs(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Account for implicit types from function/tag declarations
    let max_type_count = symbols.types.len() + symbols.functions.len();
    walk_for_unknown_type_refs(module, source, max_type_count, diagnostics);
}

/// Recursively walk the AST looking for numeric type indices in ref types and type_use.
fn walk_for_unknown_type_refs(
    node: &Node,
    source: &str,
    max_type_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    // Check ref_type nodes for numeric heap type indices
    if kind == "ref_type" || kind == "_heap_type_or_ref" {
        check_heap_type_index(node, source, max_type_count, diagnostics);
    }

    // Also check value_type_ref_type which can contain heap types
    if kind == "value_type_ref_type" {
        check_heap_type_index(node, source, max_type_count, diagnostics);
    }

    // Check type_use nodes (e.g., call_indirect (type N))
    // Use max_type_count to account for implicit types from functions
    if kind == "type_use" {
        check_type_use_index(node, source, max_type_count, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_unknown_type_refs(&child, source, max_type_count, diagnostics);
    }
}

/// Check if a ref_type node contains an out-of-bounds numeric type index.
/// Recursively searches for nat nodes within the ref_type's children.
fn check_heap_type_index(
    ref_type_node: &Node,
    source: &str,
    type_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    find_type_index_in_ref(
        ref_type_node,
        ref_type_node,
        source,
        type_count,
        diagnostics,
    );
}

/// Recursively search for numeric type indices within a ref_type subtree.
fn find_type_index_in_ref(
    root: &Node,
    node: &Node,
    source: &str,
    type_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();

        // A nat/dec_nat/hex_nat inside a ref type is a numeric type index
        if ck == "nat" || ck == "dec_nat" || ck == "hex_nat" {
            let text = node_text(&child, source);
            if let Ok(idx) = parse_nat(text) {
                if idx >= type_count {
                    diagnostics.push(
                        Diagnostic::error(node_to_range(root), format!("unknown type {}", idx))
                            .with_code("unknown-type"),
                    );
                }
            }
            return;
        }

        // Skip identifier nodes (named type references are valid)
        if ck == "identifier" {
            return;
        }

        // Recurse into intermediate nodes (ref_type_ref, index, heap_type, etc.)
        find_type_index_in_ref(root, &child, source, type_count, diagnostics);
    }
}

/// Check if a type_use node contains an out-of-bounds numeric type index.
fn check_type_use_index(
    type_use_node: &Node,
    source: &str,
    type_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = type_use_node.walk();
    for child in type_use_node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();

        if ck == "index" {
            // Check the index node's children for a numeric value
            let mut idx_cursor = child.walk();
            for idx_child in child.children(&mut idx_cursor) {
                let ick = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ick = ick.as_str();

                if ick == "nat" || ick == "dec_nat" || ick == "hex_nat" {
                    let text = node_text(&idx_child, source);
                    if let Ok(idx) = parse_nat(text) {
                        if idx >= type_count {
                            diagnostics.push(
                                Diagnostic::error(
                                    node_to_range(type_use_node),
                                    format!("unknown type {}", idx),
                                )
                                .with_code("unknown-type"),
                            );
                        }
                    }
                    return;
                }
                // Named type references: skip (handled by reference checker)
                if ick == "identifier" {
                    return;
                }
            }
            // The index node might directly contain the numeric text
            let text = node_text(&child, source);
            if let Ok(idx) = parse_nat(text) {
                if idx >= type_count {
                    diagnostics.push(
                        Diagnostic::error(
                            node_to_range(type_use_node),
                            format!("unknown type {}", idx),
                        )
                        .with_code("unknown-type"),
                    );
                }
            }
        }
    }
}

/// Parse a nat (natural number) from text, handling hex prefix.
fn parse_nat(text: &str) -> Result<usize, ()> {
    let text = text.trim().replace('_', "");
    if text.starts_with("0x") || text.starts_with("0X") {
        usize::from_str_radix(&text[2..], 16).map_err(|_| ())
    } else {
        text.parse::<usize>().map_err(|_| ())
    }
}

// ============================================================================
// Step 8: Table size limits
// ============================================================================

fn check_table_limits(symbols: &SymbolTable, diagnostics: &mut Vec<Diagnostic>) {
    const MAX_TABLE_SIZE: u64 = 0xFFFF_FFFF; // 2^32 - 1

    for table in &symbols.tables {
        let range = table
            .range
            .unwrap_or_else(|| Range::from_coords(table.line, 0, table.line, 0));

        if table.limits.0 as u64 > MAX_TABLE_SIZE {
            diagnostics.push(Diagnostic::error(
                range,
                "Size minimum must not be greater than 4294967295",
            ));
        }

        if let Some(max) = table.limits.1 {
            if max as u64 > MAX_TABLE_SIZE {
                diagnostics.push(Diagnostic::error(
                    range,
                    "Size minimum must not be greater than 4294967295",
                ));
            }
        }
    }
}

// ============================================================================
// Step 1: Constant expression type validation
// ============================================================================

/// Infer the type produced by a single const instruction name.
fn infer_const_instr_type(
    instr_name: &str,
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Option<ValueType> {
    match instr_name {
        "i32.const" | "i32.add" | "i32.sub" | "i32.mul" => Some(ValueType::I32),
        "i64.const" | "i64.add" | "i64.sub" | "i64.mul" => Some(ValueType::I64),
        "f32.const" => Some(ValueType::F32),
        "f64.const" => Some(ValueType::F64),
        "v128.const" => Some(ValueType::V128),
        "ref.null" => {
            // Resolve the heap type from the argument
            let heap_text = resolve_ref_null_heap_type(node, source);
            match heap_text.as_deref() {
                Some("func") | Some("funcref") => Some(ValueType::Funcref),
                Some("extern") | Some("externref") => Some(ValueType::Externref),
                Some("any") | Some("anyref") => Some(ValueType::Anyref),
                Some("eq") | Some("eqref") => Some(ValueType::Eqref),
                Some("i31") | Some("i31ref") => Some(ValueType::I31ref),
                Some("struct") | Some("structref") => Some(ValueType::Structref),
                Some("array") | Some("arrayref") => Some(ValueType::Arrayref),
                Some("none") | Some("nullref") => Some(ValueType::Nullref),
                Some("noextern") | Some("nullexternref") => Some(ValueType::Externref),
                Some("nofunc") | Some("nullfuncref") => Some(ValueType::Funcref),
                Some(s) if s.starts_with('$') => {
                    // Named type — resolve to Ref(n)
                    if let Some(t) = symbols.get_type_by_name(s) {
                        Some(ValueType::RefNull(t.index as u32))
                    } else {
                        Some(ValueType::Unknown)
                    }
                }
                Some(s) => {
                    // Numeric type index
                    if let Ok(idx) = s.parse::<u32>() {
                        Some(ValueType::RefNull(idx))
                    } else {
                        Some(ValueType::Unknown)
                    }
                }
                None => Some(ValueType::Unknown),
            }
        }
        "ref.func" => Some(ValueType::Funcref),
        "ref.i31" => Some(ValueType::I31ref),
        "i31.get_s" | "i31.get_u" => Some(ValueType::I32),
        "any.convert_extern" => Some(ValueType::Anyref),
        "extern.convert_any" => Some(ValueType::Externref),
        "struct.new" | "struct.new_default" | "array.new" | "array.new_default"
        | "array.new_fixed" => {
            // Try to resolve type index to Ref(n)
            if let Some(type_idx) = get_const_instr_type_index(node, source, symbols) {
                Some(ValueType::Ref(type_idx as u32))
            } else {
                Some(ValueType::Unknown)
            }
        }
        "global.get" => {
            // Resolve global type
            if let Some(global_type) = resolve_global_get_type(node, source, symbols) {
                Some(global_type)
            } else {
                Some(ValueType::Unknown)
            }
        }
        _ => None, // Not a const instruction
    }
}

/// Get the type index from a const instruction (e.g., struct.new $type_idx)
fn get_const_instr_type_index(node: &Node, source: &str, symbols: &SymbolTable) -> Option<usize> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "index" || ck == "identifier" {
            let text = node_text(&child, source);
            if text.starts_with('$') {
                return symbols.get_type_by_name(text).map(|t| t.index);
            }
            if let Ok(idx) = text.parse::<usize>() {
                return Some(idx);
            }
            // Check children
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                let ik = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ik = ik.as_str();
                if ik == "identifier" {
                    return symbols
                        .get_type_by_name(node_text(&idx_child, source))
                        .map(|t| t.index);
                }
                if ik == "nat" {
                    if let Ok(idx) = node_text(&idx_child, source).parse::<usize>() {
                        return Some(idx);
                    }
                }
            }
        }
    }
    None
}

/// Resolve the type of a global.get instruction's target global.
/// Resolve the heap type argument of a ref.null instruction.
/// Looks for the heap type text in child nodes of the instruction.
fn resolve_ref_null_heap_type(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        // The heap type can appear as various node kinds
        if matches!(
            ck,
            "heap_type" | "_heap_type_or_ref" | "identifier" | "nat" | "index"
        ) {
            let text = node_text(&child, source).trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
        // Also check inside index nodes
        if ck == "index" {
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                let ik = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ik = ik.as_str();
                if matches!(ik, "identifier" | "nat") {
                    let text = node_text(&idx_child, source).trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
    }
    // Fallback: try to extract from text
    let text = node_text(node, source);
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() >= 2 {
        return Some(parts[1].to_string());
    }
    None
}

fn resolve_global_get_type(node: &Node, source: &str, symbols: &SymbolTable) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "index" || ck == "identifier" {
            let text = node_text(&child, source);
            if text.starts_with('$') {
                return symbols.get_global_by_name(text).map(|g| g.var_type.clone());
            }
            if let Ok(idx) = text.parse::<usize>() {
                return symbols.get_global_by_index(idx).map(|g| g.var_type.clone());
            }
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                let ik = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ik = ik.as_str();
                if ik == "identifier" {
                    return symbols
                        .get_global_by_name(node_text(&idx_child, source))
                        .map(|g| g.var_type.clone());
                }
                if ik == "nat" {
                    if let Ok(idx) = node_text(&idx_child, source).parse::<usize>() {
                        return symbols.get_global_by_index(idx).map(|g| g.var_type.clone());
                    }
                }
            }
        }
    }
    None
}

/// Deep-count all leaf instructions in a const expression tree.
/// This recursively traverses ALL children to find instructions at any depth.
fn count_const_instrs_deep(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> (usize, Option<ValueType>) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    match kind {
        "instr_plain" => {
            let text = node_text(node, source);
            let first_token = text.split_whitespace().next().unwrap_or("");
            let ty = infer_const_instr_type(first_token, node, source, symbols);
            (1, ty)
        }
        "expr1_plain" => {
            // Folded expression: the instruction itself produces one value
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let ck = child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ck = ck.as_str();
                if ck == "instr_plain" {
                    let text = node_text(&child, source);
                    let first_token = text.split_whitespace().next().unwrap_or("");
                    let ty = infer_const_instr_type(first_token, &child, source, symbols);
                    return (1, ty);
                }
            }
            (1, None)
        }
        _ => {
            // Recurse into all children
            let mut total = 0;
            let mut last_type = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let (count, ty) = count_const_instrs_deep(&child, source, symbols);
                total += count;
                if ty.is_some() {
                    last_type = ty;
                }
            }
            (total, last_type)
        }
    }
}

/// Check that constant expression types match their expected types.
fn check_constant_expression_types(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut global_idx = 0usize;
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();

        match fk {
            "module_field_global" => {
                // Get expected type from global_type
                if !has_inline_import(field) {
                    if let Some(expected_type) = extract_global_value_type(field, source) {
                        check_const_expr_type_for_global(
                            field,
                            source,
                            symbols,
                            &expected_type,
                            diagnostics,
                        );
                    }
                }
                global_idx += 1;
            }
            "module_field_data" => {
                // Data offset must be i32 (memory32) or i64 (memory64)
                check_data_offset_type(field, source, symbols, diagnostics);
            }
            "module_field_elem" => {
                // Elem offset must be i32
                check_elem_offset_type(field, source, symbols, diagnostics);
            }
            "module_field_table" => {
                // Table init expression must match table's ref type
                check_table_init_type(field, source, symbols, diagnostics);
            }
            "module_field_import" => {
                // Count imported globals for forward ref checking
                let mut cursor = field.walk();
                for child in field.children(&mut cursor) {
                    let ck = child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let ck = ck.as_str();
                    if ck == "import_desc" {
                        let mut dc = child.walk();
                        for desc in child.children(&mut dc) {
                            let dk = desc.kind();
                            #[cfg(all(feature = "wasm", not(feature = "native")))]
                            let dk = dk.as_str();
                            if dk == "import_desc_global_type" {
                                global_idx += 1;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    });
}

/// Extract the value type from a global's global_type node.
fn extract_global_value_type(node: &Node, source: &str) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "global_type" {
            // Search for value_type or ref_type inside global_type
            return extract_type_from_global_type(&child, source);
        }
    }
    None
}

/// Extract ValueType from a global_type node.
fn extract_type_from_global_type(node: &Node, source: &str) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "global_type_imm"
            || ck == "global_type_mut"
            || ck == "value_type"
            || ck == "ref_type"
        {
            let text = node_text(&child, source);
            // Strip "mut" wrapper if present
            let type_text = if ck == "global_type_mut" {
                // (mut i32) -> i32
                let inner = text.trim();
                if inner.starts_with("(mut") && inner.ends_with(')') {
                    inner[4..inner.len() - 1].trim()
                } else {
                    inner
                }
            } else {
                text.trim()
            };
            if let Some(vt) = ValueType::try_parse(type_text) {
                return Some(vt);
            }
            // Try extracting from children
            return extract_value_type_recursive(&child, source);
        }
    }
    None
}

/// Recursively extract ValueType from a node tree.
fn extract_value_type_recursive(node: &Node, source: &str) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "value_type" || ck == "ref_type" {
            let text = node_text(&child, source);
            if let Some(vt) = ValueType::try_parse(text.trim()) {
                return Some(vt);
            }
        }
        if let Some(vt) = extract_value_type_recursive(&child, source) {
            return Some(vt);
        }
    }
    None
}

/// Simple type compatibility for const expression checking.
fn const_types_compatible(actual: &ValueType, expected: &ValueType) -> bool {
    if *actual == ValueType::Unknown || *expected == ValueType::Unknown {
        return true;
    }
    if actual == expected {
        return true;
    }
    // Reference subtyping: funcref <: funcref, etc.
    match (actual, expected) {
        (ValueType::Funcref, ValueType::Funcref) => true,
        (ValueType::Externref, ValueType::Externref) => true,
        // Non-null ref subtypes nullable ref
        (ValueType::Ref(a), ValueType::RefNull(b)) if a == b => true,
        // Funcref covers Ref(n) and RefNull(n)
        (ValueType::Ref(_) | ValueType::RefNull(_), ValueType::Funcref) => true,
        // Anyref covers eqref, i31ref, structref, arrayref, Ref(n)
        (
            ValueType::Eqref | ValueType::I31ref | ValueType::Structref | ValueType::Arrayref,
            ValueType::Anyref,
        ) => true,
        (ValueType::Ref(_), ValueType::Anyref | ValueType::Eqref) => true,
        // Nullref is bottom for internal ref hierarchy
        (
            ValueType::Nullref,
            ValueType::Anyref
            | ValueType::Eqref
            | ValueType::I31ref
            | ValueType::Structref
            | ValueType::Arrayref
            | ValueType::Funcref
            | ValueType::Externref,
        ) => true,
        _ => false,
    }
}

/// Check const expression type for a global initializer.
fn check_const_expr_type_for_global(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    expected: &ValueType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Count instructions after global_type
    let mut past_global_type = false;
    let mut total_instrs = 0;
    let mut last_type: Option<ValueType> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "global_type" {
            past_global_type = true;
            continue;
        }
        if past_global_type
            && ck != "identifier"
            && ck != "export"
            && ck != "import"
            && ck != "("
            && ck != ")"
            && ck != "global"
        {
            let (count, ty) = count_const_instrs_deep(&child, source, symbols);
            total_instrs += count;
            if ty.is_some() {
                last_type = ty;
            }
        }
    }

    if total_instrs != 1 {
        diagnostics.push(
            Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
        );
    } else if let Some(ref actual) = last_type {
        if !const_types_compatible(actual, expected) {
            diagnostics.push(
                Diagnostic::error(node_to_range(node), "type mismatch").with_code("type-mismatch"),
            );
        }
    }
}

/// Check data segment offset expression type.
fn check_data_offset_type(
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
            let (count, ty) = count_const_instrs_deep(&child, source, symbols);
            // Expected: i32 for memory32, i64 for memory64
            let expected = ValueType::I32; // default to i32
            if count != 1 {
                diagnostics.push(
                    Diagnostic::error(node_to_range(node), "type mismatch")
                        .with_code("type-mismatch"),
                );
            } else if let Some(ref actual) = ty {
                if !const_types_compatible(actual, &expected) {
                    diagnostics.push(
                        Diagnostic::error(node_to_range(node), "type mismatch")
                            .with_code("type-mismatch"),
                    );
                }
            }
        }
    }
}

/// Check elem segment offset expression type.
fn check_elem_offset_type(
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
            let (count, ty) = count_const_instrs_deep(&child, source, symbols);
            let expected = ValueType::I32;
            if count != 1 {
                diagnostics.push(
                    Diagnostic::error(node_to_range(node), "type mismatch")
                        .with_code("type-mismatch"),
                );
            } else if let Some(ref actual) = ty {
                if !const_types_compatible(actual, &expected) {
                    diagnostics.push(
                        Diagnostic::error(node_to_range(node), "type mismatch")
                            .with_code("type-mismatch"),
                    );
                }
            }
        }
    }
}

/// Check table init expression type matches table's ref type.
fn check_table_init_type(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the table's ref type and init expression
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "table_fields_type" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                let gk = gc.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let gk = gk.as_str();
                if gk == "expr" {
                    let (count, ty) = count_const_instrs_deep(&gc, source, symbols);
                    // Get expected type from the table's ref_type
                    let expected =
                        extract_table_ref_type(&child, source).unwrap_or(ValueType::Funcref);
                    if count == 1 {
                        if let Some(ref actual) = ty {
                            if !const_types_compatible(actual, &expected) {
                                diagnostics.push(
                                    Diagnostic::error(node_to_range(node), "type mismatch")
                                        .with_code("type-mismatch"),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Extract the ref type from a table_fields_type node.
fn extract_table_ref_type(node: &Node, source: &str) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "table_type" {
            return extract_ref_type_from_node(&child, source);
        }
        if ck == "ref_type" || ck == "value_type" {
            let text = node_text(&child, source);
            return ValueType::try_parse(text.trim());
        }
    }
    None
}

/// Extract a ref type from a table_type or similar node.
fn extract_ref_type_from_node(node: &Node, source: &str) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "ref_type" || ck == "value_type" {
            let text = node_text(&child, source);
            if let Some(vt) = ValueType::try_parse(text.trim()) {
                return Some(vt);
            }
        }
        // Also check for abbreviated ref types like funcref, externref
        if ck == "_heap_type_or_ref" {
            let text = node_text(&child, source);
            if let Some(vt) = ValueType::try_parse(text.trim()) {
                return Some(vt);
            }
        }
        // Recurse
        if let Some(vt) = extract_ref_type_from_node(&child, source) {
            return Some(vt);
        }
    }
    None
}

// ============================================================================
// Step 5: global.get forward reference validation
// ============================================================================

/// Check that global.get in constant expressions doesn't forward-reference globals.
fn check_global_forward_refs(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut current_global_idx = 0usize;

    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();

        match fk {
            "module_field_import" => {
                // Count imported globals
                let mut cursor = field.walk();
                for child in field.children(&mut cursor) {
                    let ck = child.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let ck = ck.as_str();
                    if ck == "import_desc" {
                        let mut dc = child.walk();
                        for desc in child.children(&mut dc) {
                            let dk = desc.kind();
                            #[cfg(all(feature = "wasm", not(feature = "native")))]
                            let dk = dk.as_str();
                            if dk == "import_desc_global_type" {
                                current_global_idx += 1;
                            }
                        }
                    }
                }
            }
            "module_field_global" => {
                if !has_inline_import(field) {
                    // Check global.get refs in this global's init expr
                    check_global_get_forward_ref(
                        field,
                        source,
                        symbols,
                        current_global_idx,
                        diagnostics,
                    );
                }
                current_global_idx += 1;
            }
            _ => {}
        }
    });
}

/// Check global.get instructions in a global's init expression for forward references.
fn check_global_get_forward_ref(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    current_idx: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
            check_global_get_forward_ref_recursive(
                &child,
                source,
                symbols,
                current_idx,
                diagnostics,
            );
        }
    }
}

/// Recursively find global.get instructions and check for forward references.
fn check_global_get_forward_ref_recursive(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    current_idx: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    if kind == "instr_plain" {
        let text = node_text(node, source);
        let first_token = text.split_whitespace().next().unwrap_or("");
        if first_token == "global.get" {
            // Resolve the referenced global index
            if let Some(ref_idx) = resolve_global_ref_index(node, source, symbols) {
                if ref_idx >= current_idx {
                    diagnostics.push(
                        Diagnostic::error(
                            node_to_range(node),
                            format!("unknown global {}", ref_idx),
                        )
                        .with_code("unknown-global"),
                    );
                }
            }
        }
        return;
    }

    if kind == "expr1_plain" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let ck = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let ck = ck.as_str();
            if ck == "instr_plain" {
                let text = node_text(&child, source);
                let first_token = text.split_whitespace().next().unwrap_or("");
                if first_token == "global.get" {
                    if let Some(ref_idx) = resolve_global_ref_index(&child, source, symbols) {
                        if ref_idx >= current_idx {
                            diagnostics.push(
                                Diagnostic::error(
                                    node_to_range(node),
                                    format!("unknown global {}", ref_idx),
                                )
                                .with_code("unknown-global"),
                            );
                        }
                    }
                }
            } else {
                check_global_get_forward_ref_recursive(
                    &child,
                    source,
                    symbols,
                    current_idx,
                    diagnostics,
                );
            }
        }
        return;
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_global_get_forward_ref_recursive(&child, source, symbols, current_idx, diagnostics);
    }
}

/// Resolve a global reference to its absolute index.
fn resolve_global_ref_index(
    instr_node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Option<usize> {
    let mut cursor = instr_node.walk();
    for child in instr_node.children(&mut cursor) {
        let ck = child.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let ck = ck.as_str();
        if ck == "index" || ck == "identifier" {
            let text = node_text(&child, source);
            if text.starts_with('$') {
                return symbols.get_global_by_name(text).map(|g| g.index);
            }
            if let Ok(idx) = text.parse::<usize>() {
                return Some(idx);
            }
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                let ik = idx_child.kind();
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let ik = ik.as_str();
                if ik == "identifier" {
                    return symbols
                        .get_global_by_name(node_text(&idx_child, source))
                        .map(|g| g.index);
                }
                if ik == "nat" {
                    if let Ok(idx) = node_text(&idx_child, source).parse::<usize>() {
                        return Some(idx);
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// Step 7: Block-level type_use inline type mismatches
// ============================================================================

/// Check that block/loop/if nodes with type_use + inline params/results match.
fn check_block_type_use_mismatches(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    walk_for_block_type_use_mismatches(module, source, symbols, diagnostics);
}

fn walk_for_block_type_use_mismatches(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    if matches!(
        kind,
        "block_block"
            | "loop_block"
            | "block_if"
            | "if_block"
            | "block_try_table"
            | "block_try"
            | "instr_block"
            | "instr_loop"
            | "instr_if"
    ) {
        check_block_node_type_use(node, source, symbols, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_block_type_use_mismatches(&child, source, symbols, diagnostics);
    }
}

fn check_block_node_type_use(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut type_use_node = None;
    let mut has_inline_sig = false;
    let mut inline_params: Vec<String> = Vec::new();
    let mut inline_results: Vec<String> = Vec::new();

    // Check immediate children and one level of inner block
    check_block_children_for_type_use(
        node,
        source,
        &mut type_use_node,
        &mut has_inline_sig,
        &mut inline_params,
        &mut inline_results,
    );

    let type_use = match type_use_node {
        Some(n) if has_inline_sig => n,
        _ => return,
    };

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
}

#[cfg(feature = "native")]
fn check_block_children_for_type_use<'a>(
    node: &Node<'a>,
    source: &str,
    type_use_node: &mut Option<Node<'a>>,
    has_inline_sig: &mut bool,
    inline_params: &mut Vec<String>,
    inline_results: &mut Vec<String>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        match ck {
            "type_use" => *type_use_node = Some(child),
            "func_type_params" | "func_type_params_many" => {
                *has_inline_sig = true;
                extract_param_types(&child, source, inline_params);
            }
            "func_type_results" => {
                *has_inline_sig = true;
                extract_result_types(&child, source, inline_results);
            }
            "block_block" | "loop_block" | "block_if" | "if_block" | "block_try_table"
            | "block_try" => {
                // Recurse one level
                check_block_children_for_type_use(
                    &child,
                    source,
                    type_use_node,
                    has_inline_sig,
                    inline_params,
                    inline_results,
                );
            }
            _ => {}
        }
    }
}

#[cfg(all(feature = "wasm", not(feature = "native")))]
fn check_block_children_for_type_use(
    node: &Node,
    source: &str,
    type_use_node: &mut Option<Node>,
    has_inline_sig: &mut bool,
    inline_params: &mut Vec<String>,
    inline_results: &mut Vec<String>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        let ck = ck.as_str();
        match ck {
            "type_use" => *type_use_node = Some(child.clone()),
            "func_type_params" | "func_type_params_many" => {
                *has_inline_sig = true;
                extract_param_types(&child, source, inline_params);
            }
            "func_type_results" => {
                *has_inline_sig = true;
                extract_result_types(&child, source, inline_results);
            }
            "block_block" | "loop_block" | "block_if" | "if_block" | "block_try_table"
            | "block_try" => {
                check_block_children_for_type_use(
                    &child,
                    source,
                    type_use_node,
                    has_inline_sig,
                    inline_params,
                    inline_results,
                );
            }
            _ => {}
        }
    }
}

// ============================================================================
// Step 9: Table non-nullable ref type validation
// ============================================================================

/// Tables with non-nullable ref types require an initializer expression.
/// Also validate table init expression type matches table element type.
fn check_table_non_nullable_refs(module: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk != "module_field_table" {
            return;
        }
        if has_inline_import(field) {
            return;
        }

        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            let ck = child.kind();
            #[cfg(all(feature = "wasm", not(feature = "native")))]
            let ck = ck.as_str();
            if ck == "table_fields_type" {
                let text = node_text(&child, source);
                // Check if this table has a non-nullable ref type
                // Non-nullable: (ref func), (ref $t) — NOT (ref null func)
                let has_non_nullable = text.contains("(ref ") && !text.contains("(ref null");
                if !has_non_nullable {
                    return;
                }

                // Find the init expression
                let mut init_expr = None;
                let mut inner = child.walk();
                for gc in child.children(&mut inner) {
                    let gk = gc.kind();
                    #[cfg(all(feature = "wasm", not(feature = "native")))]
                    let gk = gk.as_str();
                    if gk == "expr" {
                        init_expr = Some(gc);
                        break;
                    }
                }

                match init_expr {
                    None => {
                        // Non-nullable table requires an init expression
                        diagnostics.push(
                            Diagnostic::error(node_to_range(field), "type mismatch")
                                .with_code("type-mismatch"),
                        );
                    }
                    Some(expr_node) => {
                        // Even with an init, ref.null is not valid for non-nullable tables
                        let expr_text = node_text(&expr_node, source);
                        if expr_text.contains("ref.null") {
                            diagnostics.push(
                                Diagnostic::error(node_to_range(field), "type mismatch")
                                    .with_code("type-mismatch"),
                            );
                        }
                    }
                }
            }
        }
    });
}

// ============================================================================
// Step 3: Check memory instructions with no memory declared
// ============================================================================

/// Memory instruction prefixes that implicitly use memory 0.
const MEMORY_INSTRUCTION_PREFIXES: &[&str] = &[
    "i32.load",
    "i64.load",
    "f32.load",
    "f64.load",
    "i32.store",
    "i64.store",
    "f32.store",
    "f64.store",
    "i32.load8",
    "i32.load16",
    "i64.load8",
    "i64.load16",
    "i64.load32",
    "i32.store8",
    "i32.store16",
    "i64.store8",
    "i64.store16",
    "i64.store32",
    "v128.load",
    "v128.store",
    "memory.size",
    "memory.grow",
    "memory.fill",
    "memory.copy",
    "memory.init",
    "memory.atomic",
    "i32.atomic",
    "i64.atomic",
];

fn is_memory_instruction(name: &str) -> bool {
    MEMORY_INSTRUCTION_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Check that memory instructions don't implicitly reference memory 0
/// when no memory is declared.
fn check_implicit_memory_refs(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !symbols.memories.is_empty() {
        return; // At least one memory exists, no need to check
    }

    // Walk all function bodies looking for memory instructions
    for_each_module_field(module, |field| {
        let fk = field.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let fk = fk.as_str();
        if fk == "module_field_func" {
            walk_for_memory_instrs(field, source, diagnostics);
        }
    });
}

fn walk_for_memory_instrs(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let kind = node.kind();
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    let kind = kind.as_str();

    if kind == "instr_plain" {
        let text = node_text(node, source);
        let first_token = text.split_whitespace().next().unwrap_or("");
        if is_memory_instruction(first_token) {
            diagnostics.push(
                Diagnostic::error(node_to_range(node), "unknown memory 0".to_string())
                    .with_code("unknown-memory"),
            );
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_memory_instrs(&child, source, diagnostics);
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
    fn test_global_get_const_non_imported_ok() {
        // Extended const expressions allow global.get of any immutable global
        let source = r#"(module
            (global $g i32 (i32.const 0))
            (global $h i32 (global.get $g))
        )"#;
        let diags = get_diagnostics(source);
        assert!(
            diags.is_empty(),
            "Expected no errors for immutable global.get in const, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
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
