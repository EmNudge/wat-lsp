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
const COMPARE_BIN: &str = env!("CARGO_BIN_EXE_wast-compare");

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures_dir() -> PathBuf {
    manifest_dir().join("tests/wast_fixtures")
}

fn baseline_path() -> PathBuf {
    manifest_dir().join("tests/baseline/wast_fixtures_baseline.json")
}

fn run(args: &[&str]) -> Output {
    Command::new(RUNNER_BIN)
        .args(args)
        .output()
        .expect("failed to run wast-runner")
}

fn run_compare(base: &str, pr: &str) -> Output {
    Command::new(COMPARE_BIN)
        .args(["--base", base, "--pr", pr])
        .output()
        .expect("failed to run wast-compare")
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

/// Negative-case scoring: `assert_invalid` must only pass on a *validation*
/// failure, and `assert_malformed` on a parse/lex failure. The paired fixture
/// contains one genuine validation failure (PASS), one valid module wrongly
/// wrapped in assert_invalid (FAIL), and one malformed case (PASS).
#[test]
fn negative_case_scoring_distinguishes_validation_from_none() {
    let fixture = fixtures_dir().join("negative_cases.wast");
    let out = run(&[fixture.to_str().unwrap(), "--format", "json"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("runner should emit valid JSON");

    let file = &json["files"][0];
    // assert_invalid: 1 of 2 pass (the type-mismatch one; the valid module fails).
    assert_eq!(file["assert_invalid"]["pass"], 1);
    assert_eq!(file["assert_invalid"]["total"], 2);
    // assert_malformed: 1 of 1 pass.
    assert_eq!(file["assert_malformed"]["pass"], 1);
    assert_eq!(file["assert_malformed"]["total"], 1);
    // Overall: 2 pass, 1 fail.
    assert_eq!(file["pass"], 2);
    assert_eq!(file["fail"], 1);

    // The false-negative case must be labelled as such, not silently passed.
    let directives = file["directives"].as_array().unwrap();
    let has_no_error_fail = directives.iter().any(|d| {
        d["kind"] == "assert_invalid"
            && d["outcome"] == "fail"
            && d["label"].as_str().unwrap_or("").contains("no errors")
    });
    assert!(
        has_no_error_fail,
        "the valid-module assert_invalid should fail with a 'no errors' label, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Baseline regression gate (wast-compare)
// ---------------------------------------------------------------------------

/// Run the runner over the committed fixtures and normalize the JSON the same
/// way `scripts/normalize-baseline.py` does, so it can be diffed against the
/// committed baseline.
fn normalized_fixture_results() -> serde_json::Value {
    let out = run(&[fixtures_dir().to_str().unwrap(), "--format", "json"]);
    let mut json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).unwrap();

    if let Some(files) = json.get_mut("files").and_then(|f| f.as_array_mut()) {
        for f in files.iter_mut() {
            // basename
            let base = f
                .get("file")
                .and_then(|v| v.as_str())
                .map(|s| {
                    std::path::Path::new(s)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(s)
                        .to_string()
                })
                .unwrap_or_default();
            f["file"] = serde_json::Value::String(base);
            if let Some(dirs) = f.get_mut("directives").and_then(|d| d.as_array_mut()) {
                for d in dirs.iter_mut() {
                    if let Some(obj) = d.as_object_mut() {
                        obj.remove("diagnostics");
                    }
                }
            }
        }
        files.sort_by(|a, b| {
            a["file"]
                .as_str()
                .unwrap_or("")
                .cmp(b["file"].as_str().unwrap_or(""))
        });
    }
    json.as_object_mut().unwrap().remove("summary");
    json
}

/// The committed baseline must match a fresh run of the fixtures. If this fails,
/// regenerate it (see the workflow comment) — this keeps the gate meaningful.
#[test]
fn committed_baseline_is_up_to_date() {
    let fresh = normalized_fixture_results();
    let committed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(baseline_path()).unwrap())
            .expect("committed baseline should be valid JSON");
    assert_eq!(
        fresh, committed,
        "committed baseline is stale; regenerate tests/baseline/wast_fixtures_baseline.json"
    );
}

/// The gate passes when the current run matches the committed baseline (no
/// regression). This is the "green PR" path.
#[test]
fn gate_passes_against_committed_baseline() {
    let out = run(&[fixtures_dir().to_str().unwrap(), "--format", "json"]);
    let pr = std::env::temp_dir().join(format!("wast_gate_pr_{}.json", std::process::id()));
    std::fs::write(&pr, &out.stdout).unwrap();

    let cmp = run_compare(baseline_path().to_str().unwrap(), pr.to_str().unwrap());
    let _ = std::fs::remove_file(&pr);
    assert!(
        cmp.status.success(),
        "gate should pass against the committed baseline, got {:?}\nstderr: {}",
        cmp.status,
        String::from_utf8_lossy(&cmp.stderr)
    );
}

/// An injected pass->fail regression must fail the gate (exit 1) — this is the
/// acceptance criterion for false positives / false negatives being caught.
#[test]
fn gate_fails_on_injected_regression() {
    // Take the committed baseline and flip one passing directive to "fail",
    // simulating a PR that broke a previously-covered case.
    let baseline = std::fs::read_to_string(baseline_path()).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&baseline).unwrap();
    let mut flipped = false;
    'outer: for f in json["files"].as_array_mut().unwrap() {
        for d in f["directives"].as_array_mut().unwrap() {
            if d["outcome"] == "pass" {
                d["outcome"] = serde_json::Value::String("fail".to_string());
                flipped = true;
                break 'outer;
            }
        }
    }
    assert!(
        flipped,
        "baseline should contain a passing directive to flip"
    );

    let pr = std::env::temp_dir().join(format!("wast_gate_reg_{}.json", std::process::id()));
    std::fs::write(&pr, serde_json::to_string(&json).unwrap()).unwrap();
    let cmp = run_compare(baseline_path().to_str().unwrap(), pr.to_str().unwrap());
    let _ = std::fs::remove_file(&pr);

    assert_eq!(
        cmp.status.code(),
        Some(1),
        "gate should fail (exit 1) on a pass->fail regression, got {:?}",
        cmp.status
    );
    let stderr = String::from_utf8_lossy(&cmp.stderr);
    assert!(
        stderr.contains("regression"),
        "gate should report the regression, got: {stderr}"
    );
}

/// A disappeared covered case must also fail the gate (a case cannot silently
/// vanish to hide a regression).
#[test]
fn gate_fails_on_disappeared_case() {
    let baseline = std::fs::read_to_string(baseline_path()).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&baseline).unwrap();
    // Drop all directives from the first file.
    json["files"][0]["directives"] = serde_json::Value::Array(vec![]);

    let pr = std::env::temp_dir().join(format!("wast_gate_gone_{}.json", std::process::id()));
    std::fs::write(&pr, serde_json::to_string(&json).unwrap()).unwrap();
    let cmp = run_compare(baseline_path().to_str().unwrap(), pr.to_str().unwrap());
    let _ = std::fs::remove_file(&pr);

    assert_eq!(
        cmp.status.code(),
        Some(1),
        "gate should fail when a covered case disappears, got {:?}",
        cmp.status
    );
}
