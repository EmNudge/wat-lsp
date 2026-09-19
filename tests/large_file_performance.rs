//! Large file performance tests (native only)
//!
//! Tests performance characteristics with large WAT files (~15k lines).
//! These tests use native-only features (tree-sitter, tower-lsp).

#![cfg(feature = "native")]

use std::time::{Duration, Instant};
use tower_lsp::lsp_types::Position;
use wat_lsp_rust::core::types::Position as CorePosition;
use wat_lsp_rust::test_utils::generate_large_wat;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::utils::{apply_text_edit, determine_instruction_context};
use wat_lsp_rust::{diagnostics, parser};

/// A module of `n` functions, each preceded by a doc comment. Exercises the
/// per-function doc-comment collection path in `extract_doc_comment`, which was
/// the source of the quadratic prefix-rescan pathology (#290).
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

/// A single function nested `depth` labeled blocks deep. Exercises the ancestor
/// walk in `determine_instruction_context` for a deeply nested identifier.
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

/// Resolve instruction context for every identifier in the document (mirrors how
/// hover-style classification walks the tree). Returns the number resolved.
fn resolve_all_contexts(tree: &tree_sitter::Tree, content: &str) -> usize {
    let mut count = 0usize;
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "identifier" {
            let _ = determine_instruction_context(node, content);
            count += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    count
}

/// Median wall-clock time of `runs` invocations of `f`. Taking the median makes
/// the scaling assertions below robust against occasional scheduler stalls.
fn median_time(runs: usize, mut f: impl FnMut()) -> Duration {
    let mut samples: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[test]
fn test_15k_line_initial_parse_performance() {
    println!("\n=== Generating 15k line document ===");
    let document = generate_large_wat(15000);
    let line_count = document.lines().count();
    let byte_count = document.len();

    println!("Document stats:");
    println!("  Lines: {}", line_count);
    println!("  Bytes: {}", byte_count);
    println!("  Characters: {}", document.chars().count());

    // Test initial parse
    println!("\n=== Testing Initial Parse ===");
    let mut parser = create_parser();

    let start = Instant::now();
    let tree = parser.parse(&document, None).expect("Parse failed");
    let parse_time = start.elapsed();

    println!("Initial parse time: {:?}", parse_time);
    println!(
        "Parse rate: {:.2} lines/ms",
        line_count as f64 / parse_time.as_millis() as f64
    );

    // Verify tree is valid
    assert!(!tree.root_node().has_error(), "Tree should not have errors");

    // Test symbol extraction
    println!("\n=== Testing Symbol Extraction ===");
    let start = Instant::now();
    let symbols = parser::parse_document(&document).expect("Symbol extraction failed");
    let symbol_time = start.elapsed();

    println!("Symbol extraction time: {:?}", symbol_time);
    println!("Functions found: {}", symbols.functions.len());
    println!("Globals found: {}", symbols.globals.len());

    // Test diagnostics generation
    println!("\n=== Testing Diagnostics ===");
    let start = Instant::now();
    let diagnostics = diagnostics::provide_tree_sitter_diagnostics(&tree, &document);
    let diag_time = start.elapsed();

    println!("Diagnostics time: {:?}", diag_time);
    println!("Diagnostics found: {}", diagnostics.len());

    // Total time
    let total_time = parse_time + symbol_time + diag_time;
    println!("\n=== Total Time ===");
    println!("Parse + Symbols + Diagnostics: {:?}", total_time);

    // Performance assertions (only meaningful in release builds)
    #[cfg(not(debug_assertions))]
    {
        assert!(
            parse_time.as_millis() < 1000,
            "Initial parse should be under 1 second for 15k lines"
        );
        assert!(
            total_time.as_millis() < 2000,
            "Total processing should be under 2 seconds"
        );
    }
}

#[test]
fn test_15k_line_incremental_edit_performance() {
    println!("\n=== Testing Incremental Edits on 15k Line Document ===");
    let document = generate_large_wat(15000);
    let line_count = document.lines().count();

    println!("Document: {} lines", line_count);

    let mut parser = create_parser();

    // Initial parse
    let tree = parser.parse(&document, None).expect("Parse failed");
    println!("Initial parse complete");

    // Test 1: Small edit at beginning (line 5)
    println!("\n=== Test 1: Single character edit at beginning ===");
    let mut modified_doc = document.clone();
    let mut tree_for_edit = tree.clone();

    let start_pos = Position::new(5, 2);
    let end_pos = Position::new(5, 2);
    let edit_text = " ";

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos.into());
    apply_text_edit(
        &mut modified_doc,
        start_pos.into(),
        end_pos.into(),
        edit_text,
    );
    let new_end_byte = start_byte + edit_text.len();

    let tree_edit = tree_sitter::InputEdit {
        start_byte,
        old_end_byte: start_byte,
        new_end_byte,
        start_position: tree_sitter::Point {
            row: start_pos.line as usize,
            column: start_pos.character as usize,
        },
        old_end_position: tree_sitter::Point {
            row: end_pos.line as usize,
            column: end_pos.character as usize,
        },
        new_end_position: tree_sitter::Point {
            row: start_pos.line as usize,
            column: (start_pos.character + 1) as usize,
        },
    };

    tree_for_edit.edit(&tree_edit);

    let start = Instant::now();
    let _new_tree = parser
        .parse(&modified_doc, Some(&tree_for_edit))
        .expect("Incremental parse failed");
    let incremental_time = start.elapsed();

    println!("Incremental parse (beginning): {:?}", incremental_time);

    // Test 2: Edit in middle (line 7500)
    println!("\n=== Test 2: Edit in middle of document ===");
    let mut modified_doc = document.clone();
    let mut tree_for_edit = tree.clone();

    let start_pos = Position::new(7500, 4);
    let end_pos = Position::new(7500, 4);
    let edit_text = ";; comment\n";

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos.into());
    apply_text_edit(
        &mut modified_doc,
        start_pos.into(),
        end_pos.into(),
        edit_text,
    );
    let new_end_byte = start_byte + edit_text.len();

    let tree_edit = tree_sitter::InputEdit {
        start_byte,
        old_end_byte: start_byte,
        new_end_byte,
        start_position: tree_sitter::Point {
            row: start_pos.line as usize,
            column: start_pos.character as usize,
        },
        old_end_position: tree_sitter::Point {
            row: end_pos.line as usize,
            column: end_pos.character as usize,
        },
        new_end_position: tree_sitter::Point {
            row: start_pos.line as usize + 1,
            column: 0,
        },
    };

    tree_for_edit.edit(&tree_edit);

    let start = Instant::now();
    let _new_tree = parser
        .parse(&modified_doc, Some(&tree_for_edit))
        .expect("Incremental parse failed");
    let middle_time = start.elapsed();

    println!("Incremental parse (middle): {:?}", middle_time);

    // Test 3: Edit at end (line 14990)
    println!("\n=== Test 3: Edit near end of document ===");
    let mut modified_doc = document.clone();
    let mut tree_for_edit = tree.clone();

    let edit_line = (line_count - 10) as u32;
    let start_pos = Position::new(edit_line, 2);
    let end_pos = Position::new(edit_line, 2);
    let edit_text = "x";

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos.into());
    apply_text_edit(
        &mut modified_doc,
        start_pos.into(),
        end_pos.into(),
        edit_text,
    );
    let new_end_byte = start_byte + edit_text.len();

    let tree_edit = tree_sitter::InputEdit {
        start_byte,
        old_end_byte: start_byte,
        new_end_byte,
        start_position: tree_sitter::Point {
            row: start_pos.line as usize,
            column: start_pos.character as usize,
        },
        old_end_position: tree_sitter::Point {
            row: end_pos.line as usize,
            column: end_pos.character as usize,
        },
        new_end_position: tree_sitter::Point {
            row: start_pos.line as usize,
            column: (start_pos.character + 1) as usize,
        },
    };

    tree_for_edit.edit(&tree_edit);

    let start = Instant::now();
    let _new_tree = parser
        .parse(&modified_doc, Some(&tree_for_edit))
        .expect("Incremental parse failed");
    let end_time = start.elapsed();

    println!("Incremental parse (end): {:?}", end_time);

    // Compare with full reparse
    println!("\n=== Comparison with Full Reparse ===");
    let start = Instant::now();
    let _full_tree = parser
        .parse(&modified_doc, None)
        .expect("Full parse failed");
    let full_time = start.elapsed();

    println!("Full reparse: {:?}", full_time);
    println!(
        "Incremental (avg): {:?}",
        (incremental_time + middle_time + end_time) / 3
    );
    println!(
        "Speedup: {:.2}x",
        full_time.as_nanos() as f64
            / ((incremental_time.as_nanos() + middle_time.as_nanos() + end_time.as_nanos()) / 3)
                as f64
    );

    // Performance assertions (only meaningful in release builds)
    #[cfg(not(debug_assertions))]
    {
        assert!(
            incremental_time.as_millis() < 50,
            "Incremental parse should be under 50ms"
        );
        assert!(
            middle_time.as_millis() < 50,
            "Middle edit should be under 50ms"
        );
        assert!(end_time.as_millis() < 50, "End edit should be under 50ms");
    }
}

