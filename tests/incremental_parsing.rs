use std::time::Instant;
use tower_lsp::lsp_types::Position;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::utils::apply_text_edit;

/// Generate a large WAT document for performance testing
fn generate_large_wat(num_functions: usize) -> String {
    let mut wat = String::from("(module\n");

    for i in 0..num_functions {
        wat.push_str(&format!(
            "  (func $func{} (param $x i32) (param $y i32) (result i32)\n",
            i
        ));
        wat.push_str("    (local $temp i32)\n");
        wat.push_str("    (local.set $temp\n");
        wat.push_str("      (i32.add (local.get $x) (local.get $y)))\n");
        wat.push_str("    (i32.mul (local.get $temp) (i32.const 2)))\n");
    }

    wat.push_str(")\n");
    wat
}

#[test]
fn test_incremental_vs_full_parsing() {
    let document = generate_large_wat(100);
    let mut parser = create_parser();

    // Initial parse
    let tree = parser.parse(&document, None).expect("Initial parse failed");

    // Measure full reparse time
    let start = Instant::now();
    let _full_reparse = parser.parse(&document, None).expect("Full reparse failed");
    let full_time = start.elapsed();

    // Measure incremental reparse time
    let mut tree_for_incremental = tree.clone();
    let mut modified_doc = document.clone();

    // Make a small edit at the beginning
    let start_pos = Position::new(1, 2);
    let end_pos = Position::new(1, 2);
    let edit_text = "  ;; comment\n";

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos);
    let old_end_byte = start_byte;

    apply_text_edit(&mut modified_doc, start_pos, end_pos, edit_text);
    let new_end_byte = start_byte + edit_text.len();
    let new_end = Position::new(2, 0);

    // Apply tree edit
    let tree_edit = tree_sitter::InputEdit {
        start_byte,
        old_end_byte,
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
            row: new_end.line as usize,
            column: new_end.character as usize,
        },
    };

    tree_for_incremental.edit(&tree_edit);

    let start = Instant::now();
    let _incremental_reparse = parser
        .parse(&modified_doc, Some(&tree_for_incremental))
        .expect("Incremental reparse failed");
    let incremental_time = start.elapsed();

    println!("\n=== Performance Comparison ===");
    println!("Document size: {} functions, {} bytes", 100, document.len());
    println!("Full reparse time: {:?}", full_time);
    println!("Incremental reparse time: {:?}", incremental_time);
    println!(
        "Speedup: {:.2}x",
        full_time.as_nanos() as f64 / incremental_time.as_nanos() as f64
    );

    // Incremental parsing should be at least as fast as full parsing
    // (in practice it's usually faster, but on small edits it might be similar)
    assert!(
        incremental_time <= full_time * 2,
        "Incremental parsing should not be significantly slower than full parsing"
    );
}

#[test]
fn test_incremental_parsing_correctness() {
    let document = "(func $test (param $x i32)\n  (local.get $x))";
    let mut parser = create_parser();

    // Initial parse
    let tree = parser.parse(document, None).expect("Initial parse failed");
    let _initial_text = tree.root_node().to_sexp();

    // Apply an edit
    let mut modified_doc = document.to_string();
    let start_pos = Position::new(1, 2);
    let end_pos = Position::new(1, 2);
    let edit_text = "(i32.add ";

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos);
    let old_end_byte = start_byte;

    apply_text_edit(&mut modified_doc, start_pos, end_pos, edit_text);
    let new_end_byte = start_byte + edit_text.len();
    let new_end = Position::new(1, 2 + edit_text.len() as u32);

    // Create tree edit
    let mut tree_for_incremental = tree.clone();
    let tree_edit = tree_sitter::InputEdit {
        start_byte,
        old_end_byte,
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
            row: new_end.line as usize,
            column: new_end.character as usize,
        },
    };

    tree_for_incremental.edit(&tree_edit);

    // Incremental reparse
    let incremental_tree = parser
        .parse(&modified_doc, Some(&tree_for_incremental))
        .expect("Incremental reparse failed");

    // Full reparse for comparison
    let full_tree = parser
        .parse(&modified_doc, None)
        .expect("Full reparse failed");

    // Both should produce the same tree
    assert_eq!(
        incremental_tree.root_node().to_sexp(),
        full_tree.root_node().to_sexp(),
        "Incremental and full parsing should produce identical results"
    );
}

#[test]
fn test_multiple_incremental_edits() {
    let mut document = "(func $test\n)".to_string();
    let mut parser = create_parser();

    // Initial parse
    let mut tree = parser.parse(&document, None).expect("Initial parse failed");

    // Apply multiple edits
    let edits = vec![
        (Position::new(1, 0), Position::new(1, 0), "  (param $x i32)\n"),
        (Position::new(2, 0), Position::new(2, 0), "  (result i32)\n"),
        (Position::new(3, 0), Position::new(3, 0), "  (local.get $x)\n"),
    ];

    for (start, end, text) in edits {
        let start_byte = wat_lsp_rust::utils::position_to_byte(&document, start);
        let old_end_byte = wat_lsp_rust::utils::position_to_byte(&document, end);

        let new_end = apply_text_edit(&mut document, start, end, text);
        let new_end_byte = start_byte + text.len();

        // Create and apply tree edit
        let tree_edit = tree_sitter::InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position: tree_sitter::Point {
                row: start.line as usize,
                column: start.character as usize,
            },
            old_end_position: tree_sitter::Point {
                row: end.line as usize,
                column: end.character as usize,
            },
            new_end_position: tree_sitter::Point {
                row: new_end.line as usize,
                column: new_end.character as usize,
            },
        };

        tree.edit(&tree_edit);

        // Incremental reparse
        tree = parser
            .parse(&document, Some(&tree))
            .expect("Incremental reparse failed");
    }

    // Verify final document is valid
    assert!(document.contains("param $x i32"));
    assert!(document.contains("result i32"));
    assert!(document.contains("local.get $x"));

    // Verify tree has no errors
    let root = tree.root_node();
    assert!(!root.has_error(), "Final tree should not have errors");
}
