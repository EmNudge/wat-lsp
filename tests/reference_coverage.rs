//! Reference coverage test — validates go-to-definition and go-to-references
//! work correctly across all playground examples, including multi-module WAST files.
//!
//! For every `$name` identifier in playground examples:
//! 1. If go-to-definition succeeds, go-to-references (include_declaration=true) should
//!    return at least 1 result (the declaration itself).
//! 2. All returned reference ranges should fall within the same module as the query position
//!    (no cross-module leaks in multi-module files).

#![cfg(feature = "native")]

use std::path::Path;
use tree_sitter::{Node, Tree};
use wat_lsp_rust::{
    core::types::{Position, Range},
    features::{
        definition_core::provide_definition_core, references_core::provide_references_core_scoped,
    },
    parser::{parse_modules_from_tree, ModuleInfo},
    tree_sitter_bindings::create_parser,
};

// =============================================================================
// Known limitations — table operands in call_indirect don't resolve
// =============================================================================

/// Files to skip entirely (GC/typed-funcref not fully supported).
const SKIP_FILES: &[&str] = &[
    "gc_structs.wat",
    "gc_advanced.wat",
    "typed_funcref.wat",
    "call_ref_type_annotation.wat",
];

// =============================================================================
// Test infrastructure
// =============================================================================

/// Load all `.wat` files from playground examples, including wast/ subdirectory.
fn load_playground_examples() -> Vec<(String, String)> {
    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages/playground/examples");
    assert!(examples_dir.exists());

    let mut files: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&examples_dir).expect("Failed to read examples directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.is_dir() {
            // Include wast/ subdirectory
            if path.file_name().and_then(|n| n.to_str()) == Some("wast") {
                for wast_entry in std::fs::read_dir(&path).expect("Failed to read wast dir") {
                    let wast_entry = wast_entry.expect("Failed to read wast entry");
                    let wast_path = wast_entry.path();
                    if wast_path.extension().and_then(|e| e.to_str()) == Some("wat") {
                        let filename = wast_path.file_name().unwrap().to_str().unwrap().to_string();
                        let content = std::fs::read_to_string(&wast_path).unwrap_or_else(|e| {
                            panic!("Failed to read {}: {}", wast_path.display(), e)
                        });
                        files.push((filename, content));
                    }
                }
            }
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) == Some("wat") {
            let filename = path.file_name().unwrap().to_str().unwrap().to_string();
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
            files.push((filename, content));
        }
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn parse_tree(document: &str) -> Tree {
    let mut parser = create_parser();
    parser
        .parse(document, None)
        .expect("Failed to parse document")
}

/// Collect all `identifier` nodes starting with `$`.
fn collect_identifiers(tree: &Tree, document: &str) -> Vec<(Position, String)> {
    let mut results = Vec::new();
    collect_identifiers_recursive(tree.root_node(), document, &mut results);
    results
}

fn collect_identifiers_recursive(
    node: Node,
    document: &str,
    results: &mut Vec<(Position, String)>,
) {
    if node.kind() == "identifier" {
        let text = &document[node.byte_range()];
        if text.starts_with('$') {
            let start = node.start_position();
            results.push((
                Position::new(start.row as u32, start.column as u32),
                text.to_string(),
            ));
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifiers_recursive(child, document, results);
    }
}

/// Find which module contains a position.
fn find_module_for_position<'a>(
    modules: &'a [ModuleInfo],
    pos: &Position,
) -> Option<&'a ModuleInfo> {
    for module in modules {
        let s = &module.range.start;
        let e = &module.range.end;
        if (pos.line > s.line || (pos.line == s.line && pos.character >= s.character))
            && (pos.line < e.line || (pos.line == e.line && pos.character <= e.character))
        {
            return Some(module);
        }
    }
    modules.first()
}

/// Check if a range falls within a module's range.
fn is_within_module(range: &Range, module: &ModuleInfo) -> bool {
    let s = &module.range.start;
    let e = &module.range.end;
    (range.start.line > s.line
        || (range.start.line == s.line && range.start.character >= s.character))
        && (range.end.line < e.line
            || (range.end.line == e.line && range.end.character <= e.character))
}