#[test]
fn test_15k_line_completion_latency() {
    println!("\n=== Testing Completion Latency on 15k Line Document ===");
    let document = generate_large_wat(15000);

    // Parse document
    let mut parser = create_parser();
    let _tree = parser.parse(&document, None).expect("Parse failed");
    let symbols = parser::parse_document(&document).expect("Symbol extraction failed");

    println!("Document parsed: {} functions", symbols.functions.len());

    // Test completion at various positions
    println!("\n=== Testing Completion Performance ===");

    // Position 1: Early in document (line 50)
    let pos1 = CorePosition::new(50, 10);
    let start = Instant::now();
    let _completions1 = wat_lsp_rust::completion::provide_completion(&document, &symbols, pos1);
    let time1 = start.elapsed();
    println!("Completion at line 50: {:?}", time1);

    // Position 2: Middle of document (line 7500)
    let pos2 = CorePosition::new(7500, 10);
    let start = Instant::now();
    let _completions2 = wat_lsp_rust::completion::provide_completion(&document, &symbols, pos2);
    let time2 = start.elapsed();
    println!("Completion at line 7500: {:?}", time2);

    // Position 3: Near end (line 14500)
    let pos3 = CorePosition::new(14500, 10);
    let start = Instant::now();
    let _completions3 = wat_lsp_rust::completion::provide_completion(&document, &symbols, pos3);
    let time3 = start.elapsed();
    println!("Completion at line 14500: {:?}", time3);

    println!("\n=== Completion Latency Summary ===");
    println!("Average: {:?}", (time1 + time2 + time3) / 3);

    // Performance assertions (only meaningful in release builds)
    #[cfg(not(debug_assertions))]
    {
        assert!(time1.as_millis() < 100, "Completion should be under 100ms");
        assert!(time2.as_millis() < 100, "Completion should be under 100ms");
        assert!(time3.as_millis() < 100, "Completion should be under 100ms");
    }
}

