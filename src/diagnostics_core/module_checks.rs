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
use crate::utils::{node_text, node_to_range};

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
        check_elem_segment_types(&module_node, source, symbols, &mut diagnostics);
        check_ref_func_declarations(&module_node, source, symbols, &mut diagnostics);
        check_unknown_type_refs(&module_node, source, symbols, &mut diagnostics);
        check_type_forward_refs(&module_node, source, symbols, &mut diagnostics);
        check_block_type_use_mismatches(&module_node, source, symbols, &mut diagnostics);
        check_table_non_nullable_refs(&module_node, source, &mut diagnostics);
        check_implicit_memory_refs(&module_node, source, symbols, &mut diagnostics);
        check_call_indirect_table_requirements(&module_node, source, symbols, &mut diagnostics);
        check_inline_elem_func_refs(&module_node, source, symbols, &mut diagnostics);
    }

    diagnostics
}

// ============================================================================
// Helper: find the module node
// ============================================================================

macro_rules! find_module_node_body {
    ($root:ident) => {{
        node_kind!(rk = $root);
        if rk == "module" {
            return Some(node_copy!($root));
        }
        let mut cursor = $root.walk();
        for child in $root.children(&mut cursor) {
            node_kind!(ck = child);
            if ck == "module" {
                return Some(node_copy!(&child));
            }
            if ck == "module_field" {
                return Some(node_copy!($root));
            }
        }
        None
    }};
}
#[cfg(feature = "native")]
fn find_module_node<'a>(root: &Node<'a>) -> Option<Node<'a>> {
    find_module_node_body!(root)
}
#[cfg(all(feature = "wasm", not(feature = "native")))]
fn find_module_node(root: &Node) -> Option<Node> {
    find_module_node_body!(root)
}

/// Helper: iterate module_field children of a module or root node.
/// Handles both `(module ...)` form and bare module fields.
fn for_each_module_field(module: &Node, mut f: impl FnMut(&Node)) {
    let mut cursor = module.walk();
    for child in module.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "module_field" {
            // Each module_field wraps one actual field
            let mut fc = child.walk();
            for field_child in child.children(&mut fc) {
                f(&field_child);
            }
        }
    }
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
        node_kind!(fk = field);
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
        node_kind!(fk = field);
        if fk != "module_field_start" {
            return;
        }

        // Find the function index referenced by start
        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            node_kind!(ck = child);
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
                        node_kind!(ik = idx_child);
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
        node_kind!(fk = field);

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
                    node_kind!(ck = child);
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
        node_kind!(ck = child);
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
        node_kind!(fk = field);

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

pub(super) fn has_inline_import(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "import" {
            return true;
        }
        // Check inside memory_fields_type and table_fields_type
        if ck == "memory_fields_type" || ck == "table_fields_type" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                node_kind!(gck = gc);
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
        node_kind!(fk = field);

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
                check_duplicate_struct_fields(field, source, diagnostics);
            }
            "module_field_tag" => {
                check_identifier_dup(field, source, &mut seen_tag, "tag", diagnostics);
                check_tag_no_results(field, source, diagnostics);
            }
            "module_field_rec" => {
                // Types inside rec groups share the type index space
                let mut cursor = field.walk();
                for child in field.children(&mut cursor) {
                    // Inside rec, look for type definitions. The grammar defines rec as:
                    // (rec (type $id? type_field) ...)
                    // But the inner nodes are anonymous sequences, so we look for identifier
                    // children or children that look like type definitions.
                    node_kind!(ck = child);
                    if ck == "module_field_type" {
                        check_identifier_dup(&child, source, &mut seen_type, "type", diagnostics);
                        check_duplicate_struct_fields(&child, source, diagnostics);
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
    if let Some(id_node) = crate::utils::find_child_by_kind(node, "identifier") {
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
        node_kind!(ck = child);
        if ck == "import_desc" {
            let mut dc = child.walk();
            for desc in child.children(&mut dc) {
                node_kind!(dk = desc);
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
                        check_tag_no_results(&desc, source, diagnostics);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Check that a tag (or imported tag) has no result types.
fn check_tag_no_results(node: &Node, _source: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Recursively search for func_type_results nodes
    fn has_results(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            node_kind!(ck = child);
            if ck == "func_type_results" {
                diagnostics.push(Diagnostic::error(
                    node_to_range(&child),
                    "non-empty tag result type".to_string(),
                ));
                return true;
            }
            if has_results(&child, diagnostics) {
                return true;
            }
        }
        false
    }
    has_results(node, diagnostics);
}

/// Check for duplicate named fields within a struct type definition.
fn check_duplicate_struct_fields(
    type_node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen_fields: HashMap<String, Range> = HashMap::new();
    find_struct_and_check_fields(type_node, source, &mut seen_fields, diagnostics);
}

/// Recursively find struct_type nodes and check their fields for duplicates.
fn find_struct_and_check_fields(
    node: &Node,
    source: &str,
    seen: &mut HashMap<String, Range>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "struct_type" {
            collect_struct_field_names(&child, source, seen, diagnostics);
        } else {
            find_struct_and_check_fields(&child, source, seen, diagnostics);
        }
    }
}

/// Collect and check named fields in a struct for duplicates.
fn collect_struct_field_names(
    node: &Node,
    source: &str,
    seen: &mut HashMap<String, Range>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "field_type" {
            // Check for identifier children (named fields)
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    let name = node_text(&inner_child, source).to_string();
                    let range = node_to_range(&inner_child);
                    if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(name.clone()) {
                        e.insert(range);
                    } else {
                        diagnostics.push(Diagnostic::error(
                            range,
                            format!("duplicate field {}", name),
                        ));
                    }
                }
            }
        } else if kind != "(" && kind != ")" && kind != "struct" {
            // Recurse into other structural nodes
            collect_struct_field_names(&child, source, seen, diagnostics);
        }
    }
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
        node_kind!(fk = field);

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
            node_kind!(ck = child);
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
                    collect_value_types_recursive(&child, source, &mut inline_params);
                }
                "func_type_results" => {
                    has_inline_sig = true;
                    collect_value_types_recursive(&child, source, &mut inline_results);
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

fn collect_value_types_recursive(node: &Node, source: &str, out: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
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
        node_kind!(ck = child);
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
                node_kind!(ik = idx_child);
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
    let mut global_index = symbols.num_imported_globals;
    for_each_module_field(module, |field| {
        node_kind!(fk = field);

        match fk {
            "module_field_global" => {
                // Check instructions after global_type
                // In global init: only globals with index < current are available
                check_global_init_expr(field, source, symbols, global_index, diagnostics);
                global_index += 1;
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
    global_index: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Global: (global $id? type init_expr...)
    // init instructions appear after global_type as instr or expr nodes
    let mut past_global_type = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "global_type" {
            past_global_type = true;
            continue;
        }
        if past_global_type {
            // In global init: allow imported + earlier module-defined globals
            check_const_instruction_tree(&child, source, symbols, global_index, diagnostics);
        }
    }
}

fn check_offset_expr(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Data/elem offsets: all defined globals are available
    let max_global = symbols.globals.len();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "offset" {
            check_const_instruction_tree(&child, source, symbols, max_global, diagnostics);
        }
    }
}

fn check_elem_exprs(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Elem expressions: all defined globals are available
    let max_global = symbols.globals.len();
    // elem_expr nodes are inside elem_list, not direct children of module_field_elem
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "elem_list" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                node_kind!(gk = gc);
                if gk == "elem_expr" {
                    check_const_instruction_tree(&gc, source, symbols, max_global, diagnostics);
                }
            }
        }
    }
}

fn check_table_init_expr(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Table init: only imported globals (tables come before globals in binary format)
    let max_global = symbols.num_imported_globals;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        // table_fields_type may contain an expr child
        if ck == "table_fields_type" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                node_kind!(gk = gc);
                if gk == "expr" {
                    check_const_instruction_tree(&gc, source, symbols, max_global, diagnostics);
                }
            }
        }
    }
}

