//! E2E parity tests — run shared JSON fixtures against the native core.
//!
//! These same fixtures are also run by the WASM test suite in
//! `packages/wat-lsp/tests/parity.test.mjs`, ensuring both builds
//! produce equivalent results.

#![cfg(feature = "native")]

use serde_json::Value;
use std::path::PathBuf;
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;
use wat_lsp_rust::{
    completion, definition, diagnostics, document_symbols, features::references, folding, hover,
    parser, symbols::SymbolTable, tree_sitter_bindings::create_parser,
};

// ───────────────────────── Helpers ─────────────────────────

fn parse_doc(text: &str) -> (Tree, SymbolTable) {
    let mut parser = create_parser();
    let tree = parser.parse(text, None).expect("parse failed");
    let symbols = parser::parse_document(text).unwrap_or_default();
    (tree, symbols)
}

fn get_diagnostics(text: &str) -> Vec<Diagnostic> {
    let (tree, symbols) = parse_doc(text);
    let ts_diags = diagnostics::provide_tree_sitter_diagnostics(&tree, text);
    let sem_diags = diagnostics::provide_semantic_diagnostics(&tree, text, &symbols);
    let mut all = ts_diags;
    all.extend(sem_diags);
    all
}

fn load_fixtures() -> Vec<(String, Value)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/e2e_fixtures");
    let mut fixtures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("cannot read e2e_fixtures dir") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let content = std::fs::read_to_string(&path).unwrap();
            let val: Value = serde_json::from_str(&content).unwrap();
            let name = path.file_stem().unwrap().to_str().unwrap().to_string();
            fixtures.push((name, val));
        }
    }
    fixtures.sort_by(|a, b| a.0.cmp(&b.0));
    fixtures
}

// ───────────────────────── Runner ─────────────────────────