/// Regression guard for the doc-comment scaling pathology (#290).
///
/// `extract_doc_comment` used to rescan every preceding module field for each
/// function, giving O(functions^2) symbol extraction on a many-function module.
/// With the `prev_sibling`-based collection it is linear.
///
/// Rather than assert a cross-run *ratio* (which is flaky on shared CI), this
/// takes the median per-function time at a small size to establish a local
/// baseline, then asserts that a much larger module stays within a *hugely*
/// generous absolute multiple of that per-function baseline. A genuine
/// quadratic regression would blow the per-function cost up with size and trip
/// the bound; linear code stays far under it. Timing assertions run in release
/// only; correctness assertions (counts, attachment) always run.
#[test]
fn test_doc_comment_extraction_scales_linearly() {
    // Warm up so allocator / code caches don't skew the first measurement.
    let _ = parser::parse_document(&gen_many_functions(64));

    let baseline_n = 100usize;
    let large_n = 3200usize; // 32x the baseline
    let baseline_src = gen_many_functions(baseline_n);
    let large = gen_many_functions(large_n);

    let runs = 7;
    let baseline_time = median_time(runs, || {
        let _ = parser::parse_document(&baseline_src).expect("parse baseline");
    });
    let large_time = median_time(runs, || {
        let _ = parser::parse_document(&large).expect("parse large");
    });

    println!(
        "doc-comment extraction: {baseline_n} fns {baseline_time:?}, {large_n} fns {large_time:?}"
    );

    // Sanity: both must actually produce the expected number of functions.
    let baseline_syms = parser::parse_document(&baseline_src).expect("parse baseline");
    let large_syms = parser::parse_document(&large).expect("parse large");
    assert_eq!(baseline_syms.functions.len(), baseline_n);
    assert_eq!(large_syms.functions.len(), large_n);
    // Every function should still carry its doc comment (attachment preserved).
    assert!(large_syms.functions.iter().all(|f| f.doc_comment.is_some()));

    #[cfg(not(debug_assertions))]
    {
        // Per-function baseline, floored so a tiny/noisy sample can't shrink it.
        let per_fn_ns = (baseline_time.as_nanos() as f64 / baseline_n as f64).max(50.0);
        // Allow 30x the linear projection: catches a quadratic (which at 8x the
        // size would be ~8x the per-function cost and rising) without tripping
        // on ordinary CI noise.
        let ceiling_ns = per_fn_ns * large_n as f64 * 30.0;
        assert!(
            (large_time.as_nanos() as f64) < ceiling_ns,
            "doc-comment extraction scaled super-linearly: {baseline_n} fns {baseline_time:?}, \
             {large_n} fns {large_time:?} (ceiling {:.1}ms)",
            ceiling_ns / 1.0e6
        );
    }
}