/// Recursively check that all instructions in a tree are constant expressions.
/// `max_allowed_global`: maximum global index that can be referenced by global.get.
fn check_const_instruction_tree(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    max_allowed_global: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    node_kind!(kind = node);

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
                check_global_get_in_const(node, source, symbols, max_allowed_global, diagnostics);
            }
            return;
        }
        "expr1_plain" => {
            // Folded expression: first child is instr_plain, check its opcode
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                node_kind!(ck = child);
                if ck == "instr_plain" {
                    let text = node_text(&child, source);
                    let first_token = text.split_whitespace().next().unwrap_or("");
                    if !CONST_INSTRUCTIONS.contains(&first_token) {
                        diagnostics.push(Diagnostic::error(
                            node_to_range(node),
                            format!("Constant expression required, but found '{}'", first_token),
                        ));
                    } else if first_token == "global.get" {
                        check_global_get_in_const(
                            &child,
                            source,
                            symbols,
                            max_allowed_global,
                            diagnostics,
                        );
                    }
                } else if ck == "expr" {
                    // Check nested expressions for constness too
                    check_const_instruction_tree(
                        &child,
                        source,
                        symbols,
                        max_allowed_global,
                        diagnostics,
                    );
                }
            }
            return;
        }
        // call/call_indirect instructions are never valid in const expressions
        "instr_call" | "expr1_call" | "instr_list_call" => {
            diagnostics.push(
                Diagnostic::error(node_to_range(node), "constant expression required")
                    .with_code("constant-expression-required"),
            );
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
        check_const_instruction_tree(&child, source, symbols, max_allowed_global, diagnostics);
    }
}

