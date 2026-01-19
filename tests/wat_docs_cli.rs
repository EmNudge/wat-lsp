//! Integration tests for the wat-docs CLI binary
//!
//! These tests verify the CLI behavior including output formatting and colorization.

use std::process::Command;

/// Get the path to the wat-docs binary
fn wat_docs_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wat-docs"))
}

// ANSI escape code constants for verification
mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const BRIGHT_RED: &str = "\x1b[91m";
}

#[test]
fn test_list_command() {
    let output = wat_docs_bin()
        .args(["list"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // Should have many instructions
    assert!(
        lines.len() > 100,
        "Expected many instructions, got {}",
        lines.len()
    );

    // Should include common instructions
    assert!(lines.contains(&"i32.add"));
    assert!(lines.contains(&"memory.grow"));
    assert!(lines.contains(&"call"));
}

#[test]
fn test_list_with_filter() {
    let output = wat_docs_bin()
        .args(["list", "--filter", "i32.a"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // All results should contain "i32.a"
    for line in &lines {
        assert!(
            line.contains("i32.a"),
            "Expected all results to contain 'i32.a', got '{}'",
            line
        );
    }

    // Should include i32.add
    assert!(lines.contains(&"i32.add"));
}

#[test]
fn test_show_command_raw() {
    let output = wat_docs_bin()
        .args(["show", "i32.add", "--raw"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain documentation content
    assert!(stdout.to_lowercase().contains("add"));
    assert!(stdout.contains("i32"));

    // Raw mode should NOT contain ANSI codes
    assert!(
        !stdout.contains("\x1b["),
        "Raw mode should not contain ANSI escape codes"
    );
}

#[test]
fn test_show_command_no_color() {
    let output = wat_docs_bin()
        .args(["show", "i32.add", "--no-color"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain header
    assert!(stdout.contains("## i32.add"));

    // No-color mode should NOT contain ANSI codes
    assert!(
        !stdout.contains("\x1b["),
        "No-color mode should not contain ANSI escape codes"
    );
}

#[test]
fn test_show_command_with_color() {
    let output = wat_docs_bin()
        .args(["show", "i32.add", "--color"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Color mode SHOULD contain ANSI codes
    assert!(
        stdout.contains("\x1b["),
        "Color mode should contain ANSI escape codes"
    );

    // Should have reset codes
    assert!(stdout.contains(ansi::RESET), "Should contain RESET codes");

    // Header should be cyan and bold
    assert!(stdout.contains(ansi::CYAN), "Header should be cyan");
    assert!(stdout.contains(ansi::BOLD), "Header should be bold");
}

#[test]
fn test_show_command_colorizes_code_blocks() {
    let output = wat_docs_bin()
        .args(["show", "func", "--color"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Code blocks should have syntax highlighting
    // Keywords like "func", "param", "result" should be magenta
    assert!(
        stdout.contains(ansi::MAGENTA),
        "Keywords should be highlighted in magenta"
    );

    // Identifiers like $add should be yellow
    assert!(
        stdout.contains(ansi::YELLOW),
        "Identifiers should be highlighted in yellow"
    );

    // Types should be cyan
    assert!(
        stdout.contains(ansi::CYAN),
        "Types should be highlighted in cyan"
    );

    // Code fence should be dimmed
    assert!(stdout.contains(ansi::DIM), "Code fences should be dimmed");
}

#[test]
fn test_show_command_colorizes_numbers() {
    let output = wat_docs_bin()
        .args(["show", "i32.const", "--color"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Numbers should be bright red
    assert!(
        stdout.contains(ansi::BRIGHT_RED),
        "Numbers should be highlighted in bright red"
    );
}

#[test]
fn test_show_command_colorizes_comments() {
    let output = wat_docs_bin()
        .args(["show", "i32.add", "--color"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Comments should be dimmed (the example has ";; Returns 8")
    assert!(stdout.contains(ansi::DIM), "Comments should be dimmed");
}

#[test]
fn test_show_unknown_instruction() {
    let output = wat_docs_bin()
        .args(["show", "not.an.instruction"])
        .output()
        .expect("Failed to execute wat-docs");

    // Should fail
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown instruction"));
}

#[test]
fn test_show_suggests_similar() {
    let output = wat_docs_bin()
        .args(["show", "i32.ad"]) // Typo - missing 'd'
        .output()
        .expect("Failed to execute wat-docs");

    // Should fail but suggest alternatives
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Did you mean"));
    assert!(stderr.contains("i32.add"));
}

#[test]
fn test_search_command() {
    let output = wat_docs_bin()
        .args(["search", "memory"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should find memory-related instructions
    assert!(stdout.contains("memory"));
}

#[test]
fn test_search_names_only() {
    let output = wat_docs_bin()
        .args(["search", "i32", "--names-only"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // All lines should be instruction names containing "i32"
    for line in &lines {
        assert!(
            line.contains("i32"),
            "All results should contain 'i32', got '{}'",
            line
        );
        // Names-only should not have ": " separator (no description)
        assert!(
            !line.contains(": "),
            "Names-only should not include descriptions"
        );
    }
}

#[test]
fn test_search_no_results() {
    let output = wat_docs_bin()
        .args(["search", "xyznotfound123"])
        .output()
        .expect("Failed to execute wat-docs");

    // Should fail when no results
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No instructions found"));
}

#[test]
fn test_search_case_insensitive() {
    let output_lower = wat_docs_bin()
        .args(["search", "memory", "--names-only"])
        .output()
        .expect("Failed to execute wat-docs");

    let output_upper = wat_docs_bin()
        .args(["search", "MEMORY", "--names-only"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output_lower.status.success());
    assert!(output_upper.status.success());

    // Results should be identical
    assert_eq!(output_lower.stdout, output_upper.stdout);
}

#[test]
fn test_shell_setup_bash() {
    let output = wat_docs_bin()
        .args(["shell-setup", "bash"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain bash function definitions
    assert!(stdout.contains("wat-browse()"));
    assert!(stdout.contains("wat-search()"));
    assert!(stdout.contains("fzf"));
}

#[test]
fn test_shell_setup_fish() {
    let output = wat_docs_bin()
        .args(["shell-setup", "fish"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain fish function definitions
    assert!(stdout.contains("function wat-browse"));
    assert!(stdout.contains("function wat-search"));
    assert!(stdout.contains("fzf"));
}

#[test]
fn test_help_output() {
    let output = wat_docs_bin()
        .args(["--help"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show usage information
    assert!(stdout.contains("wat-docs"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("search"));
}

#[test]
fn test_signature_highlighted() {
    let output = wat_docs_bin()
        .args(["show", "i32.add", "--color"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Signature line should be yellow and bold
    assert!(
        stdout.contains(&format!("{}{}Signature:", ansi::BOLD, ansi::YELLOW)),
        "Signature should be bold yellow"
    );
}

#[test]
fn test_example_highlighted() {
    let output = wat_docs_bin()
        .args(["show", "i32.add", "--color"])
        .output()
        .expect("Failed to execute wat-docs");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Example line should be green and bold
    assert!(
        stdout.contains(&format!("{}{}Example:", ansi::BOLD, ansi::GREEN)),
        "Example should be bold green"
    );
}