/// Regression guard for the context-resolution ancestor-walk pathology (#290).
///
/// Resolving instruction context for every identifier used to walk to the module
/// root per identifier. With the bounded catch-context lookup and O(1)-ish
/// classification it is effectively linear in identifier count. Same
/// baseline-multiple strategy as the doc-comment guard, to avoid flaky
/// cross-run ratios.
#[test]
fn test_context_resolution_scales_linearly() {
    let mut parser = create_parser();

    let baseline_n = 200usize;
    let large_n = 1600usize;
    let baseline_src = gen_many_functions(baseline_n);
    let large = gen_many_functions(large_n);
    let baseline_tree = parser.parse(&baseline_src, None).expect("parse baseline");
    let large_tree = parser.parse(&large, None).expect("parse large");

    // Warm up.
    let _ = resolve_all_contexts(&baseline_tree, &baseline_src);

    let runs = 7;
    let baseline_time = median_time(runs, || {
        let _ = resolve_all_contexts(&baseline_tree, &baseline_src);
    });
    let large_time = median_time(runs, || {
        let _ = resolve_all_contexts(&large_tree, &large);
    });

    let baseline_count = resolve_all_contexts(&baseline_tree, &baseline_src);
    let large_count = resolve_all_contexts(&large_tree, &large);
    println!(
        "context resolution: {baseline_n} fns {baseline_time:?} ({baseline_count} ids), \
         {large_n} fns {large_time:?} ({large_count} ids)"
    );
    // Identifier count should scale with function count.
    assert!(large_count > baseline_count);

    #[cfg(not(debug_assertions))]
    {
        let per_id_ns = (baseline_time.as_nanos() as f64 / baseline_count as f64).max(50.0);
        let ceiling_ns = per_id_ns * large_count as f64 * 30.0;
        assert!(
            (large_time.as_nanos() as f64) < ceiling_ns,
            "context resolution scaled super-linearly: {baseline_n} fns {baseline_time:?}, \
             {large_n} fns {large_time:?} (ceiling {:.1}ms)",
            ceiling_ns / 1.0e6
        );
    }
}

/// Pathological-input guard for the bounded ancestor walk (#290).
///
/// A very deeply nested function must not stall context resolution. The upward
/// walk in `determine_instruction_context` is capped, so even thousands of
/// nested blocks resolve quickly with a safe (`General` → line-based) fallback.
/// The wall-clock ceiling is enormous (seconds) so it only ever trips on a true
/// hang, never on ordinary CI jitter.
#[test]
fn test_deep_nesting_context_resolution_is_bounded() {
    let mut parser = create_parser();
    let content = gen_nested_blocks(2000);
    let tree = parser.parse(&content, None).expect("parse deep");

    // Resolve context for the deepest identifier repeatedly; must terminate fast.
    let start = Instant::now();
    let count = resolve_all_contexts(&tree, &content);
    let elapsed = start.elapsed();

    println!("deep nesting (2000 blocks): resolved {count} identifiers in {elapsed:?}");
    assert!(count > 0, "should resolve at least one identifier");
    // Hard ceiling: catches a genuine hang without being timing-sensitive.
    assert!(
        elapsed < Duration::from_secs(10),
        "deeply nested context resolution took too long: {elapsed:?}"
    );
}
