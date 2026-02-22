//! Incremental parsing tests (native only)
//!
//! Tests tree-sitter incremental parsing functionality.

#![cfg(feature = "native")]

use std::time::Instant;
use tower_lsp::lsp_types::Position;
use wat_lsp_rust::diagnostics::provide_tree_sitter_diagnostics;
use wat_lsp_rust::test_utils::generate_large_wat;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::utils::apply_text_edit;

#[test]
fn test_incremental_vs_full_parsing() {
    // Generate ~1500 lines (100 functions * ~15 lines each)
    let document = generate_large_wat(1500);
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

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos.into());
    let old_end_byte = start_byte;

    apply_text_edit(
        &mut modified_doc,
        start_pos.into(),
        end_pos.into(),
        edit_text,
    );
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
    println!(
        "Document size: {} lines, {} bytes",
        document.lines().count(),
        document.len()
    );
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

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, start_pos.into());
    let old_end_byte = start_byte;

    apply_text_edit(
        &mut modified_doc,
        start_pos.into(),
        end_pos.into(),
        edit_text,
    );
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
        (
            Position::new(1, 0),
            Position::new(1, 0),
            "  (param $x i32)\n",
        ),
        (Position::new(2, 0), Position::new(2, 0), "  (result i32)\n"),
        (
            Position::new(3, 0),
            Position::new(3, 0),
            "  (local.get $x)\n",
        ),
    ];

    for (start, end, text) in edits {
        let start_byte = wat_lsp_rust::utils::position_to_byte(&document, start.into());
        let old_end_byte = wat_lsp_rust::utils::position_to_byte(&document, end.into());

        let new_end = apply_text_edit(&mut document, start.into(), end.into(), text);
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

/// Test for issue #79: false errors appear when removing and re-adding the same character
/// https://github.com/EmNudge/wat-lsp/issues/79
#[test]
fn test_delete_and_readd_character() {
    // Start with valid code
    let original_document = "(func $test (param $x i32) (result i32)\n  (local.get $x))";
    let mut parser = create_parser();

    // Initial parse - should have no errors
    let tree = parser
        .parse(original_document, None)
        .expect("Initial parse failed");
    let diagnostics = provide_tree_sitter_diagnostics(&tree, original_document);
    assert!(
        diagnostics.is_empty(),
        "Initial document should have no syntax errors"
    );

    // STEP 1: Delete the closing ')' at position (1, 16)
    // The document is:
    // (func $test (param $x i32) (result i32)
    //   (local.get $x))
    // We want to delete the last ')' at position (1, 16)

    let mut modified_doc = original_document.to_string();
    let delete_start = Position::new(1, 16);
    let delete_end = Position::new(1, 17);

    let start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, delete_start.into());
    let old_end_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, delete_end.into());

    // Apply the delete (replace with empty string)
    let new_end = apply_text_edit(
        &mut modified_doc,
        delete_start.into(),
        delete_end.into(),
        "", // delete = replace with empty string
    );
    let new_end_byte = start_byte; // Empty insert means new_end_byte == start_byte

    // Create and apply tree edit
    let mut tree_after_delete = tree.clone();
    let tree_edit = tree_sitter::InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: tree_sitter::Point {
            row: delete_start.line as usize,
            column: delete_start.character as usize,
        },
        old_end_position: tree_sitter::Point {
            row: delete_end.line as usize,
            column: delete_end.character as usize,
        },
        new_end_position: tree_sitter::Point {
            row: new_end.line as usize,
            column: new_end.character as usize,
        },
    };
    tree_after_delete.edit(&tree_edit);

    // Reparse incrementally
    let tree_after_delete = parser
        .parse(&modified_doc, Some(&tree_after_delete))
        .expect("Incremental reparse after delete failed");

    // Verify the document now has a missing ')' - should show error
    let diagnostics_after_delete =
        provide_tree_sitter_diagnostics(&tree_after_delete, &modified_doc);
    assert!(
        !diagnostics_after_delete.is_empty() || tree_after_delete.root_node().has_error(),
        "Document with missing ')' should have syntax errors. Doc: {}",
        modified_doc
    );

    // STEP 2: Re-add the ')' at position (1, 16)
    let insert_pos = Position::new(1, 16);
    let insert_start_byte = wat_lsp_rust::utils::position_to_byte(&modified_doc, insert_pos.into());
    let insert_old_end_byte = insert_start_byte; // Insert, so old range is zero-length

    // Apply the insert
    let insert_new_end = apply_text_edit(
        &mut modified_doc,
        insert_pos.into(),
        insert_pos.into(),
        ")", // re-add the character
    );
    let insert_new_end_byte = insert_start_byte + 1;

    // Create and apply tree edit
    let mut tree_after_readd = tree_after_delete.clone();
    let tree_edit2 = tree_sitter::InputEdit {
        start_byte: insert_start_byte,
        old_end_byte: insert_old_end_byte,
        new_end_byte: insert_new_end_byte,
        start_position: tree_sitter::Point {
            row: insert_pos.line as usize,
            column: insert_pos.character as usize,
        },
        old_end_position: tree_sitter::Point {
            row: insert_pos.line as usize,
            column: insert_pos.character as usize,
        },
        new_end_position: tree_sitter::Point {
            row: insert_new_end.line as usize,
            column: insert_new_end.character as usize,
        },
    };
    tree_after_readd.edit(&tree_edit2);

    // Reparse incrementally
    let tree_after_readd = parser
        .parse(&modified_doc, Some(&tree_after_readd))
        .expect("Incremental reparse after re-add failed");

    // CRITICAL CHECK: After re-adding the character, document should be valid again
    assert_eq!(
        modified_doc, original_document,
        "Document should be restored to original"
    );

    let diagnostics_after_readd = provide_tree_sitter_diagnostics(&tree_after_readd, &modified_doc);

    // Also check that any error positions are valid
    for diag in &diagnostics_after_readd {
        let start_line = diag.range.start.line as usize;
        let end_line = diag.range.end.line as usize;
        let lines: Vec<&str> = modified_doc.lines().collect();

        assert!(
            start_line < lines.len(),
            "Diagnostic start line {} is out of bounds (doc has {} lines)",
            start_line,
            lines.len()
        );
        assert!(
            end_line < lines.len() || (end_line == lines.len() && diag.range.end.character == 0),
            "Diagnostic end line {} is out of bounds (doc has {} lines)",
            end_line,
            lines.len()
        );
    }

    assert!(
        diagnostics_after_readd.is_empty(),
        "After re-adding the deleted character, there should be no syntax errors.\nDiagnostics: {:?}\nTree has_error: {}",
        diagnostics_after_readd,
        tree_after_readd.root_node().has_error()
    );

    // Also verify that incremental parse matches full parse
    let full_tree = parser
        .parse(&modified_doc, None)
        .expect("Full reparse failed");
    assert_eq!(
        tree_after_readd.root_node().to_sexp(),
        full_tree.root_node().to_sexp(),
        "Incremental parse should match full parse after delete+re-add"
    );
}