fn is_skipped_file(filename: &str) -> bool {
    SKIP_FILES.contains(&filename)
}

// =============================================================================
// Main test: for every identifier where definition resolves, references must
// (1) return ≥1 result, and (2) all results must be within the same module.
// =============================================================================

#[test]
fn test_references_consistency_with_definitions() {
    let examples = load_playground_examples();
    assert!(!examples.is_empty());

    let mut ref_empty_failures: Vec<String> = Vec::new();
    let mut cross_module_failures: Vec<String> = Vec::new();
    let mut total_checked = 0u32;
    let mut total_files = 0u32;

    for (filename, content) in &examples {
        if is_skipped_file(filename) {
            continue;
        }

        let tree = parse_tree(content);
        let modules = match parse_modules_from_tree(&tree, content) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let identifiers = collect_identifiers(&tree, content);
        if identifiers.is_empty() {
            continue;
        }

        total_files += 1;
        let is_multi_module = modules.len() > 1;

        for (position, ident_text) in &identifiers {
            let module = match find_module_for_position(&modules, position) {
                Some(m) => m,
                None => continue,
            };

            // Only check identifiers where go-to-definition succeeds
            let def_result = provide_definition_core(content, &module.symbols, &tree, *position);
            if def_result.is_none() {
                continue;
            }

            total_checked += 1;

            // Get module scope for multi-module documents
            let module_scope = if is_multi_module {
                Some(module.range)
            } else {
                None
            };

            // Get references (include declaration)
            let refs = provide_references_core_scoped(
                content,
                &module.symbols,
                &tree,
                *position,
                true,
                module_scope,
            );

            // Heuristic 1: If definition resolves, references should return ≥1 result
            if refs.is_empty() {
                ref_empty_failures.push(format!(
                    "{}:{}:{} — {} (definition found but 0 references)",
                    filename,
                    position.line + 1,
                    position.character + 1,
                    ident_text
                ));
            }

            // Heuristic 2: All references must be within the same module (multi-module only)
            if is_multi_module {
                for ref_range in &refs {
                    if !is_within_module(ref_range, module) {
                        cross_module_failures.push(format!(
                            "{}:{}:{} — {} has reference at {}:{} outside its module",
                            filename,
                            position.line + 1,
                            position.character + 1,
                            ident_text,
                            ref_range.start.line + 1,
                            ref_range.start.character + 1,
                        ));
                    }
                }
            }
        }
    }

    // STRICT: Cross-module reference leaks must be zero
    if !cross_module_failures.is_empty() {
        let mut msg = format!(
            "\n{} reference(s) leak across module boundaries:\n",
            cross_module_failures.len()
        );
        for f in &cross_module_failures {
            msg.push_str(&format!("  {}\n", f));
        }
        panic!("{}", msg);
    }

    // HEURISTIC: Definition-without-references is a known limitation for certain
    // contexts (block labels at definition only, elem/data segment refs, etc.).
    // We track the ratio and fail if it regresses significantly.
    let ref_empty_count = ref_empty_failures.len() as u32;
    let ref_coverage_pct = if total_checked > 0 {
        ((total_checked - ref_empty_count) as f64 / total_checked as f64) * 100.0
    } else {
        100.0
    };

    eprintln!(
        "Reference coverage: {}/{} identifiers ({:.1}%) have refs across {} files",
        total_checked - ref_empty_count,
        total_checked,
        ref_coverage_pct,
        total_files,
    );
    if !ref_empty_failures.is_empty() {
        eprintln!(
            "  {} identifiers with definition but no references (known gap)",
            ref_empty_count,
        );
    }

    // Fail if reference coverage drops below 85% — a regression threshold.
    // Current baseline is ~93% (174 gaps out of ~2500 checked).
    assert!(
        ref_coverage_pct >= 85.0,
        "Reference coverage dropped to {:.1}% (threshold: 85%). {} gaps out of {} checked.\nFirst 10 gaps:\n{}",
        ref_coverage_pct,
        ref_empty_count,
        total_checked,
        ref_empty_failures.iter().take(10).map(|f| format!("  {}\n", f)).collect::<String>(),
    );
}
