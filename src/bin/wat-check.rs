//! wat-check: A standalone CLI tool for scanning WAT files for issues
//!
//! This tool reuses the diagnostic capabilities of the wat-lsp-rust language server
//! to provide file validation without running as an LSP server.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use wat_lsp_rust::diagnostics::{
    merge_all_diagnostics, provide_semantic_diagnostics, provide_tree_sitter_diagnostics,
    validate_wat,
};
use wat_lsp_rust::parser::parse_document;
use wat_lsp_rust::tree_sitter_bindings::create_parser;

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum OutputFormat {
    /// Human-readable text output (default)
    #[default]
    Text,
    /// JSON output for machine processing
    Json,
    /// Compact single-line-per-diagnostic format
    Compact,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum DiagnosticLevel {
    /// Only syntax errors (fastest)
    Syntax,
    /// Syntax + semantic validation (default)
    #[default]
    Semantic,
    /// Full validation including wast deep checks (slowest but most thorough)
    Full,
}

/// A standalone WAT file checker using the wat-lsp-rust diagnostic engine
#[derive(Parser, Debug)]
#[command(name = "wat-check")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Files to check. Use '-' to read from stdin.
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Diagnostic level (controls which checks are run)
    #[arg(short, long, value_enum, default_value_t = DiagnosticLevel::Semantic)]
    level: DiagnosticLevel,

    /// Only show errors (hide warnings and hints)
    #[arg(short = 'e', long)]
    errors_only: bool,

    /// Suppress all output except errors (for scripting)
    #[arg(short, long)]
    quiet: bool,
}

#[derive(serde::Serialize)]
struct FileResult {
    file: String,
    diagnostics: Vec<DiagnosticOutput>,
    error_count: usize,
    warning_count: usize,
}

#[derive(serde::Serialize)]
struct DiagnosticOutput {
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    severity: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

impl From<&Diagnostic> for DiagnosticOutput {
    fn from(d: &Diagnostic) -> Self {
        DiagnosticOutput {
            line: d.range.start.line + 1,
            column: d.range.start.character + 1,
            end_line: d.range.end.line + 1,
            end_column: d.range.end.character + 1,
            severity: match d.severity {
                Some(DiagnosticSeverity::ERROR) => "error".to_string(),
                Some(DiagnosticSeverity::WARNING) => "warning".to_string(),
                Some(DiagnosticSeverity::INFORMATION) => "info".to_string(),
                Some(DiagnosticSeverity::HINT) => "hint".to_string(),
                _ => "unknown".to_string(),
            },
            message: d.message.clone(),
            source: d.source.clone(),
        }
    }
}

fn check_file(
    filename: &str,
    source: &str,
    level: DiagnosticLevel,
    errors_only: bool,
) -> Vec<Diagnostic> {
    let mut parser = create_parser();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            eprintln!("{}: Failed to parse file", filename);
            return vec![];
        }
    };

    // Layer 1: Tree-sitter syntax diagnostics (always run)
    let syntax_diags = provide_tree_sitter_diagnostics(&tree, source);

    let all_diags = match level {
        DiagnosticLevel::Syntax => syntax_diags,
        DiagnosticLevel::Semantic | DiagnosticLevel::Full => {
            // Layer 2: Semantic diagnostics
            let semantic_diags = match parse_document(source) {
                Ok(symbols) => provide_semantic_diagnostics(&tree, source, &symbols),
                Err(_) => vec![],
            };

            if matches!(level, DiagnosticLevel::Full) {
                // Layer 3: WAST deep validation
                let wast_diags = validate_wat(source);
                merge_all_diagnostics(syntax_diags, semantic_diags, wast_diags)
            } else {
                merge_all_diagnostics(syntax_diags, semantic_diags, vec![])
            }
        }
    };

    if errors_only {
        all_diags
            .into_iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect()
    } else {
        all_diags
    }
}

fn severity_symbol(severity: Option<DiagnosticSeverity>) -> &'static str {
    match severity {
        Some(DiagnosticSeverity::ERROR) => "error",
        Some(DiagnosticSeverity::WARNING) => "warning",
        Some(DiagnosticSeverity::INFORMATION) => "info",
        Some(DiagnosticSeverity::HINT) => "hint",
        _ => "unknown",
    }
}

fn print_diagnostics_text(filename: &str, diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        let line = d.range.start.line + 1;
        let col = d.range.start.character + 1;
        let severity = severity_symbol(d.severity);
        println!("{}:{}:{}: {}: {}", filename, line, col, severity, d.message);
    }
}

fn print_diagnostics_compact(filename: &str, diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        let line = d.range.start.line + 1;
        let col = d.range.start.character + 1;
        let severity = match d.severity {
            Some(DiagnosticSeverity::ERROR) => "E",
            Some(DiagnosticSeverity::WARNING) => "W",
            Some(DiagnosticSeverity::INFORMATION) => "I",
            Some(DiagnosticSeverity::HINT) => "H",
            _ => "?",
        };
        println!("{}:{}:{}:{}:{}", filename, line, col, severity, d.message);
    }
}

fn main() -> ExitCode {
    let args = Args::parse();

    let mut all_results: Vec<FileResult> = vec![];
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut had_read_error = false;

    for path in &args.files {
        let (filename, source) = if path.as_os_str() == "-" {
            // Read from stdin
            let mut buf = String::new();
            if let Err(e) = io::stdin().read_to_string(&mut buf) {
                eprintln!("stdin: Failed to read: {}", e);
                had_read_error = true;
                continue;
            }
            ("<stdin>".to_string(), buf)
        } else {
            match fs::read_to_string(path) {
                Ok(content) => (path.display().to_string(), content),
                Err(e) => {
                    eprintln!("{}: Failed to read: {}", path.display(), e);
                    had_read_error = true;
                    continue;
                }
            }
        };

        let diagnostics = check_file(&filename, &source, args.level, args.errors_only);

        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .count();
        let warning_count = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
            .count();

        total_errors += error_count;
        total_warnings += warning_count;

        match args.format {
            OutputFormat::Text => {
                if !diagnostics.is_empty() {
                    print_diagnostics_text(&filename, &diagnostics);
                }
            }
            OutputFormat::Compact => {
                print_diagnostics_compact(&filename, &diagnostics);
            }
            OutputFormat::Json => {
                all_results.push(FileResult {
                    file: filename,
                    diagnostics: diagnostics.iter().map(DiagnosticOutput::from).collect(),
                    error_count,
                    warning_count,
                });
            }
        }
    }

    // JSON output at the end
    if matches!(args.format, OutputFormat::Json) {
        let output = serde_json::json!({
            "files": all_results,
            "summary": {
                "total_errors": total_errors,
                "total_warnings": total_warnings,
                "files_checked": args.files.len(),
            }
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else if !args.quiet {
        // Summary for text output
        if args.files.len() > 1 || total_errors > 0 || total_warnings > 0 {
            let file_count = args.files.len();
            let file_word = if file_count == 1 { "file" } else { "files" };
            eprintln!(
                "\nChecked {} {}: {} error(s), {} warning(s)",
                file_count, file_word, total_errors, total_warnings
            );
        }
    }

    // Exit code: 1 if errors found, 2 if read errors, 0 if clean
    if total_errors > 0 {
        ExitCode::from(1)
    } else if had_read_error {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}
