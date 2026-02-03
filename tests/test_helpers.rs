//! Shared test utilities for WAT LSP tests and benchmarks.
//!
//! This module provides common helpers used across integration tests and benchmarks.

/// Generate a synthetic WAT file with the specified approximate line count.
///
/// The generated file contains:
/// - Globals (10 or num_lines/1500, whichever is smaller)
/// - Type definitions (5 or num_lines/3000, whichever is smaller)
/// - Functions (~15 lines each)
///
/// # Arguments
/// * `target_lines` - Approximate target line count for the generated file
///
/// # Example
/// ```
/// let wat = generate_large_wat(15000);
/// assert!(wat.lines().count() > 14000);
/// ```
pub fn generate_large_wat(target_lines: usize) -> String {
    let mut wat = String::from("(module\n");

    // Calculate how many functions we need (each function is ~15 lines)
    let num_functions = target_lines / 15;
    let num_globals = 10.min(num_functions / 10);
    let num_types = 5.min(num_functions / 20);

    // Add globals
    for i in 0..num_globals {
        wat.push_str(&format!(
            "  (global $global{} (mut i32) (i32.const {}))\n",
            i, i
        ));
    }

    // Add types
    for i in 0..num_types {
        wat.push_str(&format!(
            "  (type $type{} (func (param i32 i32) (result i32)))\n",
            i
        ));
    }

    // Add functions
    for i in 0..num_functions {
        wat.push_str(&format!(
            "  (func $func{} (param $x i32) (param $y i32) (result i32)\n",
            i
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
        if num_globals > 0 {
            wat.push_str("    ;; Add global value\n");
            wat.push_str("    (local.set $result\n");
            wat.push_str(&format!(
                "      (i32.add (local.get $result) (global.get $global{})))\n",
                i % num_globals
            ));
        }
        wat.push_str("    (local.get $result))\n");
    }

    wat.push_str(")\n");
    wat
}