/// Check that global.get in a constant expression references a valid global.
fn check_global_get_in_const(
    instr_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    max_allowed_global: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the index/identifier child
    let mut cursor = instr_node.walk();
    for child in instr_node.children(&mut cursor) {
        node_kind!(ck = child);
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
                    node_kind!(ik = idx_child);
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
                } else if global.index >= max_allowed_global {
                    // global.get can only reference globals defined before this point
                    diagnostics.push(
                        Diagnostic::error(
                            node_to_range(instr_node),
                            format!("unknown global {}", global.index),
                        )
                        .with_code("unknown-global"),
                    );
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
        node_kind!(fk = field);
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
            node_kind!(ck = child);
            if ck == "offset" {
                has_offset = true;
            }
            if ck == "memory_use" {
                // (memory $idx) or (memory N)
                let mut mc = child.walk();
                for mc_child in child.children(&mut mc) {
                    node_kind!(mk = mc_child);
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
        node_kind!(fk = field);
        if fk != "module_field_elem" {
            return;
        }

        // Look for table_use child or active segment with offset
        let mut has_offset = false;
        let mut table_idx: Option<usize> = None;
        let mut table_node_range = None;

        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            node_kind!(ck = child);
            if ck == "offset" {
                has_offset = true;
            }
            if ck == "table_use" {
                let mut tc = child.walk();
                for tc_child in child.children(&mut tc) {
                    node_kind!(tk = tc_child);
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
// Element segment type validation
// ============================================================================

/// Validate elem segment item types against declared elem type and target table type.
fn check_elem_segment_types(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        node_kind!(fk = field);
        if fk != "module_field_elem" {
            return;
        }

        // Extract declared elem type from elem_list (e.g. funcref, externref)
        let mut elem_type: Option<ValueType> = None;
        let mut elem_type_nullable = true; // nullable until proven otherwise
        let mut table_idx: Option<usize> = None;
        let mut has_offset = false;

        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            node_kind!(ck = child);

            if ck == "offset" {
                has_offset = true;
            }
            if ck == "table_use" {
                let mut tc = child.walk();
                for tc_child in child.children(&mut tc) {
                    node_kind!(tk = tc_child);
                    if tk == "index" || tk == "identifier" || tk == "nat" {
                        let text = node_text(&tc_child, source);
                        if text.starts_with('$') {
                            if let Some(tbl) = symbols.get_table_by_name(text.trim()) {
                                table_idx = Some(tbl.index);
                            }
                        } else if let Ok(idx) = text.trim().parse::<usize>() {
                            table_idx = Some(idx);
                        }
                    }
                }
            }
            if ck == "elem_list" {
                // Extract ref_type from elem_list
                let mut ec = child.walk();
                for ec_child in child.children(&mut ec) {
                    node_kind!(ek = ec_child);
                    if ek == "ref_type" || ek == "value_type" {
                        let text = node_text(&ec_child, source);
                        elem_type = ValueType::try_parse(text.trim());
                        // Check if the ref_type AST node is non-nullable
                        if is_non_nullable_ref_type_node(&ec_child, source) {
                            elem_type_nullable = false;
                        }
                    }
                }
            }
            // Also check for ref_type directly on the elem segment node
            if ck == "ref_type" && elem_type.is_none() {
                let text = node_text(&child, source);
                elem_type = ValueType::try_parse(text.trim());
                if is_non_nullable_ref_type_node(&child, source) {
                    elem_type_nullable = false;
                }
            }
        }

        // Old-style elem segments without explicit type:
        // "func" keyword or bare indices = non-nullable func references
        let elem_type = elem_type.unwrap_or(ValueType::Funcref);
        if elem_type == ValueType::Funcref && !has_explicit_nullable_type(field, source) {
            elem_type_nullable = false;
        }

        // Check 1: elem type vs target table type (for active segments)
        if has_offset {
            let tbl_idx = table_idx.unwrap_or(0);
            if let Some(table) = symbols.tables.get(tbl_idx) {
                if !elem_ref_compatible(
                    &elem_type,
                    &table.ref_type,
                    table.ref_type_non_nullable,
                    elem_type_nullable,
                ) {
                    diagnostics.push(
                        Diagnostic::error(node_to_range(field), "type mismatch")
                            .with_code("type-mismatch"),
                    );
                }
            }
        }

        // Check 2: validate each elem expression item against the declared elem type
        let mut cursor2 = field.walk();
        for child in field.children(&mut cursor2) {
            node_kind!(ck = child);

            if ck == "elem_list" {
                check_elem_list_items(&child, &elem_type, source, symbols, diagnostics);
            }
        }
    });
}

/// Check if an elem segment has an explicitly nullable type keyword (`funcref` or `externref`).
/// Returns false for old-style segments (bare indices, `func` keyword), `(ref func)`, etc.
fn has_explicit_nullable_type(elem_node: &Node, source: &str) -> bool {
    let mut cursor = elem_node.walk();
    for child in elem_node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "elem_list" || ck == "ref_type" || ck == "value_type" {
            let text = node_text(&child, source).trim().to_string();
            if text == "funcref" || text == "externref" {
                return true;
            }
            // Check for nested ref_type children
            let mut ec = child.walk();
            for gc in child.children(&mut ec) {
                node_kind!(gk = gc);
                if gk == "ref_type" || gk == "ref_type_funcref" || gk == "ref_type_externref" {
                    let gtext = node_text(&gc, source).trim().to_string();
                    if gtext == "funcref" || gtext == "externref" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if a ref_type AST node is a non-nullable reference type.
fn is_non_nullable_ref_type_node(node: &Node, source: &str) -> bool {
    node_kind!(kind = node);
    match kind {
        "ref_type_ref" | "ref_type_concrete" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if &source[child.byte_range()] == "null" {
                    return false;
                }
            }
            true
        }
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
            ) {
                return false;
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if is_non_nullable_ref_type_node(&child, source) {
                    return true;
                }
            }
            false
        }
    }
}

/// Check if elem ref type is compatible with table element type.
fn elem_ref_compatible(
    elem_type: &ValueType,
    table_type: &ValueType,
    table_non_nullable: bool,
    elem_nullable: bool,
) -> bool {
    use ValueType::*;

    // If the table type is non-nullable, nullable elem types are incompatible
    if table_non_nullable && elem_nullable {
        return false;
    }

    if elem_type == table_type {
        return true;
    }
    match (elem_type, table_type) {
        // funcref subtypes match funcref or non-nullable funcref types
        (Funcref | NullFuncref | Ref(_) | RefNull(_), Funcref | Ref(_) | RefNull(_)) => true,
        // externref subtypes match externref
        (Externref | NullExternref, Externref) => true,
        // Cross-hierarchy is always incompatible
        (Funcref | NullFuncref | Ref(_) | RefNull(_), Externref) => false,
        (Externref | NullExternref, Funcref | Ref(_) | RefNull(_)) => false,
        // For types we can't resolve (Structref fallback, etc.), be permissive
        _ => true,
    }
}

/// Check items in an elem_list against the declared elem type.
fn check_elem_list_items(
    elem_list: &Node,
    declared_type: &ValueType,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = elem_list.walk();
    for child in elem_list.children(&mut cursor) {
        node_kind!(ck = child);

        if ck == "elem_expr" {
            check_single_elem_expr(&child, declared_type, source, symbols, diagnostics);
        }
    }
}

/// Check a single elem expression against the declared elem type.
fn check_single_elem_expr(
    elem_expr: &Node,
    declared_type: &ValueType,
    source: &str,
    _symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Count instruction-producing expressions
    let mut instr_count = 0;
    let mut inferred_type: Option<ValueType> = None;

    let mut cursor = elem_expr.walk();
    for child in elem_expr.children(&mut cursor) {
        node_kind!(ck = child);

        match ck {
            "elem_expr_item" => {
                // (item instr...) — count instructions inside
                let mut item_instrs = 0;
                let mut item_type: Option<ValueType> = None;
                let mut ic = child.walk();
                for item_child in child.children(&mut ic) {
                    if let Some(ty) = infer_elem_instr_type(&item_child, source) {
                        item_instrs += 1;
                        item_type = Some(ty);
                    }
                }
                if item_instrs > 1 {
                    diagnostics.push(
                        Diagnostic::error(node_to_range(elem_expr), "type mismatch")
                            .with_code("type-mismatch"),
                    );
                    return;
                }
                if let Some(ref ty) = item_type {
                    if !ref_type_matches(ty, declared_type) {
                        diagnostics.push(
                            Diagnostic::error(node_to_range(elem_expr), "type mismatch")
                                .with_code("type-mismatch"),
                        );
                    }
                }
                return;
            }
            "elem_expr_expr" | "expr" => {
                instr_count += 1;
                inferred_type = infer_elem_expr_type(&child, source);
            }
            _ => {}
        }
    }

    if instr_count > 1 {
        diagnostics.push(
            Diagnostic::error(node_to_range(elem_expr), "type mismatch").with_code("type-mismatch"),
        );
        return;
    }
    if let Some(ref ty) = inferred_type {
        if !ref_type_matches(ty, declared_type) {
            diagnostics.push(
                Diagnostic::error(node_to_range(elem_expr), "type mismatch")
                    .with_code("type-mismatch"),
            );
        }
    }
}

/// Infer the type produced by an instruction in an elem context.
/// Returns None for non-instruction nodes (parens, keywords, etc.)
fn infer_elem_instr_type(node: &Node, source: &str) -> Option<ValueType> {
    node_kind!(kind = node);

    match kind {
        "instr_plain" | "expr1_plain" => {
            let text = node_text(node, source);
            let first_token = text.split_whitespace().next()?;
            infer_const_instr_result_type(first_token, &text)
        }
        "expr" | "expr1" | "instr" => {
            // Recurse to find the instruction inside
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(ty) = infer_elem_instr_type(&child, source) {
                    return Some(ty);
                }
            }
            None
        }
        _ => None,
    }
}

/// Infer the result type of an elem expression (which is an expr node).
fn infer_elem_expr_type(node: &Node, source: &str) -> Option<ValueType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(ty) = infer_elem_instr_type(&child, source) {
            return Some(ty);
        }
    }
    None
}

