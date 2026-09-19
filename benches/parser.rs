//! Parser benchmarks for WAT LSP
//!
//! Run with: cargo bench --bench parser --features native

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wat_lsp_rust::parser::parse_document;
use wat_lsp_rust::test_utils::generate_large_wat;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::utils::determine_instruction_context;

// ── Pathological-shape generators ───────────────────────────────────────────
// These target the traversals whose scaling was fixed: extract_doc_comment
// (rescanned all preceding module fields per function) and the catch-clause /
// context ancestor walk (walked to the module root per identifier).

/// A module of `n` functions, each preceded by its own doc comment. Stresses
/// per-function doc-comment collection.
fn gen_many_functions(n: usize) -> String {
    let mut s = String::from("(module\n");
    for i in 0..n {
        s.push_str(&format!(
            "  ;; doc comment for function number {i}\n  (func $f{i} (result i32) (i32.const {i}))\n"
        ));
    }
    s.push(')');
    s
}

/// A single function body nested `depth` labeled blocks deep. Stresses the
/// ancestor walk when resolving context for deeply nested identifiers.
///
/// Note: this shape stays superlinear even after the fix because tree-sitter's
/// `Node::parent()` is itself O(depth-from-root). The fix cut the number of
/// `parent()` calls per identifier from O(depth) (a walk to the module root) to
/// O(1); the residual growth is the cost of those individual `parent()` calls,
/// not a redundant traversal. The sibling-block shape below stays linear.
fn gen_nested_blocks(depth: usize) -> String {
    let mut s = String::from("(module\n  (func $f\n");
    for i in 0..depth {
        s.push_str(&format!("    block $b{i}\n"));
    }
    s.push_str("    nop\n");
    for _ in 0..depth {
        s.push_str("    end\n");
    }
    s.push_str("))");
    s
}

/// A single function body with `width` shallow sibling blocks. Stresses the
/// many-siblings case rather than deep nesting.
fn gen_sibling_blocks(width: usize) -> String {
    let mut s = String::from("(module\n  (func $f\n");
    for i in 0..width {
        s.push_str(&format!("    block $b{i} nop end\n"));
    }
    s.push_str("))");
    s
}

/// Resolve instruction context for every identifier in the document, mirroring
/// how hover/completion classify nodes. Takes a pre-parsed tree so the
/// benchmark isolates the context-resolution (ancestor-walk) cost from parsing.
/// Returns the number resolved.
fn resolve_all_contexts(tree: &tree_sitter::Tree, content: &str) -> usize {
    let mut count = 0usize;
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "identifier" {
            black_box(determine_instruction_context(node, content));
            count += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    count
}

fn bench_doc_comment_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("doc_comment_scaling");
    // Doubling function count should roughly double time (linear), not quadruple.
    for n in [100usize, 200, 400, 800] {
        let content = gen_many_functions(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("functions", n), &content, |b, content| {
            b.iter(|| black_box(parse_document(content)));
        });
    }
    group.finish();
}

fn bench_context_nested_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_nested_blocks");
    for depth in [50usize, 100, 200, 400] {
        let content = gen_nested_blocks(depth);
        let mut parser = create_parser();
        let tree = parser.parse(&content, None).unwrap();
        group.throughput(Throughput::Elements(depth as u64));
        group.bench_with_input(
            BenchmarkId::new("depth", depth),
            &(tree, content),
            |b, (tree, content)| {
                b.iter(|| black_box(resolve_all_contexts(tree, content)));
            },
        );
    }
    group.finish();
}

fn bench_context_sibling_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_sibling_blocks");
    for width in [100usize, 200, 400, 800] {
        let content = gen_sibling_blocks(width);
        let mut parser = create_parser();
        let tree = parser.parse(&content, None).unwrap();
        group.throughput(Throughput::Elements(width as u64));
        group.bench_with_input(
            BenchmarkId::new("width", width),
            &(tree, content),
            |b, (tree, content)| {
                b.iter(|| black_box(resolve_all_contexts(tree, content)));
            },
        );
    }
    group.finish();
}

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
    bench_doc_comment_scaling,
    bench_context_nested_blocks,
    bench_context_sibling_blocks,
);

criterion_main!(benches);
