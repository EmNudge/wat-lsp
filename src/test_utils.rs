/// Generate a synthetic WAT file with the specified approximate line count.
///
/// The generated file contains globals, type definitions, and functions (~15 lines each).
/// Functions include cross-function calls for realistic reference benchmarks.
pub fn generate_large_wat(target_lines: usize) -> String {
    let mut wat = String::from("(module\n");

    let num_functions = target_lines / 15;
    let num_globals = 10.min(num_functions / 10);
    let num_types = 5.min(num_functions / 20);

    for i in 0..num_globals {
        wat.push_str(&format!(
            "  (global $global{} (mut i32) (i32.const {}))\n",
            i, i
        ));
    }

    for i in 0..num_types {
        wat.push_str(&format!(
            "  (type $type{} (func (param i32 i32) (result i32)))\n",
            i
        ));
    }

    for i in 0..num_functions {
        wat.push_str(&format!(
            "  (func $func{} (param $x i32) (param $y i32) (result i32)\n",
            i
        ));
        wat.push_str("    (local $temp i32)\n");
        wat.push_str("    (local $result i32)\n");
        wat.push_str("    (local.set $temp (i32.const 0))\n");
        wat.push_str("    (local.set $temp\n");
        wat.push_str("      (i32.add (local.get $x) (local.get $y)))\n");
        wat.push_str("    (local.set $result\n");
        wat.push_str("      (i32.mul (local.get $temp) (i32.const 2)))\n");
        if num_globals > 0 {
            wat.push_str("    (local.set $result\n");
            wat.push_str(&format!(
                "      (i32.add (local.get $result) (global.get $global{})))\n",
                i % num_globals
            ));
        }
        if i > 0 {
            wat.push_str(&format!(
                "    (drop (call $func{}))\n",
                (i - 1) % num_functions
            ));
        }
        wat.push_str("    (local.get $result))\n");
    }

    wat.push_str(")\n");
    wat
}
