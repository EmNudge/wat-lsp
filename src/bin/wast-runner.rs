//! wast-runner: Run WebAssembly spec test suite (.wast files) against the LSP diagnostic pipeline
//!
//! Parses .wast files, extracts module definitions and assertions, runs the full
//! diagnostic pipeline on each module, and reports how many pass/fail/skip.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use tower_lsp::lsp_types::DiagnosticSeverity;

use wat_lsp_rust::diagnostics::{
    merge_all_diagnostics, provide_semantic_diagnostics, provide_tree_sitter_diagnostics,
    validate_wat,
};
use wat_lsp_rust::parser::parse_document;
use wat_lsp_rust::tree_sitter_bindings::create_parser;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum OutputFormat {
    #[default]
    Text,
    Json,
}

/// Run WebAssembly spec test suite (.wast) files against the LSP diagnostic pipeline
#[derive(Parser, Debug)]
#[command(name = "wast-runner")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to .wast file(s) or directories
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Only show FAIL lines (hide PASS and SKIP)
    #[arg(long)]
    failures_only: bool,

    /// Show full diagnostic details on failures
    #[arg(short, long)]
    verbose: bool,

    /// Filter to .wast files whose name contains this substring
    #[arg(long)]
    filter: Option<String>,
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Pass,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum DirectiveKind {
    Module,
    AssertInvalid,
    AssertMalformed,
    Skip,
}

#[derive(Debug, serde::Serialize)]
struct DirectiveResult {
    line: usize,
    kind: DirectiveKind,
    outcome: Outcome,
    label: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<String>,
}

#[derive(Debug, Default, serde::Serialize)]
struct CategoryStats {
    pass: usize,
    total: usize,
}

#[derive(Debug, serde::Serialize)]
struct FileSummary {
    file: String,
    directives: Vec<DirectiveResult>,
    pass: usize,
    fail: usize,
    skip: usize,
    modules: CategoryStats,
    assert_invalid: CategoryStats,
    assert_malformed: CategoryStats,
}

#[derive(Debug, Default, serde::Serialize)]
struct GlobalSummary {
    files_processed: usize,
    files_parse_errors: usize,
    total: usize,
    pass: usize,
    fail: usize,
    skip: usize,
    modules: CategoryStats,
    assert_invalid: CategoryStats,
    assert_malformed: CategoryStats,
}

// ---------------------------------------------------------------------------
// WAT text extraction from QuoteWat
// ---------------------------------------------------------------------------

