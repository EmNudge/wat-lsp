//! WAT LSP Benchmark Tool
//!
//! Benchmarks all LSP operations with detailed timing breakdowns.
//! Run with: cargo run --bin wat-bench --features native -- [OPTIONS]

use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tower_lsp::lsp_types::Position;
use wat_lsp_rust::core::types::Position as CorePosition;
use wat_lsp_rust::test_utils::generate_large_wat;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::{completion, definition, diagnostics, hover, parser, references};

#[derive(Parser, Debug)]
#[command(name = "wat-bench")]
#[command(about = "Benchmark WAT LSP operations")]
struct Args {
    /// WAT file or directory to benchmark (defaults to packages/playground/examples)
    #[arg(short, long)]
    path: Option<PathBuf>,

    /// Number of iterations for each benchmark
    #[arg(short, long, default_value = "10")]
    iterations: usize,

    /// Include synthetic large file tests
    #[arg(long)]
    include_large: bool,

    /// Show detailed timing for each operation
    #[arg(short, long)]
    verbose: bool,

    /// Output format: text, json, or markdown
    #[arg(short, long, default_value = "text")]
    format: String,

    /// Only run specific benchmarks (parse, symbols, diagnostics, hover, completion, definition, references)
    #[arg(long)]
    only: Option<String>,
}

/// Timing results for a single benchmark run
#[derive(Debug, Clone)]
struct TimingResult {
    name: String,
    file: String,
    lines: usize,
    min_time: Duration,
    max_time: Duration,
    avg_time: Duration,
}

impl TimingResult {
    fn throughput_lines_per_ms(&self) -> f64 {
        if self.avg_time.as_micros() == 0 {
            return 0.0;
        }
        self.lines as f64 / (self.avg_time.as_micros() as f64 / 1000.0)
    }
}

/// Aggregate results across multiple files
#[derive(Debug, Default)]
struct BenchmarkSummary {
    operation: String,
    results: Vec<TimingResult>,
}

impl BenchmarkSummary {
    fn new(operation: &str) -> Self {
        Self {
            operation: operation.to_string(),
            results: Vec::new(),
        }
    }

    fn add(&mut self, result: TimingResult) {
        self.results.push(result);
    }

    fn total_avg(&self) -> Duration {
        if self.results.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.results.iter().map(|r| r.avg_time).sum();
        total / self.results.len() as u32
    }

    fn total_min(&self) -> Duration {
        self.results
            .iter()
            .map(|r| r.min_time)
            .min()
            .unwrap_or(Duration::ZERO)
    }

    fn total_max(&self) -> Duration {
        self.results
            .iter()
            .map(|r| r.max_time)
            .max()
            .unwrap_or(Duration::ZERO)
    }
}

