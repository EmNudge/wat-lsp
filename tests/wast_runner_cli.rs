//! CLI exit-code and discovery tests for the `wast-runner` conformance binary.
//!
//! These pin the "regression gate" contract: missing inputs must fail (never
//! look like a successful zero-test run), discovery must recurse into nested
//! directories (e.g. the spec suite's `proposals/`), and the default exit
//! status stays baseline-friendly (parse errors are reported, not fatal) while
//! `--strict` gates on them for local use.

#![cfg(feature = "native")]

use std::path::PathBuf;
use std::process::{Command, Output};

const RUNNER_BIN: &str = env!("CARGO_BIN_EXE_wast-runner");

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/wast_fixtures")
}

fn run(args: &[&str]) -> Output {
    Command::new(RUNNER_BIN)
        .args(args)
        .output()
        .expect("failed to run wast-runner")
}

/// A requested path that does not exist must be a hard error, not a silent
/// "0 tests, all pass" — this is the classic unexpanded-`testsuite/*.wast` bug.
#[test]
fn missing_input_path_exits_nonzero() {
    let missing = fixtures_dir().join("this-file-does-not-exist.wast");
    let out = run(&[missing.to_str().unwrap()]);

    assert!(
        !out.status.success(),
        "missing input should exit non-zero, got {:?}",
        out.status
    );
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not found"),
        "missing input should be reported on stderr, got: {stderr}"
    );
}

/// A directory containing no `.wast` files is also not a successful run.
#[test]
fn no_wast_files_found_exits_nonzero() {
    let empty = std::env::temp_dir().join(format!("wast_runner_empty_{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();

    let out = run(&[empty.to_str().unwrap()]);
    let _ = std::fs::remove_dir(&empty);

    assert_eq!(
        out.status.code(),
        Some(2),
        "empty directory should exit 2, got {:?}",
        out.status
    );
}

/// Discovery recurses into nested directories (the fixtures place a module
/// under `nested/deeper/`).
#[test]
fn discovery_recurses_into_subdirectories() {
    let out = run(&[fixtures_dir().to_str().unwrap(), "--format", "json"]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        stdout.contains("nested.wast"),
        "recursive discovery should find nested/deeper/nested.wast, got: {stdout}"
    );
}

/// A single valid file exits 0.
#[test]
fn valid_file_exits_success() {
    let valid = fixtures_dir().join("valid.wast");
    let out = run(&[valid.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "a valid file should exit 0, got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// By default a file the `wast` crate cannot parse is reported but does not fail
/// the run, so the baseline-aware CI comparison stays usable.
#[test]
fn parse_error_is_reported_but_not_fatal_by_default() {
    let out = run(&[fixtures_dir().to_str().unwrap(), "--format", "json"]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        out.status.success(),
        "parse errors should not fail the default run, got {:?}",
        out.status
    );
    assert!(
        stdout.contains("\"files_parse_errors\": 1"),
        "the parse error should be counted in the summary, got: {stdout}"
    );
}

/// `--strict` turns directive failures / parse errors into a non-zero exit for
/// local gating (distinct from the infrastructure exit code 2).
#[test]
fn strict_fails_on_parse_error() {
    let out = run(&[fixtures_dir().to_str().unwrap(), "--strict"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "--strict should exit 1 on a parse error, got {:?}",
        out.status
    );
}
