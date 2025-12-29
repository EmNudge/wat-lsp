use std::time::Instant;
use tower_lsp::lsp_types::Position;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::utils::apply_text_edit;
use wat_lsp_rust::{parser, diagnostics};

/// Generate a large WAT document (approximately 15k lines)
fn generate_large_wat_15k() -> String {
    let mut wat = String::from("(module\n");

    // Add 10 globals (10 lines)
    for i in 0..10 {
        wat.push_str(&format!("  (global $global{} (mut i32) (i32.const {}))\n", i, i));
    }

    // Add 5 tables (5 lines)
    for i in 0..5 {
        wat.push_str(&format!("  (table $table{} 100 funcref)\n", i));
    }

    // Add 10 type definitions (30 lines)
    for i in 0..10 {
        wat.push_str(&format!("  (type $type{} (func (param i32 i32) (result i32)))\n", i));
    }

    // Add 1000 functions with ~15 lines each = ~15,000 lines
    for i in 0..1000 {
        wat.push_str(&format!(
            "  (func $func{} (param $x i32) (param $y i32) (result i32)\n", i
        ));
        wat.push_str("    (local $temp i32)\n");
        wat.push_str("    (local $result i32)\n");
        wat.push_str("    ;; Initialize local\n");
        wat.push_str("    (local.set $temp (i32.const 0))\n");
        wat.push_str("    ;; Calculate sum\n");
        wat.push_str("    (local.set $temp\n");
        wat.push_str("      (i32.add (local.get $x) (local.get $y)))\n");
        wat.push_str("    ;; Double the result\n");
        wat.push_str("    (local.set $result\n");
        wat.push_str("      (i32.mul (local.get $temp) (i32.const 2)))\n");
        wat.push_str("    ;; Add global value\n");
        wat.push_str("    (local.set $result\n");
        wat.push_str(&format!("      (i32.add (local.get $result) (global.get $global{})))\n", i % 10));
        wat.push_str("    (local.get $result))\n");
    }

    wat.push_str(")\n");
    wat
}

#[test]
fn test_15k_line_initial_parse_performance() {
    println!("\n=== Generating 15k line document ===");
    let document = generate_large_wat_15k();
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
    println!("Parse rate: {:.2} lines/ms", line_count as f64 / parse_time.as_millis() as f64);

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
    println!("Tables found: {}", symbols.tables.len());

    // Test diagnostics generation
    println!("\n=== Testing Diagnostics ===");
    let start = Instant::now();
    let diagnostics = diagnostics::provide_diagnostics(&tree, &document);
    let diag_time = start.elapsed();

    println!("Diagnostics time: {:?}", diag_time);
    println!("Diagnostics found: {}", diagnostics.len());

    // Total time
    let total_time = parse_time + symbol_time + diag_time;
    println!("\n=== Total Time ===");
    println!("Parse + Symbols + Diagnostics: {:?}", total_time);

    // Performance assertions
    assert!(parse_time.as_millis() < 1000, "Initial parse should be under 1 second for 15k lines");
    assert!(total_time.as_millis() < 2000, "Total processing should be under 2 seconds");
}

#[test]
fn test_15k_line_incremental_edit_performance() {
    println!("\n=== Testing Incremental Edits on 15k Line Document ===");
    let document = generate_large_wat_15k();
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

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos);
    apply_text_edit(&mut modified_doc, start_pos, end_pos, edit_text);
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
    let _new_tree = parser.parse(&modified_doc, Some(&tree_for_edit)).expect("Incremental parse failed");
    let incremental_time = start.elapsed();

    println!("Incremental parse (beginning): {:?}", incremental_time);

    // Test 2: Edit in middle (line 7500)
    println!("\n=== Test 2: Edit in middle of document ===");
    let mut modified_doc = document.clone();
    let mut tree_for_edit = tree.clone();

    let start_pos = Position::new(7500, 4);
    let end_pos = Position::new(7500, 4);
    let edit_text = ";; comment\n";

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos);
    apply_text_edit(&mut modified_doc, start_pos, end_pos, edit_text);
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
    let _new_tree = parser.parse(&modified_doc, Some(&tree_for_edit)).expect("Incremental parse failed");
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

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos);
    apply_text_edit(&mut modified_doc, start_pos, end_pos, edit_text);
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
    let _new_tree = parser.parse(&modified_doc, Some(&tree_for_edit)).expect("Incremental parse failed");
    let end_time = start.elapsed();

    println!("Incremental parse (end): {:?}", end_time);

    // Compare with full reparse
    println!("\n=== Comparison with Full Reparse ===");
    let start = Instant::now();
    let _full_tree = parser.parse(&modified_doc, None).expect("Full parse failed");
    let full_time = start.elapsed();

    println!("Full reparse: {:?}", full_time);
    println!("Incremental (avg): {:?}", (incremental_time + middle_time + end_time) / 3);
    println!("Speedup: {:.2}x",
        full_time.as_nanos() as f64 / ((incremental_time.as_nanos() + middle_time.as_nanos() + end_time.as_nanos()) / 3) as f64
    );

    // Performance assertions
    assert!(incremental_time.as_millis() < 50, "Incremental parse should be under 50ms");
    assert!(middle_time.as_millis() < 50, "Middle edit should be under 50ms");
    assert!(end_time.as_millis() < 50, "End edit should be under 50ms");
}

#[test]
fn test_15k_line_completion_latency() {
    println!("\n=== Testing Completion Latency on 15k Line Document ===");
    let document = generate_large_wat_15k();

    // Parse document
    let mut parser = create_parser();
    let tree = parser.parse(&document, None).expect("Parse failed");
    let symbols = parser::parse_document(&document).expect("Symbol extraction failed");

    println!("Document parsed: {} functions", symbols.functions.len());

    // Test completion at various positions
    println!("\n=== Testing Completion Performance ===");

    // Position 1: Early in document (line 50)
    let pos1 = Position::new(50, 10);
    let start = Instant::now();
    let _completions1 = wat_lsp_rust::completion::provide_completion(&document, &symbols, &tree, pos1);
    let time1 = start.elapsed();
    println!("Completion at line 50: {:?}", time1);

    // Position 2: Middle of document (line 7500)
    let pos2 = Position::new(7500, 10);
    let start = Instant::now();
    let _completions2 = wat_lsp_rust::completion::provide_completion(&document, &symbols, &tree, pos2);
    let time2 = start.elapsed();
    println!("Completion at line 7500: {:?}", time2);

    // Position 3: Near end (line 14500)
    let pos3 = Position::new(14500, 10);
    let start = Instant::now();
    let _completions3 = wat_lsp_rust::completion::provide_completion(&document, &symbols, &tree, pos3);
    let time3 = start.elapsed();
    println!("Completion at line 14500: {:?}", time3);

    println!("\n=== Completion Latency Summary ===");
    println!("Average: {:?}", (time1 + time2 + time3) / 3);

    // Performance assertions
    assert!(time1.as_millis() < 100, "Completion should be under 100ms");
    assert!(time2.as_millis() < 100, "Completion should be under 100ms");
    assert!(time3.as_millis() < 100, "Completion should be under 100ms");
}
