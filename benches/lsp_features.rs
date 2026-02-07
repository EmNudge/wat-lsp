//! LSP Features benchmarks for WAT LSP
//!
//! Benchmarks hover, completion, definition, references, and diagnostics.
//! Run with: cargo bench --bench lsp_features --features native

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tower_lsp::lsp_types::Position;
use wat_lsp_rust::core::types::Position as CorePosition;
use wat_lsp_rust::parser::parse_document;
use wat_lsp_rust::test_utils::generate_large_wat;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::{completion, definition, diagnostics, hover, references};

/// Setup struct to avoid repeated parsing in benchmarks
struct BenchSetup {
    content: String,
    tree: tree_sitter::Tree,
    symbols: wat_lsp_rust::symbols::SymbolTable,
    positions: Vec<Position>,
    core_positions: Vec<CorePosition>,
}

impl BenchSetup {
    fn new(target_lines: usize) -> Self {
        let content = generate_large_wat(target_lines);
        let mut parser = create_parser();
        let tree = parser.parse(&content, None).unwrap();
        let symbols = parse_document(&content).unwrap();
        let lines = content.lines().count();

        // Generate test positions spread across the file
        let positions: Vec<Position> = (0..10)
            .map(|i| {
                let line = ((i * lines) / 10).min(lines.saturating_sub(1)) as u32;
                Position::new(line, 5)
            })
            .collect();

        let core_positions: Vec<CorePosition> = positions
            .iter()
            .map(|p| CorePosition::new(p.line, p.character))
            .collect();

        Self {
            content,
            tree,
            symbols,
            positions,
            core_positions,
        }
    }
}

fn bench_hover(c: &mut Criterion) {
    let mut group = c.benchmark_group("hover");

    for size in [100, 500, 1000, 5000] {
        let setup = BenchSetup::new(size);
        let lines = setup.content.lines().count();

        group.throughput(Throughput::Elements(setup.positions.len() as u64));
        group.bench_with_input(BenchmarkId::new("lines", lines), &setup, |b, setup| {
            b.iter(|| {
                for pos in &setup.positions {
                    black_box(hover::provide_hover(
                        &setup.content,
                        &setup.symbols,
                        &setup.tree,
                        *pos,
                    ));
                }
            });
        });
    }

    group.finish();
}

fn bench_completion(c: &mut Criterion) {
    let mut group = c.benchmark_group("completion");

    for size in [100, 500, 1000, 5000] {
        let setup = BenchSetup::new(size);
        let lines = setup.content.lines().count();

        group.throughput(Throughput::Elements(setup.core_positions.len() as u64));
        group.bench_with_input(BenchmarkId::new("lines", lines), &setup, |b, setup| {
            b.iter(|| {
                for pos in &setup.core_positions {
                    black_box(completion::provide_completion(
                        &setup.content,
                        &setup.symbols,
                        *pos,
                    ));
                }
            });
        });
    }

    group.finish();
}

fn bench_definition(c: &mut Criterion) {
    let mut group = c.benchmark_group("definition");

    for size in [100, 500, 1000, 5000] {
        let setup = BenchSetup::new(size);
        let lines = setup.content.lines().count();
        let uri = "file:///test.wat";

        group.throughput(Throughput::Elements(setup.positions.len() as u64));
        group.bench_with_input(BenchmarkId::new("lines", lines), &setup, |b, setup| {
            b.iter(|| {
                for pos in &setup.positions {
                    black_box(definition::provide_definition(
                        &setup.content,
                        &setup.symbols,
                        &setup.tree,
                        *pos,
                        uri,
                    ));
                }
            });
        });
    }

    group.finish();
}

