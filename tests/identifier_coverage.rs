//! Identifier coverage test — verifies every `$name` identifier in playground examples resolves.
//!
//! Walks all `.wat` files in `packages/playground/examples/` (skipping `invalid/`),
//! collects every `identifier` node whose text starts with `$`, and asserts that
//! `provide_definition_core()` returns `Some` for each one.

#![cfg(feature = "native")]

use std::path::Path;
use tree_sitter::{Node, Tree};
use wat_lsp_rust::{
    core::types::Position,
    features::definition_core::provide_definition_core,
    parser::{parse_document, parse_modules_from_tree, ModuleInfo},
    tree_sitter_bindings::create_parser,
};

// =============================================================================
// Expected failures — known limitations documented by category
// =============================================================================

/// Files to skip entirely. These use GC or typed-funcref features extensively,
/// and the LSP doesn't yet support struct field resolution or type references
/// in all GC/typed-funcref-specific contexts (ref types, struct.get/set field
/// operands, array type operands, call_ref type operands, etc.).
const SKIP_FILES: &[&str] = &[
    "gc_structs.wat",
    "gc_advanced.wat",
    "typed_funcref.wat",
    "call_ref_type_annotation.wat",
];

/// Individual expected failures: `(filename, identifier, line_0indexed)`.
/// These are specific context-detection gaps outside the GC/typed-funcref files.
const EXPECTED_FAILURES: &[(&str, &str, u32)] = &[
    // Table operand in call_indirect / return_call_indirect:
    // context detection sees instr_call/expr1_call and returns Call instead of Table.
    ("imports_exports.wat", "$callbacks", 77),
    ("imports_exports.wat", "$local_fns", 141),
    ("ref_expressions.wat", "$ops", 60),
    ("reference_types.wat", "$func_table", 89),
    ("reference_types.wat", "$func_table", 106),
    ("reference_types.wat", "$func_table", 137),
    ("table64.wat", "$large_table", 64),
    ("tables.wat", "$unary_ops", 59),
    ("tables.wat", "$binary_ops", 67),
    ("tables.wat", "$unary_ops", 85),
    ("tables.wat", "$binary_ops", 106),
    ("tables.wat", "$unary_ops", 152),
    ("tables.wat", "$unary_ops", 153),
    ("tail_calls.wat", "$ops", 98),
    // Multi-module example: table operand in call_indirect (same as above)
    ("multi_module_types.wat", "$fns", 17),
];

// =============================================================================
// Test infrastructure
// =============================================================================

/// An identifier that failed to resolve.
#[derive(Debug)]
struct Failure {
    file: String,
    line: u32,
    column: u32,
    identifier: String,
}

/// Load all `.wat` files from `packages/playground/examples/`, skipping `invalid/`.
fn load_playground_examples() -> Vec<(String, String)> {
    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages/playground/examples");
    assert!(
        examples_dir.exists(),
        "Playground examples directory not found: {}",
        examples_dir.display()
    );

    let mut files: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&examples_dir).expect("Failed to read examples directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        // Skip subdirectories (except wast/)
        if path.is_dir() {
            // Also load .wat files from the wast/ subdirectory (multi-module examples)
            if path.file_name().and_then(|n| n.to_str()) == Some("wast") {
                for wast_entry in std::fs::read_dir(&path).expect("Failed to read wast directory") {
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

/// Parse a document into a tree-sitter tree.
fn parse_tree(document: &str) -> Tree {
    let mut parser = create_parser();
    parser
        .parse(document, None)
        .expect("Failed to parse document")
}

/// Collect all `identifier` nodes starting with `$` from the tree.
/// Returns `(Position, identifier_text)` pairs.
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

fn is_expected_failure(filename: &str, identifier: &str, line: u32) -> bool {
    EXPECTED_FAILURES
        .iter()
        .any(|(f, id, l)| *f == filename && *id == identifier && *l == line)
}

fn is_skipped_file(filename: &str) -> bool {
    SKIP_FILES.contains(&filename)
}

#[test]
fn test_all_identifiers_resolve_to_definition() {
    let examples = load_playground_examples();
    assert!(
        !examples.is_empty(),
        "No playground examples found — check the examples directory"
    );

    let mut failures: Vec<Failure> = Vec::new();
    let mut total_identifiers = 0u32;
    let mut total_files = 0u32;
    let mut skipped_files = 0u32;
    let mut expected_failure_count = 0u32;

    for (filename, content) in &examples {
        if is_skipped_file(filename) {
            skipped_files += 1;
            continue;
        }

        let tree = parse_tree(content);

        // Parse into per-module symbol tables (multi-module aware)
        let modules: Vec<ModuleInfo> = match parse_modules_from_tree(&tree, content) {
            Ok(m) => m,
            Err(_) => match parse_document(content) {
                Ok(s) => vec![ModuleInfo {
                    range: wat_lsp_rust::core::types::Range::default(),
                    symbols: s,
                }],
                Err(_) => continue,
            },
        };

        let identifiers = collect_identifiers(&tree, content);

        if identifiers.is_empty() {
            continue;
        }

        total_files += 1;
        total_identifiers += identifiers.len() as u32;

        for (position, ident_text) in &identifiers {
            // Find the module containing this identifier's position
            let symbols = if modules.len() > 1 {
                modules
                    .iter()
                    .find(|m| {
                        let s = &m.range.start;
                        let e = &m.range.end;
                        (position.line > s.line
                            || (position.line == s.line && position.character >= s.character))
                            && (position.line < e.line
                                || (position.line == e.line && position.character <= e.character))
                    })
                    .map(|m| &m.symbols)
                    .unwrap_or(&modules[0].symbols)
            } else {
                &modules[0].symbols
            };
            let result = provide_definition_core(content, symbols, &tree, *position);

            if result.is_none() {
                if is_expected_failure(filename, ident_text, position.line) {
                    expected_failure_count += 1;
                    continue;
                }
                failures.push(Failure {
                    file: filename.clone(),
                    line: position.line,
                    column: position.character,
                    identifier: ident_text.clone(),
                });
            }
        }
    }

    if !failures.is_empty() {
        let mut msg = format!(
            "\n{} identifier(s) failed to resolve out of {} total across {} files:\n\n",
            failures.len(),
            total_identifiers,
            total_files,
        );
        for f in &failures {
            msg.push_str(&format!(
                "  {}:{}:{} — {}\n",
                f.file,
                f.line + 1,
                f.column + 1,
                f.identifier,
            ));
        }
        panic!("{}", msg);
    }

    // Print summary on success (visible with --nocapture)
    eprintln!(
        "Identifier coverage: {} identifiers across {} files — all resolved \
         ({} expected failures, {} files skipped for GC/typed-funcref).",
        total_identifiers, total_files, expected_failure_count, skipped_files,
    );
}