/// Find the matching close paren starting from an offset that points at or before '('.
/// Skips block comments `(; ... ;)` (including nested ones) and line comments `;;`.
fn find_matching_close_paren(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    // Find the opening paren at or after start
    let open = bytes.iter().skip(start).position(|&b| b == b'(')? + start;
    let mut depth = 0i32;
    let mut i = open;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'(' && bytes[i + 1] == b';' {
            // Block comment — skip, handling nesting
            i = skip_block_comment(bytes, i)?;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b';' && bytes[i + 1] == b';' {
            // Line comment — skip to end of line
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        match bytes[i] {
            b'"' => {
                // String literal — skip, handling escape sequences
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1; // skip backslash
                    }
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1; // skip closing '"'
                }
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    None
}

/// Walk backward from end of `text` to find the opening `(` that is NOT part of a block comment.
fn rfind_open_paren_skip_comments(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        // Check for block comment end `;)` — walk backwards to find matching `(;`
        if i > 0 && bytes[i] == b')' && bytes[i - 1] == b';' {
            // Skip this block comment backwards
            let mut depth = 1;
            i -= 2; // Skip `;)`
            while i > 0 && depth > 0 {
                if bytes[i] == b';' && i > 0 && bytes[i - 1] == b'(' {
                    depth -= 1;
                    if depth > 0 {
                        i -= 2;
                    } else {
                        i -= 1; // Move past the `(` of `(;`, then continue search
                    }
                } else if bytes[i] == b')' && i > 0 && bytes[i - 1] == b';' {
                    depth += 1;
                    i -= 2;
                } else {
                    i -= 1;
                }
            }
            continue;
        }
        if bytes[i] == b'(' {
            return Some(i);
        }
    }
    None
}

/// Skip a block comment `(; ... ;)`, handling nested block comments.
/// Returns the position right after the closing `;)`.
fn skip_block_comment(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut i = start + 2; // Skip opening `(;`
    while i + 1 < bytes.len() && depth > 0 {
        if bytes[i] == b'(' && bytes[i + 1] == b';' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b';' && bytes[i + 1] == b')' {
            depth -= 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    if depth == 0 {
        Some(i)
    } else {
        None
    }
}

/// Extract the WAT text for a QuoteWat directive from the original source.
fn extract_wat_text<'a>(qw: &wast::QuoteWat<'a>, source: &'a str) -> Option<String> {
    match qw {
        wast::QuoteWat::Wat(wat) => {
            let span = wat.span();
            let offset = span.offset();
            // span points to 'module' or 'component' keyword — walk back to '('
            // Must skip block comments like `(;comment;)` when searching backwards
            let start = rfind_open_paren_skip_comments(&source[..offset])?;
            let end = find_matching_close_paren(source, start)?;
            Some(source[start..=end].to_string())
        }
        wast::QuoteWat::QuoteModule(_span, parts) => {
            let mut text = String::new();
            for (_part_span, bytes) in parts {
                text.push_str(&String::from_utf8_lossy(bytes));
            }
            Some(text)
        }
        wast::QuoteWat::QuoteComponent(..) => None, // Components not supported
    }
}

/// Returns true if the QuoteWat is a binary module (not text).
fn is_binary_module(qw: &wast::QuoteWat<'_>) -> bool {
    matches!(
        qw,
        wast::QuoteWat::Wat(wast::Wat::Module(m))
            if matches!(m.kind, wast::core::ModuleKind::Binary(_))
    )
}

/// Returns true if the QuoteWat is a component.
fn is_component(qw: &wast::QuoteWat<'_>) -> bool {
    matches!(
        qw,
        wast::QuoteWat::Wat(wast::Wat::Component(_)) | wast::QuoteWat::QuoteComponent(..)
    )
}

// ---------------------------------------------------------------------------
// Diagnostic pipeline
// ---------------------------------------------------------------------------

/// Run the full LSP diagnostic pipeline and return the count of error-level diagnostics
/// plus all diagnostic messages (for verbose output).
fn get_diagnostics(wat_text: &str) -> (usize, Vec<String>) {
    let mut parser = create_parser();
    let tree = match parser.parse(wat_text, None) {
        Some(t) => t,
        None => return (1, vec!["Failed to parse with tree-sitter".to_string()]),
    };

    let syntax_diags = provide_tree_sitter_diagnostics(&tree, wat_text);
    let semantic_diags = match parse_document(wat_text) {
        Ok(symbols) => provide_semantic_diagnostics(&tree, wat_text, &symbols),
        Err(_) => vec![],
    };
    let wast_diags = validate_wat(wat_text);
    let all = merge_all_diagnostics(syntax_diags, semantic_diags, wast_diags);

    let error_count = all
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .count();

    let messages: Vec<String> = all
        .iter()
        .map(|d| {
            let sev = match d.severity {
                Some(DiagnosticSeverity::ERROR) => "error",
                Some(DiagnosticSeverity::WARNING) => "warning",
                Some(DiagnosticSeverity::HINT) => "hint",
                _ => "info",
            };
            format!(
                "  L{}:{}: {}: {}",
                d.range.start.line + 1,
                d.range.start.character + 1,
                sev,
                d.message
            )
        })
        .collect();

    (error_count, messages)
}

// ---------------------------------------------------------------------------
// Directive processing
// ---------------------------------------------------------------------------

/// Strip the WAST-only `definition` keyword from `(module definition ...)` text,
/// producing standard WAT `(module ...)`.
fn strip_module_definition(text: &str) -> String {
    if let Some(pos) = text.find("definition") {
        let before = &text[..pos];
        let after = &text[pos + "definition".len()..];
        format!("{}{}", before, after.trim_start())
    } else {
        text.to_string()
    }
}

fn process_directive(directive: &wast::WastDirective<'_>, source: &str) -> DirectiveResult {
    let span = directive.span();
    let (line, _) = span.linecol_in(source);
    let line_1based = line + 1;

    match directive {
        wast::WastDirective::Module(qw) | wast::WastDirective::ModuleDefinition(qw) => {
            let is_definition = matches!(directive, wast::WastDirective::ModuleDefinition(_));
            if is_binary_module(qw) || is_component(qw) {
                return DirectiveResult {
                    line: line_1based,
                    kind: DirectiveKind::Skip,
                    outcome: Outcome::Skip,
                    label: "Binary/Component module: not applicable".to_string(),
                    diagnostics: vec![],
                };
            }
            match extract_wat_text(qw, source) {
                Some(text) => {
                    // Strip WAST-only `definition` keyword:
                    // `(module definition $M ...)` -> `(module $M ...)`
                    let text = if is_definition {
                        strip_module_definition(&text)
                    } else {
                        text
                    };
                    let (errors, diags) = get_diagnostics(&text);
                    if errors == 0 {
                        DirectiveResult {
                            line: line_1based,
                            kind: DirectiveKind::Module,
                            outcome: Outcome::Pass,
                            label: format!("Module: valid ({} errors)", errors),
                            diagnostics: diags,
                        }
                    } else {
                        DirectiveResult {
                            line: line_1based,
                            kind: DirectiveKind::Module,
                            outcome: Outcome::Fail,
                            label: format!("Module: expected valid but got {} error(s)", errors),
                            diagnostics: diags,
                        }
                    }
                }
                None => DirectiveResult {
                    line: line_1based,
                    kind: DirectiveKind::Skip,
                    outcome: Outcome::Skip,
                    label: "Module: could not extract text".to_string(),
                    diagnostics: vec![],
                },
            }
        }

        wast::WastDirective::AssertInvalid {
            module, message, ..
        } => {
            if is_binary_module(module) || is_component(module) {
                return DirectiveResult {
                    line: line_1based,
                    kind: DirectiveKind::Skip,
                    outcome: Outcome::Skip,
                    label: "AssertInvalid (binary/component): not applicable".to_string(),
                    diagnostics: vec![],
                };
            }
            match extract_wat_text(module, source) {
                Some(text) => {
                    let (errors, diags) = get_diagnostics(&text);
                    if errors > 0 {
                        DirectiveResult {
                            line: line_1based,
                            kind: DirectiveKind::AssertInvalid,
                            outcome: Outcome::Pass,
                            label: format!(
                                "AssertInvalid: found errors (expected \"{}\")",
                                message
                            ),
                            diagnostics: diags,
                        }
                    } else {
                        DirectiveResult {
                            line: line_1based,
                            kind: DirectiveKind::AssertInvalid,
                            outcome: Outcome::Fail,
                            label: format!("AssertInvalid: no errors (expected \"{}\")", message),
                            diagnostics: diags,
                        }
                    }
                }
                None => DirectiveResult {
                    line: line_1based,
                    kind: DirectiveKind::Skip,
                    outcome: Outcome::Skip,
                    label: "AssertInvalid: could not extract text".to_string(),
                    diagnostics: vec![],
                },
            }
        }

        wast::WastDirective::AssertMalformed {
            module, message, ..
        } => {
            if is_binary_module(module) || is_component(module) {
                return DirectiveResult {
                    line: line_1based,
                    kind: DirectiveKind::Skip,
                    outcome: Outcome::Skip,
                    label: "AssertMalformed (binary/component): not applicable".to_string(),
                    diagnostics: vec![],
                };
            }
            match extract_wat_text(module, source) {
                Some(text) => {
                    let (errors, diags) = get_diagnostics(&text);
                    if errors > 0 {
                        DirectiveResult {
                            line: line_1based,
                            kind: DirectiveKind::AssertMalformed,
                            outcome: Outcome::Pass,
                            label: format!(
                                "AssertMalformed: found errors (expected \"{}\")",
                                message
                            ),
                            diagnostics: diags,
                        }
                    } else {
                        DirectiveResult {
                            line: line_1based,
                            kind: DirectiveKind::AssertMalformed,
                            outcome: Outcome::Fail,
                            label: format!("AssertMalformed: no errors (expected \"{}\")", message),
                            diagnostics: diags,
                        }
                    }
                }
                None => DirectiveResult {
                    line: line_1based,
                    kind: DirectiveKind::Skip,
                    outcome: Outcome::Skip,
                    label: "AssertMalformed: could not extract text".to_string(),
                    diagnostics: vec![],
                },
            }
        }

        // Runtime / infrastructure directives — skip
        _ => DirectiveResult {
            line: line_1based,
            kind: DirectiveKind::Skip,
            outcome: Outcome::Skip,
            label: "Runtime/infrastructure directive: not applicable".to_string(),
            diagnostics: vec![],
        },
    }
}

// ---------------------------------------------------------------------------
// File processing
// ---------------------------------------------------------------------------

fn process_wast_file(path: &Path) -> Result<FileSummary, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;
    let filename = path.display().to_string();

    let buf = wast::parser::ParseBuffer::new(&source)
        .map_err(|e| format!("Failed to lex {}: {}", filename, e))?;

    let wast: wast::Wast =
        wast::parser::parse(&buf).map_err(|e| format!("Failed to parse {}: {}", filename, e))?;

    let mut summary = FileSummary {
        file: filename,
        directives: Vec::new(),
        pass: 0,
        fail: 0,
        skip: 0,
        modules: CategoryStats::default(),
        assert_invalid: CategoryStats::default(),
        assert_malformed: CategoryStats::default(),
    };

    for directive in &wast.directives {
        let result = process_directive(directive, &source);

        // Update category stats
        match result.kind {
            DirectiveKind::Module => {
                summary.modules.total += 1;
                if result.outcome == Outcome::Pass {
                    summary.modules.pass += 1;
                }
            }
            DirectiveKind::AssertInvalid => {
                summary.assert_invalid.total += 1;
                if result.outcome == Outcome::Pass {
                    summary.assert_invalid.pass += 1;
                }
            }
            DirectiveKind::AssertMalformed => {
                summary.assert_malformed.total += 1;
                if result.outcome == Outcome::Pass {
                    summary.assert_malformed.pass += 1;
                }
            }
            DirectiveKind::Skip => {}
        }

        match result.outcome {
            Outcome::Pass => summary.pass += 1,
            Outcome::Fail => summary.fail += 1,
            Outcome::Skip => summary.skip += 1,
        }

        summary.directives.push(result);
    }

    Ok(summary)
}

// ---------------------------------------------------------------------------
// File collection
// ---------------------------------------------------------------------------

fn collect_wast_files(paths: &[PathBuf], filter: &Option<String>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                let mut dir_files: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|ext| ext == "wast"))
                    .collect();
                dir_files.sort();
                files.extend(dir_files);
            }
        } else {
            files.push(path.clone());
        }
    }

    if let Some(substr) = filter {
        files.retain(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(substr.as_str()))
        });
    }

    files
}

