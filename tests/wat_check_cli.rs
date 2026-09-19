//! CLI exit-code and diagnostic-level tests for the `wat-check` binary.
//!
//! These pin the issue #292 contract that `wat-check` defaults to the full
//! pipeline (syntax + semantic + wast deep validation) while still allowing an
//! explicit narrower `--level`, and that the exit code reflects whether errors
//! were found.

#![cfg(feature = "native")]

use std::io::Write;
use std::process::{Command, Output, Stdio};

const CHECK_BIN: &str = env!("CARGO_BIN_EXE_wat-check");

/// Run wat-check with `stdin` piped in, returning the full output.
fn run_stdin(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(CHECK_BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn wat-check");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child
        .wait_with_output()
        .expect("failed to wait for wat-check")
}

/// A well-formed, valid module exits 0 with no diagnostics.
#[test]
fn valid_module_exits_zero() {
    let out = run_stdin(&["-"], "(module (func (result i32) (i32.const 0)))");
    assert!(
        out.status.success(),
        "valid module should exit 0, got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A module with an error exits 1.
#[test]
fn errored_module_exits_one() {
    // Syntactically parseable but type-invalid: result i32 but body pushes i64.
    let out = run_stdin(&["-"], "(module (func (result i32) (i64.const 0)))");
    assert_eq!(
        out.status.code(),
        Some(1),
        "a module with errors should exit 1, got {:?}\nstdout: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
}

/// The default level is the full pipeline: a type-mismatch error (a
/// validation-layer diagnostic) is reported without needing `--level full`.
/// Under `--level syntax` the same input is clean, proving the default is *not*
/// syntax-only and that explicit level selection still narrows the checks.
#[test]
fn default_level_runs_full_pipeline() {
    let src = "(module (func (result i32) (i64.const 0)))";

    // Default (no --level): the type mismatch must be caught -> exit 1.
    let default_out = run_stdin(&["-"], src);
    assert_eq!(
        default_out.status.code(),
        Some(1),
        "default level should catch the type mismatch, got {:?}",
        default_out.status
    );

    // Explicit syntax level: no syntax error here, so it is clean -> exit 0.
    let syntax_out = run_stdin(&["--level", "syntax", "-"], src);
    assert!(
        syntax_out.status.success(),
        "syntax level should not flag the type mismatch, got {:?}",
        syntax_out.status
    );
}

/// Explicit `--level full` matches the default behavior for a validation error.
#[test]
fn explicit_full_level_matches_default() {
    let src = "(module (func (result i32) (i64.const 0)))";
    let out = run_stdin(&["--level", "full", "-"], src);
    assert_eq!(out.status.code(), Some(1));
}

/// A hard syntax error is caught at every level (exit 1).
#[test]
fn syntax_error_exits_one_at_syntax_level() {
    // Unknown/misspelled operator produces a tree-sitter syntax error.
    let out = run_stdin(&["--level", "syntax", "-"], "(module (func (bogusop 3)))");
    assert_eq!(
        out.status.code(),
        Some(1),
        "a syntax error should exit 1 even at syntax level, got {:?}\nstdout: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
}
