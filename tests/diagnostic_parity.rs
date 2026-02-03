//! Diagnostic Parity Tests (native only)
//!
//! This module tests the native diagnostic implementation against the diagnostic
//! corpus to ensure parity with the WASM implementation. The same corpus is used
//! by the WASM/Playwright tests to verify both implementations produce the same
//! results.
//!
//! Run with: `cargo test diagnostic_parity`

#![cfg(feature = "native")]

use serde::Deserialize;
use std::fs;
use std::path::Path;
use tower_lsp::lsp_types::Diagnostic;
use wat_lsp_rust::diagnostics::{
    merge_all_diagnostics, provide_semantic_diagnostics, provide_tree_sitter_diagnostics,
    validate_wat,
};
use wat_lsp_rust::parser;
use wat_lsp_rust::ts_facade;

#[derive(Debug, Deserialize)]
struct ExpectedDiagnostic {
    line: u32,
    #[serde(default)]
    message_contains: Option<String>,
    #[serde(default)]
    message_contains_all: Option<Vec<String>>,
    severity: u32,
}

#[derive(Debug, Deserialize)]
struct ExpectedFile {
    description: String,
    diagnostics: Vec<ExpectedDiagnostic>,
}

fn get_all_diagnostics(wat: &str) -> Vec<Diagnostic> {
    let mut parser = ts_facade::create_parser();
    let tree = parser.parse(wat, None).expect("Failed to parse");
    let symbols = parser::parse_document_from_tree(&tree, wat).unwrap_or_default();

    let tree_sitter_diags = provide_tree_sitter_diagnostics(&tree, wat);
    let semantic_diags = provide_semantic_diagnostics(&tree, wat, &symbols);
    let wast_diags = validate_wat(wat);

    merge_all_diagnostics(tree_sitter_diags, semantic_diags, wast_diags)
}

fn check_diagnostic_matches(actual: &Diagnostic, expected: &ExpectedDiagnostic) -> bool {
    // Check line number
    if actual.range.start.line != expected.line {
        return false;
    }

    // Check severity (1 = error, 2 = warning, 3 = info, 4 = hint)
    let actual_severity = match actual.severity {
        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR) => 1,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING) => 2,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION) => 3,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT) => 4,
        _ => return false,
    };
    if actual_severity != expected.severity {
        return false;
    }

    // Check message contains
    if let Some(ref contains) = expected.message_contains {
        if !actual.message.contains(contains) {
            return false;
        }
    }

    // Check message contains all
    if let Some(ref contains_all) = expected.message_contains_all {
        for s in contains_all {
            if !actual.message.contains(s) {
                return false;
            }
        }
    }

    true
}

fn run_corpus_test(wat_path: &Path) {
    let expected_path = wat_path.with_extension("expected.json");

    // Read WAT file
    let wat_content = fs::read_to_string(wat_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", wat_path.display(), e));

    // Read expected file
    let expected_content = fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", expected_path.display(), e));

    let expected: ExpectedFile = serde_json::from_str(&expected_content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", expected_path.display(), e));

    // Get actual diagnostics
    let actual_diagnostics = get_all_diagnostics(&wat_content);

    // Verify each expected diagnostic is found
    for (i, exp) in expected.diagnostics.iter().enumerate() {
        let found = actual_diagnostics
            .iter()
            .any(|actual| check_diagnostic_matches(actual, exp));

        if !found {
            let actual_on_line: Vec<_> = actual_diagnostics
                .iter()
                .filter(|d| d.range.start.line == exp.line)
                .map(|d| format!("  - {}", d.message))
                .collect();

            panic!(
                "PARITY FAILURE in {}\n\
                 Description: {}\n\
                 Expected diagnostic #{} not found:\n\
                   Line: {}\n\
                   Message should contain: {:?}\n\
                   Message should contain all: {:?}\n\
                   Severity: {}\n\
                 Actual diagnostics on line {}:\n{}\n\
                 All actual diagnostics:\n{}",
                wat_path.display(),
                expected.description,
                i + 1,
                exp.line,
                exp.message_contains,
                exp.message_contains_all,
                exp.severity,
                exp.line,
                if actual_on_line.is_empty() {
                    "  (none)".to_string()
                } else {
                    actual_on_line.join("\n")
                },
                actual_diagnostics
                    .iter()
                    .map(|d| format!("  L{}: {}", d.range.start.line, d.message))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }

    // Check for unexpected diagnostics (errors only, not warnings/hints)
    let unexpected: Vec<_> = actual_diagnostics
        .iter()
        .filter(|d| {
            d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
                && !expected
                    .diagnostics
                    .iter()
                    .any(|exp| check_diagnostic_matches(d, exp))
        })
        .collect();

    if !unexpected.is_empty() && !expected.diagnostics.is_empty() {
        // Only warn about unexpected errors if we expected some diagnostics
        // (for valid files, we want exactly 0 errors)
        let unexpected_msgs: Vec<_> = unexpected
            .iter()
            .map(|d| format!("  L{}: {}", d.range.start.line, d.message))
            .collect();

        // For valid_no_errors test, this is a failure
        if expected.diagnostics.is_empty() && !unexpected.is_empty() {
            panic!(
                "PARITY FAILURE in {}\n\
                 Description: {}\n\
                 Expected no errors but found:\n{}",
                wat_path.display(),
                expected.description,
                unexpected_msgs.join("\n")
            );
        }
    }

    // Special case: if we expect no diagnostics, verify we got none
    if expected.diagnostics.is_empty() {
        let errors: Vec<_> = actual_diagnostics
            .iter()
            .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
            .collect();

        if !errors.is_empty() {
            panic!(
                "PARITY FAILURE in {}\n\
                 Description: {}\n\
                 Expected no errors but found {} error(s):\n{}",
                wat_path.display(),
                expected.description,
                errors.len(),
                errors
                    .iter()
                    .map(|d| format!("  L{}: {}", d.range.start.line, d.message))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

#[test]
fn diagnostic_parity_corpus() {
    let corpus_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diagnostic_corpus");

    if !corpus_dir.exists() {
        panic!(
            "Diagnostic corpus directory not found: {}",
            corpus_dir.display()
        );
    }

    let mut test_count = 0;
    let mut errors = Vec::new();

    for entry in fs::read_dir(&corpus_dir).expect("Failed to read corpus directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "wat") {
            let expected_path = path.with_extension("expected.json");
            if expected_path.exists() {
                test_count += 1;
                println!("Testing: {}", path.file_name().unwrap().to_string_lossy());

                // Run test and collect any error
                let result = std::panic::catch_unwind(|| {
                    run_corpus_test(&path);
                });

                if let Err(e) = result {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown error".to_string()
                    };
                    errors.push((path.clone(), msg));
                }
            } else {
                eprintln!(
                    "Warning: {} has no expected.json file, skipping",
                    path.display()
                );
            }
        }
    }

    if !errors.is_empty() {
        let error_report = errors
            .iter()
            .map(|(path, msg)| format!("\n=== {} ===\n{}", path.display(), msg))
            .collect::<Vec<_>>()
            .join("\n");

        panic!(
            "\n{} of {} diagnostic parity tests failed:\n{}",
            errors.len(),
            test_count,
            error_report
        );
    }

    println!("\nAll {} diagnostic parity tests passed!", test_count);
}
