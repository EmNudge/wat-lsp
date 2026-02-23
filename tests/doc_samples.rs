//! Doc Sample Validation Tests (native only)
//!
//! Validates all WAT code samples in the documentation site against the native
//! diagnostic pipeline. Supports hidden context lines (prefixed with `# `) that
//! are included during validation but stripped during rendering — following the
//! Rust doc-test convention.
//!
//! Run with: `cargo test doc_samples --features native`

#![cfg(feature = "native")]

use std::fs;
use std::path::Path;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use wat_lsp_rust::diagnostics::{
    merge_all_diagnostics, provide_semantic_diagnostics, provide_tree_sitter_diagnostics,
    validate_wat,
};
use wat_lsp_rust::parser;
use wat_lsp_rust::ts_facade;

/// Known LSP false positives — valid WAT that the LSP incorrectly flags due to
/// incomplete type checking or missing proposal support.
/// Format: "relative/path.md:line_number"
const KNOWN_ISSUES: &[&str] = &[
    // br_if/br_table stack validation in typed blocks (type checker limitation)
    "control/block.md:11",
    "control/br-table.md:11",
    "control/branching.md:11",
    // try_table type checking not fully supported
    "extensions/exception-handling.md:11",
    // call_ref syntax: `(call_ref (type $t))` vs `(call_ref $t)`
    "extensions/typed-func-refs.md:11",
    // ref.null funcref syntax difference
    "language/reference-types.md:18",
    // Inline import after inline export (import ordering with inline syntax)
    "language/text-format.md:160",
    // try_table catch clause: type checker doesn't track values delivered via branch
    "instructions/exceptions.md:78",
    // i64 atomic RMW operations: type checker returns i32 instead of i64
    "instructions/atomic.md:397",
    "instructions/atomic.md:416",
    "instructions/atomic.md:435",
    "instructions/atomic.md:454",
    "instructions/atomic.md:473",
];

fn is_known_issue(rel_path: &Path, line_number: usize) -> bool {
    // Normalize Windows backslashes to forward slashes for cross-platform matching
    let key = format!(
        "{}:{}",
        rel_path.display().to_string().replace('\\', "/"),
        line_number
    );
    KNOWN_ISSUES.contains(&key.as_str())
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

/// A WAT code block extracted from a markdown file.
struct WatBlock {
    /// The assembled WAT source (hidden lines included, prefix stripped).
    code: String,
    /// 1-indexed line number where the code block starts in the markdown file.
    line_number: usize,
}

/// Extract all ```wat code blocks from markdown content.
/// Processes `# ` hidden lines (includes them with prefix stripped).
/// Skips ```wat-snippet blocks.
fn extract_wat_blocks(markdown: &str) -> Vec<WatBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = markdown.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        // Match ```wat but not ```wat-snippet or other variants
        if line.starts_with("```wat") && !line.starts_with("```wat-") {
            let block_start = i + 1; // 0-indexed line after the fence
            let mut block_lines = Vec::new();
            i += 1;

            while i < lines.len() && !lines[i].starts_with("```") {
                let content_line = lines[i];
                if let Some(stripped) = content_line.strip_prefix("# ") {
                    block_lines.push(stripped.to_string());
                } else if content_line == "#" {
                    // Bare `#` is a hidden empty line
                    block_lines.push(String::new());
                } else {
                    block_lines.push(content_line.to_string());
                }
                i += 1;
            }

            blocks.push(WatBlock {
                code: block_lines.join("\n"),
                line_number: block_start + 1, // 1-indexed
            });
        }
        i += 1;
    }

    blocks
}

/// Recursively collect all .md files under a directory.
fn collect_md_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_md_files(&path, &mut *files);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "md" || ext == "mdx")
            {
                files.push(path);
            }
        }
    }
}

#[test]
fn doc_samples_have_no_errors() {
    let docs_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages")
        .join("docs")
        .join("src")
        .join("content")
        .join("docs");

    if !docs_dir.exists() {
        eprintln!("Docs directory not found: {}", docs_dir.display());
        return;
    }

    let mut md_files = Vec::new();
    collect_md_files(&docs_dir, &mut md_files);
    md_files.sort();

    let mut failures: Vec<String> = Vec::new();
    let mut total_blocks = 0;
    let mut skipped_known = 0;

    for md_path in &md_files {
        let rel_path = md_path.strip_prefix(&docs_dir).unwrap_or(md_path);
        let markdown = fs::read_to_string(md_path).expect("Failed to read markdown file");
        let blocks = extract_wat_blocks(&markdown);

        for block in &blocks {
            // Every `wat` block must have module context (visible or hidden).
            // Use ```wat-snippet for syntax-only demos that don't need validation.
            assert!(
                block.code.contains("(module"),
                "Unannotated wat block at {}:{} — add hidden `# (module` context or use ```wat-snippet\n  Code:\n{}",
                rel_path.display(),
                block.line_number,
                block.code.lines().enumerate()
                    .map(|(i, l)| format!("    {}: {}", i + 1, l))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );

            total_blocks += 1;

            if is_known_issue(rel_path, block.line_number) {
                skipped_known += 1;
                continue;
            }

            let diagnostics = get_all_diagnostics(&block.code);
            let errors: Vec<&Diagnostic> = diagnostics
                .iter()
                .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
                .collect();

            if !errors.is_empty() {
                let error_msgs: Vec<String> = errors
                    .iter()
                    .map(|d| format!("  L{}: {}", d.range.start.line + 1, d.message))
                    .collect();

                failures.push(format!(
                    "{}:{} — {} error(s):\n{}\n  Code:\n{}",
                    rel_path.display(),
                    block.line_number,
                    errors.len(),
                    error_msgs.join("\n"),
                    block
                        .code
                        .lines()
                        .enumerate()
                        .map(|(i, l)| format!("    {}: {}", i + 1, l))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ));
            }
        }
    }

    eprintln!(
        "Doc samples: {} blocks validated, {} known issues skipped, {} failures",
        total_blocks,
        skipped_known,
        failures.len()
    );

    if !failures.is_empty() {
        panic!(
            "\n{} doc sample(s) have errors:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