fn main() {
    let args = Args::parse();

    // Determine which files to benchmark
    let files = collect_wat_files(&args);
    if files.is_empty() {
        eprintln!("No WAT files found");
        std::process::exit(1);
    }

    println!("WAT LSP Benchmark");
    println!("=================");
    println!("Files: {}", files.len());
    println!("Iterations: {}", args.iterations);
    println!();

    let benchmarks_to_run = if let Some(ref only) = args.only {
        only.split(',').map(|s| s.trim().to_lowercase()).collect()
    } else {
        vec![
            "parse".to_string(),
            "symbols".to_string(),
            "diagnostics".to_string(),
            "hover".to_string(),
            "completion".to_string(),
            "definition".to_string(),
            "references".to_string(),
        ]
    };

    let mut all_summaries = Vec::new();

    // Run benchmarks for each file
    for file_path in &files {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read {}: {}", file_path.display(), e);
                continue;
            }
        };

        let file_name = file_path.file_name().unwrap().to_string_lossy().to_string();
        let lines = content.lines().count();
        let bytes = content.len();

        if args.verbose {
            println!("\n--- {} ({} lines, {} bytes) ---", file_name, lines, bytes);
        }

        // Parse once to get tree and symbols for other benchmarks
        let mut parser = create_parser();
        let tree = match parser.parse(&content, None) {
            Some(t) => t,
            None => {
                eprintln!("Failed to parse {}", file_name);
                continue;
            }
        };
        let symbols = match parser::parse_document(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to extract symbols from {}: {}", file_name, e);
                continue;
            }
        };

        // Run each benchmark
        if benchmarks_to_run.contains(&"parse".to_string()) {
            let result = benchmark_parse(&content, &file_name, lines, args.iterations);
            print_result(&result, args.verbose, &args.format);
            add_to_summary(&mut all_summaries, "parse", result);
        }

        if benchmarks_to_run.contains(&"symbols".to_string()) {
            let result = benchmark_symbols(&content, &file_name, lines, args.iterations);
            print_result(&result, args.verbose, &args.format);
            add_to_summary(&mut all_summaries, "symbols", result);
        }

        if benchmarks_to_run.contains(&"diagnostics".to_string()) {
            let result = benchmark_diagnostics(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, args.verbose, &args.format);
            add_to_summary(&mut all_summaries, "diagnostics", result);
        }

        if benchmarks_to_run.contains(&"hover".to_string()) {
            let result = benchmark_hover(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, args.verbose, &args.format);
            add_to_summary(&mut all_summaries, "hover", result);
        }

        if benchmarks_to_run.contains(&"completion".to_string()) {
            let result =
                benchmark_completion(&content, &symbols, &file_name, lines, args.iterations);
            print_result(&result, args.verbose, &args.format);
            add_to_summary(&mut all_summaries, "completion", result);
        }

        if benchmarks_to_run.contains(&"definition".to_string()) {
            let result = benchmark_definition(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, args.verbose, &args.format);
            add_to_summary(&mut all_summaries, "definition", result);
        }

        if benchmarks_to_run.contains(&"references".to_string()) {
            let result = benchmark_references(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, args.verbose, &args.format);
            add_to_summary(&mut all_summaries, "references", result);
        }
    }

    // Run large file benchmarks if requested
    if args.include_large {
        println!("\n=== Large File Benchmarks ===");
        run_large_file_benchmarks(&args, &benchmarks_to_run, &mut all_summaries);
    }

    // Print summary
    print_summary(&all_summaries, &args.format);
}

fn collect_wat_files(args: &Args) -> Vec<PathBuf> {
    let path = args
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from("packages/playground/examples"));

    if path.is_file() {
        vec![path]
    } else if path.is_dir() {
        let mut files: Vec<PathBuf> = fs::read_dir(&path)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "wat").unwrap_or(false))
            .collect();
        files.sort();
        files
    } else {
        Vec::new()
    }
}

fn benchmark_parse(
    content: &str,
    file_name: &str,
    lines: usize,
    iterations: usize,
) -> TimingResult {
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let mut parser = create_parser();
        let start = Instant::now();
        let _tree = parser.parse(content, None);
        times.push(start.elapsed());
    }

    create_timing_result("parse", file_name, lines, &times)
}

fn benchmark_symbols(
    content: &str,
    file_name: &str,
    lines: usize,
    iterations: usize,
) -> TimingResult {
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();
        let _symbols = parser::parse_document(content);
        times.push(start.elapsed());
    }

    create_timing_result("symbols", file_name, lines, &times)
}

fn benchmark_diagnostics(
    tree: &tree_sitter::Tree,
    content: &str,
    symbols: &wat_lsp_rust::symbols::SymbolTable,
    file_name: &str,
    lines: usize,
    iterations: usize,
) -> TimingResult {
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();
        let _syntax = diagnostics::provide_tree_sitter_diagnostics(tree, content);
        let _semantic = diagnostics::provide_semantic_diagnostics(tree, content, symbols);
        times.push(start.elapsed());
    }

    create_timing_result("diagnostics", file_name, lines, &times)
}

fn benchmark_hover(
    tree: &tree_sitter::Tree,
    content: &str,
    symbols: &wat_lsp_rust::symbols::SymbolTable,
    file_name: &str,
    lines: usize,
    iterations: usize,
) -> TimingResult {
    let mut times = Vec::with_capacity(iterations);

    // Test hover at multiple positions
    let positions = generate_test_positions(lines, 10);

    for _ in 0..iterations {
        let start = Instant::now();
        for pos in &positions {
            let _hover = hover::provide_hover(content, symbols, tree, *pos);
        }
        times.push(start.elapsed());
    }

    create_timing_result("hover", file_name, lines, &times)
}

