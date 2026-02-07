//! Parser benchmarks for WAT LSP
//!
//! Run with: cargo bench --bench parser --features native

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wat_lsp_rust::parser::parse_document;
use wat_lsp_rust::test_utils::generate_large_wat;
use wat_lsp_rust::tree_sitter_bindings::create_parser;

fn bench_tree_sitter_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_sitter_parse");

    for size in [100, 500, 1000, 5000] {
        let content = generate_large_wat(size);
        let lines = content.lines().count();

        group.throughput(Throughput::Elements(lines as u64));
        group.bench_with_input(BenchmarkId::new("lines", lines), &content, |b, content| {
            b.iter(|| {
                let mut parser = create_parser();
                black_box(parser.parse(content, None))
            });
        });
    }

    group.finish();
}

fn bench_symbol_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_extraction");

    for size in [100, 500, 1000, 5000] {
        let content = generate_large_wat(size);
        let lines = content.lines().count();

        group.throughput(Throughput::Elements(lines as u64));
        group.bench_with_input(BenchmarkId::new("lines", lines), &content, |b, content| {
            b.iter(|| black_box(parse_document(content)));
        });
    }

    group.finish();
}

fn bench_incremental_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_parse");

    let content = generate_large_wat(1000);
    let lines = content.lines().count();

    // Initial parse
    let mut parser = create_parser();
    let tree = parser.parse(&content, None).unwrap();

    // Simulate a small edit (add a comment)
    let mut modified = content.clone();
    modified.insert_str(50, ";; comment\n");

    group.throughput(Throughput::Elements(lines as u64));
    group.bench_function("with_old_tree", |b| {
        b.iter(|| {
            let mut parser = create_parser();
            black_box(parser.parse(&modified, Some(&tree)))
        });
    });

    group.bench_function("without_old_tree", |b| {
        b.iter(|| {
            let mut parser = create_parser();
            black_box(parser.parse(&modified, None))
        });
    });

    group.finish();
}

fn bench_real_world_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_parse");

    // A representative complex module
    let complex_module = r#"(module
  ;; Types
  (type $binop (func (param i32 i32) (result i32)))
  (type $unop (func (param i32) (result i32)))

  ;; Imports
  (import "env" "log" (func $log (param i32)))

  ;; Memory
  (memory $mem 1 10)

  ;; Globals
  (global $counter (mut i32) (i32.const 0))
  (global $max_value i32 (i32.const 1000))

  ;; Tables
  (table $funcs 10 funcref)

  ;; Functions
  (func $add (type $binop)
    (local $temp i32)
    (local.set $temp (i32.add (local.get 0) (local.get 1)))
    (local.get $temp))

  (func $sub (type $binop)
    (i32.sub (local.get 0) (local.get 1)))

  (func $factorial (type $unop)
    (local $result i32)
    (local.set $result (i32.const 1))
    (block $done
      (loop $loop
        (br_if $done (i32.le_s (local.get 0) (i32.const 1)))
        (local.set $result (i32.mul (local.get $result) (local.get 0)))
        (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
        (br $loop)))
    (local.get $result))

  (func $fibonacci (param $n i32) (result i32)
    (local $a i32)
    (local $b i32)
    (local $temp i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then (local.get $n))
      (else
        (local.set $a (i32.const 0))
        (local.set $b (i32.const 1))
        (block $done
          (loop $loop
            (br_if $done (i32.le_s (local.get $n) (i32.const 1)))
            (local.set $temp (i32.add (local.get $a) (local.get $b)))
            (local.set $a (local.get $b))
            (local.set $b (local.get $temp))
            (local.set $n (i32.sub (local.get $n) (i32.const 1)))
            (br $loop)))
        (local.get $b))))

  ;; Exports
  (export "add" (func $add))
  (export "factorial" (func $factorial))
  (export "fibonacci" (func $fibonacci))
)"#;

    group.bench_function("complex_module", |b| {
        b.iter(|| {
            let mut parser = create_parser();
            black_box(parser.parse(complex_module, None))
        });
    });

    group.bench_function("complex_module_symbols", |b| {
        b.iter(|| black_box(parse_document(complex_module)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tree_sitter_parse,
    bench_symbol_extraction,
    bench_incremental_parse,
    bench_real_world_files,
);

criterion_main!(benches);