/// Infer the result type of a const instruction by its name and full text.
fn infer_const_instr_result_type(name: &str, full_text: &str) -> Option<ValueType> {
    match name {
        "ref.func" => Some(ValueType::Funcref),
        "ref.null" => {
            // Determine null type from argument: ref.null func, ref.null extern, etc.
            let arg = full_text.split_whitespace().nth(1).unwrap_or("func");
            match arg {
                "extern" | "noextern" => Some(ValueType::Externref),
                _ => Some(ValueType::Funcref), // func, nofunc, and concrete types
            }
        }
        "i32.const" => Some(ValueType::I32),
        "i64.const" => Some(ValueType::I64),
        "f32.const" => Some(ValueType::F32),
        "f64.const" => Some(ValueType::F64),
        _ => None,
    }
}

/// Check if a value type matches the declared elem ref type.
fn ref_type_matches(actual: &ValueType, expected: &ValueType) -> bool {
    use ValueType::*;
    if actual == expected {
        return true;
    }
    match (actual, expected) {
        // funcref (from ref.null or ref.func) matches funcref
        (Funcref, Funcref) => true,
        (NullFuncref, Funcref) => true,
        // externref matches externref
        (Externref, Externref) => true,
        (NullExternref, Externref) => true,
        // funcref subtypes match funcref
        (Ref(_) | RefNull(_), Funcref) => true,
        // non-ref types don't match ref types
        (I32 | I64 | F32 | F64 | V128, _) => false,
        // funcref doesn't match externref and vice versa
        (Funcref, Externref) | (Externref, Funcref) => false,
        // Unknown matches anything
        (Unknown, _) | (_, Unknown) => true,
        _ => true, // Be permissive for types we don't fully track
    }
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
        node_kind!(fk = field);
        if fk != "module_field_elem" {
            return;
        }
        collect_elem_declared_funcs(field, source, symbols, &mut declared_funcs);
    });

    // Also collect functions referenced in inline elem expressions on tables
    for_each_module_field(module, |field| {
        node_kind!(fk = field);
        if fk != "module_field_table" {
            return;
        }
        collect_inline_table_elem_funcs(field, source, symbols, &mut declared_funcs);
    });

    // Also collect functions referenced via ref.func in global/table init expressions
    // Per spec, ref.func in const init contexts counts as a function declaration
    for_each_module_field(module, |field| {
        node_kind!(fk = field);
        if fk == "module_field_global" || fk == "module_field_table" {
            collect_ref_func_in_const_expr(field, source, symbols, &mut declared_funcs);
        }
    });

    // Also collect exported functions — per spec, exported functions are considered "declared"
    for_each_module_field(module, |field| {
        node_kind!(fk = field);
        match fk {
            "module_field_export" => {
                // Standalone export: (export "name" (func $idx))
                collect_exported_func(field, source, symbols, &mut declared_funcs);
            }
            "module_field_func" => {
                // Inline export: (func $f (export "name") ...)
                let mut cursor = field.walk();
                let mut has_export = false;
                for child in field.children(&mut cursor) {
                    node_kind!(ck = child);
                    if ck == "export" {
                        has_export = true;
                        break;
                    }
                }
                if has_export {
                    // Find the function's index from its identifier or position
                    let mut cursor2 = field.walk();
                    for child in field.children(&mut cursor2) {
                        node_kind!(ck = child);
                        if ck == "identifier" {
                            let name = node_text(&child, source).trim();
                            if let Some(idx) = resolve_func_index(name, symbols) {
                                declared_funcs.insert(idx);
                            }
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    });

    // Step 2: Walk all function bodies to find ref.func usage
    collect_ref_func_errors(module, source, symbols, &declared_funcs, diagnostics);
}

/// Collect function index from a standalone export node: (export "name" (func $idx))
fn collect_exported_func(
    export_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &mut HashSet<usize>,
) {
    // Walk recursively to find export_desc_func (nested inside export_desc)
    find_exported_func_recursive(export_node, source, symbols, declared);
}

fn find_exported_func_recursive(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &mut HashSet<usize>,
) {
    node_kind!(kind = node);
    if kind == "export_desc_func" {
        // (func $idx) — find the index child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            node_kind!(ck = child);
            if ck == "index" {
                let idx_text = node_text(&child, source).trim();
                if let Some(idx) = resolve_func_index(idx_text, symbols) {
                    declared.insert(idx);
                }
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_exported_func_recursive(&child, source, symbols, declared);
    }
}

/// Collect function indices referenced by ref.func in a global init expression.
fn collect_ref_func_in_const_expr(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    declared: &mut HashSet<usize>,
) {
    node_kind!(kind = node);

    if kind == "instr_plain" {
        let text = node_text(node, source);
        if text.starts_with("ref.func") {
            // Find the function reference
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                node_kind!(ck = child);
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
        node_kind!(ck = child);

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
    node_kind!(kind = node);

    // Check if this is a ref.func instruction
    if kind == "instr_plain" || kind == "op_index" || kind == "op_nullary" {
        let text = node_text(node, source).trim();
        if text.starts_with("ref.func") {
            // Find the function reference argument
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                node_kind!(ck = child);

                // Look inside op_ nodes for the index
                if ck.starts_with("op_") {
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        node_kind!(ik = inner);
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
    node_kind!(kind = node);

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
        node_kind!(ck = child);
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
        node_kind!(ck = child);

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
    // Compute actual type count: explicit types + unique implicit types from functions.
    // Functions with a type_use or matching an existing signature don't create new implicit types.
    let max_type_count = symbols.types.len() + count_unique_implicit_types(symbols);
    walk_for_unknown_type_refs(module, source, max_type_count, diagnostics);
}

/// Check for forward type references in type definitions.
/// Outside of `(rec ...)` groups, type definitions can only reference
/// previously-defined types (no forward or self references).
fn check_type_forward_refs(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut type_index = 0usize;
    for_each_module_field(module, |node| {
        node_kind!(kind = node);

        if kind == "module_field_type" {
            check_type_body_for_forward_refs(node, source, symbols, type_index, diagnostics);
            type_index += 1;
        } else if kind == "module_field_rec" {
            // Count types inside rec group but skip forward ref checks
            // (forward references within rec groups are allowed)
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                node_kind!(ck = child);
                if ck == "module_field_type" {
                    type_index += 1;
                }
            }
        }
    });
}

/// Walk a type definition body and check for forward type references.
fn check_type_body_for_forward_refs(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    current_type_index: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    node_kind!(kind = node);

    // Look for type references in ref_type, _heap_type_or_ref, value_type_ref_type
    if kind == "ref_type" || kind == "_heap_type_or_ref" || kind == "value_type_ref_type" {
        check_ref_for_forward_type(node, source, symbols, current_type_index, diagnostics);
        return; // Don't recurse into children — already checked
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_type_body_for_forward_refs(&child, source, symbols, current_type_index, diagnostics);
    }
}

/// Check if a ref_type or similar node contains a forward type reference.
fn check_ref_for_forward_type(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    current_type_index: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);

        if ck == "identifier" {
            let name = node_text(&child, source);
            if let Some(td) = symbols.get_type_by_name(name) {
                if td.index > current_type_index {
                    diagnostics.push(
                        Diagnostic::error(node_to_range(&child), format!("unknown type {}", name))
                            .with_code("unknown-type"),
                    );
                }
            }
            return;
        }

        if ck == "index" {
            // Check for numeric or identifier children
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                node_kind!(ik = idx_child);

                if ik == "identifier" {
                    let name = node_text(&idx_child, source);
                    if let Some(td) = symbols.get_type_by_name(name) {
                        if td.index > current_type_index {
                            diagnostics.push(
                                Diagnostic::error(
                                    node_to_range(&idx_child),
                                    format!("unknown type {}", name),
                                )
                                .with_code("unknown-type"),
                            );
                        }
                    }
                } else if ik == "nat" || ik == "dec_nat" || ik == "hex_nat" {
                    if let Ok(idx) = parse_nat(node_text(&idx_child, source)) {
                        if idx > current_type_index {
                            diagnostics.push(
                                Diagnostic::error(
                                    node_to_range(&idx_child),
                                    format!("unknown type {}", idx),
                                )
                                .with_code("unknown-type"),
                            );
                        }
                    }
                }
            }
            return;
        }

        // Recurse into children (e.g., nested ref types)
        check_ref_for_forward_type(&child, source, symbols, current_type_index, diagnostics);
    }
}

/// Count unique implicit function types (signatures not matching any explicit type).
/// Only functions with inline signatures (no type_use) create implicit types.
fn count_unique_implicit_types(symbols: &SymbolTable) -> usize {
    // Use HashSet for O(1) dedup instead of O(n) Vec.any()
    let mut seen = std::collections::HashSet::new();
    for type_def in &symbols.types {
        if let TypeKind::Func { params, results } = &type_def.kind {
            seen.insert((params.clone(), results.clone()));
        }
    }

    let mut implicit_count = 0;
    for func in &symbols.functions {
        if func.has_type_use {
            continue;
        }
        let params: Vec<ValueType> = func.parameters.iter().map(|p| p.param_type).collect();
        if seen.insert((params, func.results.clone())) {
            implicit_count += 1;
        }
    }
    implicit_count
}

/// Recursively walk the AST looking for numeric type indices in ref types and type_use.
fn walk_for_unknown_type_refs(
    node: &Node,
    source: &str,
    max_type_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    node_kind!(kind = node);

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
        node_kind!(ck = child);

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
        node_kind!(ck = child);

        if ck == "index" {
            // Check the index node's children for a numeric value
            let mut idx_cursor = child.walk();
            for idx_child in child.children(&mut idx_cursor) {
                node_kind!(ick = idx_child);

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
    crate::parser::parse_wat_nat(text)
        .map(|v| v as usize)
        .ok_or(())
}

// ============================================================================
// Step 8: Table size limits
// ============================================================================

fn check_table_limits(symbols: &SymbolTable, diagnostics: &mut Vec<Diagnostic>) {
    const MAX_TABLE32_SIZE: u64 = 0xFFFF_FFFF; // 2^32 - 1

    for table in &symbols.tables {
        // table64 has a much higher limit; skip the 32-bit check
        if table.is_table64 {
            continue;
        }

        let range = table
            .range
            .unwrap_or_else(|| Range::from_coords(table.line, 0, table.line, 0));

        if table.limits.0 > MAX_TABLE32_SIZE {
            diagnostics.push(
                Diagnostic::error(range, "table size must be at most 2^32-1")
                    .with_code("table-size"),
            );
        }

        if let Some(max) = table.limits.1 {
            if max > MAX_TABLE32_SIZE {
                diagnostics.push(
                    Diagnostic::error(range, "table size must be at most 2^32-1")
                        .with_code("table-size"),
                );
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
                Some("noextern") | Some("nullexternref") => Some(ValueType::NullExternref),
                Some("nofunc") | Some("nullfuncref") => Some(ValueType::NullFuncref),
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
        node_kind!(ck = child);
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
                node_kind!(ik = idx_child);
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
        node_kind!(ck = child);
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
                node_kind!(ik = idx_child);
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
        node_kind!(ck = child);
        if ck == "index" || ck == "identifier" {
            let text = node_text(&child, source);
            if text.starts_with('$') {
                return symbols.get_global_by_name(text).map(|g| g.var_type);
            }
            if let Ok(idx) = text.parse::<usize>() {
                return symbols.get_global_by_index(idx).map(|g| g.var_type);
            }
            let mut ic = child.walk();
            for idx_child in child.children(&mut ic) {
                node_kind!(ik = idx_child);
                if ik == "identifier" {
                    return symbols
                        .get_global_by_name(node_text(&idx_child, source))
                        .map(|g| g.var_type);
                }
                if ik == "nat" {
                    if let Ok(idx) = node_text(&idx_child, source).parse::<usize>() {
                        return symbols.get_global_by_index(idx).map(|g| g.var_type);
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
    node_kind!(kind = node);

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
                node_kind!(ck = child);
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
        node_kind!(fk = field);

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
                    node_kind!(ck = child);
                    if ck == "import_desc" {
                        let mut dc = child.walk();
                        for desc in child.children(&mut dc) {
                            node_kind!(dk = desc);
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
        node_kind!(ck = child);
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
        node_kind!(ck = child);
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
        node_kind!(ck = child);
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
        // NullFuncref is bottom for func hierarchy
        (
            ValueType::NullFuncref,
            ValueType::Funcref | ValueType::RefNull(_) | ValueType::Ref(_),
        ) => true,
        // NullExternref is bottom for extern hierarchy
        (ValueType::NullExternref, ValueType::Externref) => true,
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
        node_kind!(ck = child);
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

/// Resolve the expected offset type for a data segment's target memory.
/// Checks for `memory_use` child (explicit memory ref) or defaults to memory 0.
fn resolve_data_offset_type(node: &Node, source: &str, symbols: &SymbolTable) -> ValueType {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "memory_use" {
            // Explicit (memory $idx) — resolve against symbol table
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                node_kind!(gk = gc);
                if gk == "index" || gk == "identifier" {
                    let idx_text = node_text(&gc, source);
                    if let Some(mem) = symbols.get_memory_by_name(idx_text) {
                        return if mem.is_memory64 {
                            ValueType::I64
                        } else {
                            ValueType::I32
                        };
                    }
                    if let Some(n) = crate::parser::parse_wat_nat(idx_text) {
                        if let Some(mem) = symbols.memories.get(n as usize) {
                            return if mem.is_memory64 {
                                ValueType::I64
                            } else {
                                ValueType::I32
                            };
                        }
                    }
                }
            }
        }
    }
    // Default to memory 0
    symbols
        .memories
        .first()
        .map(|m| {
            if m.is_memory64 {
                ValueType::I64
            } else {
                ValueType::I32
            }
        })
        .unwrap_or(ValueType::I32)
}

/// Check data segment offset expression type.
fn check_data_offset_type(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expected = resolve_data_offset_type(node, source, symbols);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "offset" {
            let (count, ty) = count_const_instrs_deep(&child, source, symbols);
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

/// Resolve the expected offset type for an elem segment's target table.
/// Checks for `table_use` child (explicit table ref) or defaults to table 0.
fn resolve_elem_offset_type(node: &Node, source: &str, symbols: &SymbolTable) -> ValueType {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "table_use" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                node_kind!(gk = gc);
                if gk == "index" || gk == "identifier" {
                    let idx_text = node_text(&gc, source);
                    if let Some(table) = symbols.get_table_by_name(idx_text) {
                        return if table.is_table64 {
                            ValueType::I64
                        } else {
                            ValueType::I32
                        };
                    }
                    if let Some(n) = crate::parser::parse_wat_nat(idx_text) {
                        if let Some(table) = symbols.tables.get(n as usize) {
                            return if table.is_table64 {
                                ValueType::I64
                            } else {
                                ValueType::I32
                            };
                        }
                    }
                }
            }
        }
    }
    // Default to table 0
    symbols
        .tables
        .first()
        .map(|t| {
            if t.is_table64 {
                ValueType::I64
            } else {
                ValueType::I32
            }
        })
        .unwrap_or(ValueType::I32)
}

/// Check elem segment offset expression type.
fn check_elem_offset_type(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expected = resolve_elem_offset_type(node, source, symbols);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "offset" {
            let (count, ty) = count_const_instrs_deep(&child, source, symbols);
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
        node_kind!(ck = child);
        if ck == "table_fields_type" {
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                node_kind!(gk = gc);
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
        node_kind!(ck = child);
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
        node_kind!(ck = child);
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
        node_kind!(fk = field);

        match fk {
            "module_field_import" => {
                // Count imported globals
                let mut cursor = field.walk();
                for child in field.children(&mut cursor) {
                    node_kind!(ck = child);
                    if ck == "import_desc" {
                        let mut dc = child.walk();
                        for desc in child.children(&mut dc) {
                            node_kind!(dk = desc);
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
        node_kind!(ck = child);
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
    node_kind!(kind = node);

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
            node_kind!(ck = child);
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
        node_kind!(ck = child);
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
                node_kind!(ik = idx_child);
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
    node_kind!(kind = node);

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
            | "expr1_block"
            | "expr1_loop"
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
                collect_value_types_recursive(&child, source, inline_params);
            }
            "func_type_results" => {
                *has_inline_sig = true;
                collect_value_types_recursive(&child, source, inline_results);
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
                collect_value_types_recursive(&child, source, inline_params);
            }
            "func_type_results" => {
                *has_inline_sig = true;
                collect_value_types_recursive(&child, source, inline_results);
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
        node_kind!(fk = field);
        if fk != "module_field_table" {
            return;
        }
        if has_inline_import(field) {
            return;
        }

        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            node_kind!(ck = child);
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
                    node_kind!(gk = gc);
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
        node_kind!(fk = field);
        if fk == "module_field_func" {
            walk_for_memory_instrs(field, source, diagnostics);
        }
    });
}

fn walk_for_memory_instrs(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    node_kind!(kind = node);

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
// call_indirect / return_call_indirect table requirement checks
// ============================================================================

/// Check that call_indirect/return_call_indirect instructions have a valid table.
/// - If no table exists: "unknown table"
/// - If table exists but is externref: "type mismatch" (needs funcref table)
fn check_call_indirect_table_requirements(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        node_kind!(fk = field);
        if fk == "module_field_func" {
            walk_for_call_indirect_checks(field, source, symbols, diagnostics);
        }
    });
}

/// Resolve which table a call_indirect node targets.
/// For multi-table, the first index/identifier child that matches a table name
/// (or numeric index) is used. Falls back to table 0.
fn resolve_call_indirect_table_idx(node: &Node, source: &str, symbols: &SymbolTable) -> usize {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "index" || ck == "identifier" {
            let text = source[child.byte_range()].trim();
            // Check if this is a table name
            if text.starts_with('$') {
                if let Some((i, _)) = symbols
                    .tables
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.name.as_deref() == Some(text))
                {
                    return i;
                }
                // Not a table name — could be a type name, skip
            } else if let Ok(idx) = parse_nat(text) {
                // Numeric index — if it's within table count, it's a table index
                // (For single-table modules, the first numeric index is the type index)
                if idx < symbols.tables.len() && symbols.tables.len() > 1 {
                    return idx;
                }
            }
        }
    }
    0 // default to table 0
}

fn check_call_indirect_table(
    node: &Node,
    report_node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if symbols.tables.is_empty() {
        diagnostics.push(
            Diagnostic::error(node_to_range(report_node), "unknown table".to_string())
                .with_code("unknown-table"),
        );
    } else {
        let table_idx = resolve_call_indirect_table_idx(node, source, symbols);
        if let Some(table) = symbols.tables.get(table_idx) {
            if table.ref_type == ValueType::Externref {
                diagnostics.push(
                    Diagnostic::error(node_to_range(report_node), "type mismatch".to_string())
                        .with_code("type-mismatch"),
                );
            }
        }
    }
}

fn walk_for_call_indirect_checks(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    node_kind!(kind = node);

    if kind == "instr_plain" {
        let text = node_text(node, source);
        let first_token = text.split_whitespace().next().unwrap_or("");
        if first_token == "call_indirect" || first_token == "return_call_indirect" {
            check_call_indirect_table(node, node, source, symbols, diagnostics);
        }
        return;
    }

    // Also check folded form
    if kind == "expr1_plain" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            node_kind!(ck = child);
            if ck == "instr_plain" {
                let text = node_text(&child, source);
                let first_token = text.split_whitespace().next().unwrap_or("");
                if first_token == "call_indirect" || first_token == "return_call_indirect" {
                    check_call_indirect_table(&child, node, source, symbols, diagnostics);
                }
            }
        }
        return;
    }

    // Also handle expr1_call and instr_call nodes (grammar-specific for call_indirect)
    if kind == "expr1_call" || kind == "instr_call" || kind == "instr_list_call" {
        let text = node_text(node, source);
        let first_token = text.split_whitespace().next().unwrap_or("");
        if first_token == "call_indirect" || first_token == "return_call_indirect" {
            check_call_indirect_table(node, node, source, symbols, diagnostics);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_call_indirect_checks(&child, source, symbols, diagnostics);
    }
}

/// Check inline elem segments for unknown function references.
/// e.g., (table funcref (elem 0 0)) with no functions defined → "unknown function 0"
fn check_inline_elem_func_refs(
    module: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_module_field(module, |field| {
        node_kind!(fk = field);
        if fk != "module_field_table" {
            return;
        }

        // Look for inline elem segments: table_fields_elem → (elem index*)
        let mut cursor = field.walk();
        for child in field.children(&mut cursor) {
            node_kind!(ck = child);
            if ck == "table_fields_elem" {
                check_inline_table_elem_refs(&child, source, symbols, diagnostics);
            }
        }
    });
}

fn check_inline_table_elem_refs(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(ck = child);
        if ck == "nat" || ck == "index" {
            let text = node_text(&child, source).trim().to_string();
            if let Ok(idx) = text.parse::<usize>() {
                if idx >= symbols.functions.len() {
                    diagnostics.push(
                        Diagnostic::error(
                            node_to_range(&child),
                            format!("unknown function {}", idx),
                        )
                        .with_code("unknown-function"),
                    );
                }
            }
        }
        // Also recurse to handle nested structures
        if ck == "elem_list" || ck == "elem_expr" || ck == "elem_expr_item" {
            check_inline_table_elem_refs(&child, source, symbols, diagnostics);
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
    fn test_global_get_const_earlier_global_ok() {
        // global.get of earlier immutable module-defined global is valid
        let source = r#"(module
            (global $g i32 (i32.const 0))
            (global $h i32 (global.get $g))
        )"#;
        let diags = get_diagnostics(source);
        assert!(
            diags.is_empty(),
            "Expected no errors for global.get of earlier global, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_global_get_const_forward_ref_error() {
        // global.get of later-defined global is an error (forward reference)
        let source = r#"(module
            (global $g1 i32 (global.get $g2))
            (global $g2 i32 (i32.const 0))
        )"#;
        let diags = get_diagnostics(source);
        assert!(
            diags.iter().any(|d| d.message.contains("unknown global")),
            "Expected 'unknown global' for forward ref, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_table_init_non_imported_global_error() {
        // Table init can only use imported globals
        let source = r#"(module
            (global $g funcref (ref.null func))
            (table $t 10 funcref (global.get $g))
        )"#;
        let diags = get_diagnostics(source);
        assert!(
            diags.iter().any(|d| d.message.contains("unknown global")),
            "Expected 'unknown global' for non-imported global in table init, got: {:?}",
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