fn benchmark_completion(
    content: &str,
    symbols: &wat_lsp_rust::symbols::SymbolTable,
    file_name: &str,
    lines: usize,
    iterations: usize,
) -> TimingResult {
    let mut times = Vec::with_capacity(iterations);

    // Test completion at multiple positions
    let positions: Vec<CorePosition> = generate_test_positions(lines, 10)
        .into_iter()
        .map(|p| CorePosition::new(p.line, p.character))
        .collect();

    for _ in 0..iterations {
        let start = Instant::now();
        for pos in &positions {
            let _completions = completion::provide_completion(content, symbols, *pos);
        }
        times.push(start.elapsed());
    }

    create_timing_result("completion", file_name, lines, &times)
}

fn benchmark_definition(
    tree: &tree_sitter::Tree,
    content: &str,
    symbols: &wat_lsp_rust::symbols::SymbolTable,
    file_name: &str,
    lines: usize,
    iterations: usize,
) -> TimingResult {
    let mut times = Vec::with_capacity(iterations);

    let positions = generate_test_positions(lines, 10);
    let uri = format!("file:///{}", file_name);

    for _ in 0..iterations {
        let start = Instant::now();
        for pos in &positions {
            let _def = definition::provide_definition(content, symbols, tree, *pos, &uri);
        }
        times.push(start.elapsed());
    }

    create_timing_result("definition", file_name, lines, &times)
}

fn benchmark_references(
    tree: &tree_sitter::Tree,
    content: &str,
    symbols: &wat_lsp_rust::symbols::SymbolTable,
    file_name: &str,
    lines: usize,
    iterations: usize,
) -> TimingResult {
    let mut times = Vec::with_capacity(iterations);

    let positions = generate_test_positions(lines, 5); // Fewer positions as references is more expensive
    let uri = format!("file:///{}", file_name);

    for _ in 0..iterations {
        let start = Instant::now();
        for pos in &positions {
            let _refs = references::provide_references(content, symbols, tree, *pos, &uri, true);
        }
        times.push(start.elapsed());
    }

    create_timing_result("references", file_name, lines, &times)
}

fn generate_test_positions(lines: usize, count: usize) -> Vec<Position> {
    let mut positions = Vec::with_capacity(count);
    let step = lines.max(1) / count.max(1);

    for i in 0..count {
        let line = (i * step).min(lines.saturating_sub(1)) as u32;
        positions.push(Position::new(line, 5));
    }

    positions
}

fn create_timing_result(name: &str, file: &str, lines: usize, times: &[Duration]) -> TimingResult {
    let total: Duration = times.iter().sum();
    let min = *times.iter().min().unwrap_or(&Duration::ZERO);
    let max = *times.iter().max().unwrap_or(&Duration::ZERO);
    let avg = total / times.len() as u32;

    TimingResult {
        name: name.to_string(),
        file: file.to_string(),
        lines,
        min_time: min,
        max_time: max,
        avg_time: avg,
    }
}

fn add_to_summary(summaries: &mut Vec<BenchmarkSummary>, operation: &str, result: TimingResult) {
    if let Some(summary) = summaries.iter_mut().find(|s| s.operation == operation) {
        summary.add(result);
    } else {
        let mut summary = BenchmarkSummary::new(operation);
        summary.add(result);
        summaries.push(summary);
    }
}

fn print_result(result: &TimingResult, verbose: bool, format: &str) {
    if !verbose {
        return;
    }

    match format {
        "json" => {
            println!(
                r#"  {{"op": "{}", "file": "{}", "avg_us": {}, "min_us": {}, "max_us": {}, "lines_per_ms": {:.2}}}"#,
                result.name,
                result.file,
                result.avg_time.as_micros(),
                result.min_time.as_micros(),
                result.max_time.as_micros(),
                result.throughput_lines_per_ms()
            );
        }
        "markdown" => {
            println!(
                "| {} | {} | {:?} | {:?} | {:?} | {:.2} |",
                result.name,
                result.file,
                result.avg_time,
                result.min_time,
                result.max_time,
                result.throughput_lines_per_ms()
            );
        }
        _ => {
            println!(
                "  {:12} avg={:>10?}  min={:>10?}  max={:>10?}  ({:.2} lines/ms)",
                result.name,
                result.avg_time,
                result.min_time,
                result.max_time,
                result.throughput_lines_per_ms()
            );
        }
    }
}