// ---------------------------------------------------------------------------
// Output formatters
// ---------------------------------------------------------------------------

fn print_category(label: &str, stats: &CategoryStats) {
    if stats.total > 0 {
        let pct = 100.0 * stats.pass as f64 / stats.total as f64;
        eprintln!(
            "    {:<20} {}/{} ({:.1}%)",
            label, stats.pass, stats.total, pct
        );
    }
}

fn print_file_text(summary: &FileSummary, failures_only: bool, verbose: bool) {
    let total = summary.directives.len();
    eprintln!("\n=== {} ({} directives) ===", summary.file, total);

    for r in &summary.directives {
        match r.outcome {
            Outcome::Skip if failures_only => continue,
            Outcome::Pass if failures_only => continue,
            _ => {}
        }

        let tag = match r.outcome {
            Outcome::Pass => "\x1b[32m[PASS]\x1b[0m",
            Outcome::Fail => "\x1b[31m[FAIL]\x1b[0m",
            Outcome::Skip => "\x1b[33m[SKIP]\x1b[0m",
        };
        eprintln!("  {} L{:<4} {}", tag, r.line, r.label);

        if verbose && r.outcome == Outcome::Fail && !r.diagnostics.is_empty() {
            for d in &r.diagnostics {
                eprintln!("         {}", d);
            }
        }
    }

    eprintln!(
        "\n  Summary: {} pass, {} fail, {} skip",
        summary.pass, summary.fail, summary.skip
    );
    print_category("Valid modules:", &summary.modules);
    print_category("assert_invalid:", &summary.assert_invalid);
    print_category("assert_malformed:", &summary.assert_malformed);
}