fn bench_references(c: &mut Criterion) {
    let mut group = c.benchmark_group("references");
    group.sample_size(50); // Fewer samples as references is expensive

    for size in [100, 500, 1000] {
        let setup = BenchSetup::new(size);
        let lines = setup.content.lines().count();
        let uri = "file:///test.wat";
        // Use fewer positions for references as it's more expensive
        let positions: Vec<Position> = setup.positions.iter().take(5).cloned().collect();

        group.throughput(Throughput::Elements(positions.len() as u64));
        group.bench_with_input(BenchmarkId::new("lines", lines), &setup, |b, setup| {
            b.iter(|| {
                for pos in &positions {
                    black_box(references::provide_references(
                        &setup.content,
                        &setup.symbols,
                        &setup.tree,
                        *pos,
                        uri,
                        true,
                    ));
                }
            });
        });
    }

    group.finish();
}

fn bench_diagnostics(c: &mut Criterion) {
    let mut group = c.benchmark_group("diagnostics");

    for size in [100, 500, 1000, 5000] {
        let setup = BenchSetup::new(size);
        let lines = setup.content.lines().count();

        group.throughput(Throughput::Elements(lines as u64));
        group.bench_with_input(BenchmarkId::new("lines", lines), &setup, |b, setup| {
            b.iter(|| {
                let syntax =
                    diagnostics::provide_tree_sitter_diagnostics(&setup.tree, &setup.content);
                let semantic = diagnostics::provide_semantic_diagnostics(
                    &setup.tree,
                    &setup.content,
                    &setup.symbols,
                );
                black_box((syntax, semantic))
            });
        });
    }

    group.finish();
}

fn bench_diagnostics_breakdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("diagnostics_breakdown");

    let setup = BenchSetup::new(1000);

    group.bench_function("syntax_only", |b| {
        b.iter(|| {
            black_box(diagnostics::provide_tree_sitter_diagnostics(
                &setup.tree,
                &setup.content,
            ))
        });
    });

    group.bench_function("semantic_only", |b| {
        b.iter(|| {
            black_box(diagnostics::provide_semantic_diagnostics(
                &setup.tree,
                &setup.content,
                &setup.symbols,
            ))
        });
    });

    group.finish();
}

fn bench_single_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_operation");

    // Use a realistic module for single operation benchmarks
    let content = r#"(module
  (type $binop (func (param i32 i32) (result i32)))
  (global $counter (mut i32) (i32.const 0))
  (func $add (type $binop)
    (local $temp i32)
    (local.set $temp (i32.add (local.get 0) (local.get 1)))
    (global.set $counter (i32.add (global.get $counter) (i32.const 1)))
    (local.get $temp))
  (func $main (result i32)
    (call $add (i32.const 1) (i32.const 2)))
)"#;

    let mut parser = create_parser();
    let tree = parser.parse(content, None).unwrap();
    let symbols = parse_document(content).unwrap();
    let uri = "file:///test.wat";

    // Position on "i32.add" instruction
    let instr_pos = Position::new(5, 25);
    let instr_core_pos = CorePosition::new(5, 25);

    // Position on "$add" function call
    let call_pos = Position::new(8, 10);

    // Position on "$temp" local
    let local_pos = Position::new(5, 16);

    group.bench_function("hover_instruction", |b| {
        b.iter(|| black_box(hover::provide_hover(content, &symbols, &tree, instr_pos)));
    });

    group.bench_function("hover_function_call", |b| {
        b.iter(|| black_box(hover::provide_hover(content, &symbols, &tree, call_pos)));
    });

    group.bench_function("completion_context", |b| {
        b.iter(|| {
            black_box(completion::provide_completion(
                content,
                &symbols,
                instr_core_pos,
            ))
        });
    });

    group.bench_function("definition_local", |b| {
        b.iter(|| {
            black_box(definition::provide_definition(
                content, &symbols, &tree, local_pos, uri,
            ))
        });
    });

    group.bench_function("definition_function", |b| {
        b.iter(|| {
            black_box(definition::provide_definition(
                content, &symbols, &tree, call_pos, uri,
            ))
        });
    });

    group.bench_function("references_function", |b| {
        b.iter(|| {
            black_box(references::provide_references(
                content, &symbols, &tree, call_pos, uri, true,
            ))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_hover,
    bench_completion,
    bench_definition,
    bench_references,
    bench_diagnostics,
    bench_diagnostics_breakdown,
    bench_single_operations,
);

criterion_main!(benches);