fn run_fixture(name: &str, fixture: &Value) {
    let _description = fixture["description"].as_str().unwrap_or(name);
    let document = fixture["document"]
        .as_str()
        .expect("fixture needs 'document'");
    let tests = fixture["tests"].as_array().expect("fixture needs 'tests'");

    for (i, test) in tests.iter().enumerate() {
        let feature = test["feature"].as_str().expect("test needs 'feature'");
        let expect = &test["expect"];
        let test_label = format!("{name}[{i}]:{feature}");

        // Some tests override the document (e.g. for completion with partial input)
        let doc = test
            .get("document_override")
            .and_then(|v| v.as_str())
            .unwrap_or(document);

        let (tree, symbols) = parse_doc(doc);
        let uri = "file:///test.wat";

        match feature {
            "diagnostics" => {
                let diags = get_diagnostics(doc);
                if let Some(count) = expect.get("error_count").and_then(|v| v.as_u64()) {
                    assert_eq!(
                        diags.len(),
                        count as usize,
                        "{test_label}: expected {count} errors, got {} — {:?}",
                        diags.len(),
                        diags
                    );
                }
                if let Some(min) = expect.get("min_error_count").and_then(|v| v.as_u64()) {
                    assert!(
                        diags.len() >= min as usize,
                        "{test_label}: expected >= {min} errors, got {}",
                        diags.len()
                    );
                }
            }

            "hover" => {
                let [line, col] = pos_from(test);
                let result = hover::provide_hover(doc, &symbols, &tree, Position::new(line, col));
                if expect
                    .get("non_null")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    assert!(result.is_some(), "{test_label}: hover returned None");
                }
                if let Some(substr) = expect.get("contains").and_then(|v| v.as_str()) {
                    let h = result.unwrap_or_else(|| panic!("{test_label}: hover is None"));
                    let text = match &h.contents {
                        HoverContents::Markup(m) => m.value.clone(),
                        HoverContents::Scalar(s) => match s {
                            MarkedString::String(s) => s.clone(),
                            MarkedString::LanguageString(ls) => ls.value.clone(),
                        },
                        HoverContents::Array(a) => a
                            .iter()
                            .map(|m| match m {
                                MarkedString::String(s) => s.clone(),
                                MarkedString::LanguageString(ls) => ls.value.clone(),
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    };
                    assert!(
                        text.contains(substr),
                        "{test_label}: hover should contain '{substr}', got: {text}"
                    );
                }
            }

            "definition" => {
                let [line, col] = pos_from(test);
                let result = definition::provide_definition(
                    doc,
                    &symbols,
                    &tree,
                    Position::new(line, col),
                    uri,
                );
                if let Some(start_line) = expect.get("start_line").and_then(|v| v.as_u64()) {
                    let loc = result.unwrap_or_else(|| panic!("{test_label}: definition is None"));
                    assert_eq!(
                        loc.range.start.line, start_line as u32,
                        "{test_label}: expected definition on line {start_line}"
                    );
                }
            }

            "references" => {
                let [line, col] = pos_from(test);
                let include_decl = test
                    .get("include_declaration")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let refs = references::provide_references(
                    doc,
                    &symbols,
                    &tree,
                    Position::new(line, col),
                    uri,
                    include_decl,
                );
                if let Some(min) = expect.get("min_count").and_then(|v| v.as_u64()) {
                    assert!(
                        refs.len() >= min as usize,
                        "{test_label}: expected >= {min} refs, got {}",
                        refs.len()
                    );
                }
            }

            "document_symbols" => {
                let syms = document_symbols::provide_document_symbols(&symbols);
                if let Some(min) = expect.get("min_count").and_then(|v| v.as_u64()) {
                    assert!(
                        syms.len() >= min as usize,
                        "{test_label}: expected >= {min} symbols, got {}",
                        syms.len()
                    );
                }
                if let Some(name) = expect.get("contains_name").and_then(|v| v.as_str()) {
                    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
                    assert!(
                        names.iter().any(|n| n.contains(name)),
                        "{test_label}: should contain '{name}', got: {names:?}"
                    );
                }
            }

            "completion" => {
                let [line, col] = pos_from(test);
                let completions =
                    completion::provide_completion(doc, &symbols, Position::new(line, col).into());
                if expect
                    .get("non_empty")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    assert!(
                        !completions.is_empty(),
                        "{test_label}: expected non-empty completions"
                    );
                }
                if let Some(label) = expect.get("contains_label").and_then(|v| v.as_str()) {
                    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
                    assert!(
                        labels.iter().any(|l| l.contains(label)),
                        "{test_label}: should contain '{label}', got: {labels:?}"
                    );
                }
            }

            "folding_ranges" => {
                let ranges = folding::provide_folding_ranges_lsp(doc, &symbols, &tree);
                if expect
                    .get("non_empty")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    assert!(
                        !ranges.is_empty(),
                        "{test_label}: expected non-empty folding ranges"
                    );
                }
            }

            "rename" => {
                let [line, col] = pos_from(test);
                let new_name = test["new_name"]
                    .as_str()
                    .expect("rename test needs new_name");
                let refs = references::provide_references(
                    doc,
                    &symbols,
                    &tree,
                    Position::new(line, col),
                    uri,
                    true,
                );
                if let Some(min) = expect.get("min_edit_count").and_then(|v| v.as_u64()) {
                    assert!(
                        refs.len() >= min as usize,
                        "{test_label}: expected >= {min} rename edits, got {}",
                        refs.len()
                    );
                }
                if let Some(expected_text) = expect.get("new_text").and_then(|v| v.as_str()) {
                    assert_eq!(
                        new_name, expected_text,
                        "{test_label}: new_text should match new_name"
                    );
                }
            }

            other => panic!("{test_label}: unknown feature '{other}'"),
        }
    }
}

fn pos_from(test: &Value) -> [u32; 2] {
    let pos = test["position"].as_array().expect("test needs 'position'");
    [
        pos[0].as_u64().unwrap() as u32,
        pos[1].as_u64().unwrap() as u32,
    ]
}

// ───────────────────────── Test entry ─────────────────────────

#[test]
fn parity_fixtures() {
    let fixtures = load_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no fixtures found in tests/e2e_fixtures/"
    );

    for (name, fixture) in &fixtures {
        run_fixture(name, fixture);
    }
}