fn print_global_text(global: &GlobalSummary) {
    eprintln!("\n=== GLOBAL SUMMARY ===");
    eprintln!(
        "Files: {} processed, {} parse errors",
        global.files_processed, global.files_parse_errors
    );
    eprintln!(
        "Directives: {} total — {} pass, {} fail, {} skip",
        global.total, global.pass, global.fail, global.skip
    );
    print_category("Valid modules:", &global.modules);
    print_category("assert_invalid:", &global.assert_invalid);
    print_category("assert_malformed:", &global.assert_malformed);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let args = Args::parse();
    let files = collect_wast_files(&args.paths, &args.filter);

    if files.is_empty() {
        eprintln!("No .wast files found");
        return ExitCode::from(1);
    }

    let mut global = GlobalSummary::default();
    let mut all_summaries: Vec<FileSummary> = Vec::new();

    for path in &files {
        match process_wast_file(path) {
            Ok(summary) => {
                global.files_processed += 1;
                global.total += summary.directives.len();
                global.pass += summary.pass;
                global.fail += summary.fail;
                global.skip += summary.skip;
                global.modules.pass += summary.modules.pass;
                global.modules.total += summary.modules.total;
                global.assert_invalid.pass += summary.assert_invalid.pass;
                global.assert_invalid.total += summary.assert_invalid.total;
                global.assert_malformed.pass += summary.assert_malformed.pass;
                global.assert_malformed.total += summary.assert_malformed.total;

                if matches!(args.format, OutputFormat::Text) {
                    print_file_text(&summary, args.failures_only, args.verbose);
                } else {
                    all_summaries.push(summary);
                }
            }
            Err(e) => {
                global.files_parse_errors += 1;
                if matches!(args.format, OutputFormat::Text) {
                    eprintln!("\n=== {} ===", path.display());
                    eprintln!("  ERROR: {}", e);
                }
            }
        }
    }

    match args.format {
        OutputFormat::Text => {
            if files.len() > 1 {
                print_global_text(&global);
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "files": all_summaries,
                "summary": global,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
    }

    ExitCode::SUCCESS
}