fn print_summary(summaries: &[BenchmarkSummary], format: &str) {
    println!("\n=== Benchmark Summary ===");

    match format {
        "markdown" => {
            println!("| Operation | Avg | Min | Max | Files |");
            println!("|-----------|-----|-----|-----|-------|");
            for summary in summaries {
                println!(
                    "| {} | {:?} | {:?} | {:?} | {} |",
                    summary.operation,
                    summary.total_avg(),
                    summary.total_min(),
                    summary.total_max(),
                    summary.results.len()
                );
            }
        }
        "json" => {
            println!("{{");
            for (i, summary) in summaries.iter().enumerate() {
                let comma = if i < summaries.len() - 1 { "," } else { "" };
                println!(
                    r#"  "{}": {{"avg_us": {}, "min_us": {}, "max_us": {}, "files": {}}}{}"#,
                    summary.operation,
                    summary.total_avg().as_micros(),
                    summary.total_min().as_micros(),
                    summary.total_max().as_micros(),
                    summary.results.len(),
                    comma
                );
            }
            println!("}}");
        }
        _ => {
            println!(
                "{:<15} {:>12} {:>12} {:>12} {:>8}",
                "Operation", "Avg", "Min", "Max", "Files"
            );
            println!("{}", "-".repeat(62));
            for summary in summaries {
                println!(
                    "{:<15} {:>12?} {:>12?} {:>12?} {:>8}",
                    summary.operation,
                    summary.total_avg(),
                    summary.total_min(),
                    summary.total_max(),
                    summary.results.len()
                );
            }
        }
    }

    // Performance assessment
    println!("\n=== Performance Assessment ===");
    for summary in summaries {
        let avg_us = summary.total_avg().as_micros();
        let status = if avg_us < 1000 {
            "EXCELLENT (<1ms)"
        } else if avg_us < 10000 {
            "GOOD (<10ms)"
        } else if avg_us < 50000 {
            "ACCEPTABLE (<50ms)"
        } else if avg_us < 100000 {
            "SLOW (<100ms)"
        } else {
            "NEEDS OPTIMIZATION (>100ms)"
        };
        println!(
            "{:<15} {:>12?} - {}",
            summary.operation,
            summary.total_avg(),
            status
        );
    }
}

fn run_large_file_benchmarks(
    args: &Args,
    benchmarks: &[String],
    summaries: &mut Vec<BenchmarkSummary>,
) {
    // Generate synthetic large files of various sizes
    let sizes = [1000, 5000, 15000];

    for size in sizes {
        println!("\n--- Synthetic {} lines ---", size);
        let content = generate_large_wat(size);
        let file_name = format!("synthetic_{}lines", size);
        let lines = content.lines().count();

        let mut parser = create_parser();
        let tree = match parser.parse(&content, None) {
            Some(t) => t,
            None => {
                eprintln!("Failed to parse synthetic file");
                continue;
            }
        };
        let symbols = match parser::parse_document(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to extract symbols: {}", e);
                continue;
            }
        };

        if benchmarks.contains(&"parse".to_string()) {
            let result = benchmark_parse(&content, &file_name, lines, args.iterations);
            print_result(&result, true, &args.format);
            add_to_summary(summaries, "parse (large)", result);
        }

        if benchmarks.contains(&"symbols".to_string()) {
            let result = benchmark_symbols(&content, &file_name, lines, args.iterations);
            print_result(&result, true, &args.format);
            add_to_summary(summaries, "symbols (large)", result);
        }

        if benchmarks.contains(&"diagnostics".to_string()) {
            let result = benchmark_diagnostics(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, true, &args.format);
            add_to_summary(summaries, "diagnostics (large)", result);
        }

        if benchmarks.contains(&"hover".to_string()) {
            let result = benchmark_hover(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, true, &args.format);
            add_to_summary(summaries, "hover (large)", result);
        }

        if benchmarks.contains(&"completion".to_string()) {
            let result =
                benchmark_completion(&content, &symbols, &file_name, lines, args.iterations);
            print_result(&result, true, &args.format);
            add_to_summary(summaries, "completion (large)", result);
        }

        if benchmarks.contains(&"definition".to_string()) {
            let result = benchmark_definition(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, true, &args.format);
            add_to_summary(summaries, "definition (large)", result);
        }

        if benchmarks.contains(&"references".to_string()) {
            let result = benchmark_references(
                &tree,
                &content,
                &symbols,
                &file_name,
                lines,
                args.iterations,
            );
            print_result(&result, true, &args.format);
            add_to_summary(summaries, "references (large)", result);
        }
    }
}
