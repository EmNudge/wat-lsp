//! wast-compare: baseline-aware regression gate for the conformance runner.
//!
//! Reads two `wast-runner --format json` result files (a *base* baseline and a
//! *PR* run) and FAILS (exit 1) when the PR regresses relative to the base:
//!
//! * a directive that passed in the base now fails or is skipped (a pass→fail
//!   or pass→skip regression), or
//! * a directive that was covered in the base has disappeared entirely from the
//!   PR results (a covered case vanished).
//!
//! Improvements (fail→pass, new passing directives, new files) never fail the
//! gate. This is deliberately separate from the informational PR comment so CI
//! can hard-fail on regressions while known failures do not make every run
//! unusable.
//!
//! Identity is `(file basename, line, kind)`. Using the basename (not the full
//! path) keeps the comparison stable across the different checkout directories
//! CI uses for the base and PR builds. We compare *structural* outcomes, never
//! upstream error wording.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use serde::Deserialize;

/// Compare two wast-runner JSON result files and fail on regressions.
#[derive(Parser, Debug)]
#[command(name = "wast-compare")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Baseline results JSON (e.g. from the base branch).
    #[arg(long)]
    base: PathBuf,

    /// PR / candidate results JSON.
    #[arg(long)]
    pr: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Results {
    #[serde(default)]
    files: Vec<FileSummary>,
}

#[derive(Debug, Deserialize)]
struct FileSummary {
    file: String,
    #[serde(default)]
    directives: Vec<Directive>,
}

#[derive(Debug, Deserialize, Clone)]
struct Directive {
    line: usize,
    kind: String,
    outcome: String,
    #[serde(default)]
    label: String,
}

/// A directive's identity, stable across checkout paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    file: String,
    line: usize,
    kind: String,
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Index every directive by identity. If the same identity somehow appears
/// twice (it should not), the first wins; that is fine for the comparison.
fn index(results: &Results) -> HashMap<Key, Directive> {
    let mut map = HashMap::new();
    for file in &results.files {
        let base = basename(&file.file);
        for d in &file.directives {
            let key = Key {
                file: base.clone(),
                line: d.line,
                kind: d.kind.clone(),
            };
            map.entry(key).or_insert_with(|| d.clone());
        }
    }
    map
}

fn read(path: &Path) -> Results {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "failed to parse {} as wast-runner JSON: {e}",
            path.display()
        )
    })
}

/// A regression found by the comparison.
struct Regression {
    key: Key,
    kind: RegressionKind,
    detail: String,
}

enum RegressionKind {
    /// Passed in base, now fails or is skipped.
    PassToFail,
    PassToSkip,
    /// Covered in base, entirely absent from the PR results.
    Disappeared,
}

impl RegressionKind {
    fn label(&self) -> &'static str {
        match self {
            RegressionKind::PassToFail => "pass→fail",
            RegressionKind::PassToSkip => "pass→skip",
            RegressionKind::Disappeared => "disappeared",
        }
    }
}

fn compare(base: &Results, pr: &Results) -> Vec<Regression> {
    let base_index = index(base);
    let pr_index = index(pr);

    let mut regressions = Vec::new();

    for (key, base_dir) in &base_index {
        // We only gate against cases that were *passing* in the baseline (plus
        // disappearance of any covered case). A base failure that stays a
        // failure is a known gap, not a regression.
        match pr_index.get(key) {
            None => {
                // A covered case that vanished. This hides regressions behind
                // "the case is just gone", so it must fail regardless of the
                // base outcome.
                regressions.push(Regression {
                    key: key.clone(),
                    kind: RegressionKind::Disappeared,
                    detail: format!("base outcome was {}", base_dir.outcome),
                });
            }
            Some(pr_dir) => {
                if base_dir.outcome == "pass" && pr_dir.outcome != "pass" {
                    let kind = if pr_dir.outcome == "skip" {
                        RegressionKind::PassToSkip
                    } else {
                        RegressionKind::PassToFail
                    };
                    regressions.push(Regression {
                        key: key.clone(),
                        kind,
                        detail: pr_dir.label.clone(),
                    });
                }
            }
        }
    }

    // Deterministic ordering for stable CI output.
    regressions.sort_by(|a, b| {
        a.key
            .file
            .cmp(&b.key.file)
            .then(a.key.line.cmp(&b.key.line))
            .then(a.key.kind.cmp(&b.key.kind))
    });
    regressions
}

fn main() -> ExitCode {
    let args = Args::parse();
    let base = read(&args.base);
    let pr = read(&args.pr);

    let regressions = compare(&base, &pr);

    if regressions.is_empty() {
        println!("wast-compare: no regressions detected");
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "wast-compare: {} regression(s) detected:",
        regressions.len()
    );
    for r in &regressions {
        eprintln!(
            "  [{}] {}:{} ({}) — {}",
            r.kind.label(),
            r.key.file,
            r.key.line,
            r.key.kind,
            r.detail
        );
    }
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results(json: &str) -> Results {
        serde_json::from_str(json).unwrap()
    }

    const BASE: &str = r#"{
        "files": [{
            "file": "/base/testsuite/a.wast",
            "directives": [
                {"line": 1, "kind": "module", "outcome": "pass", "label": ""},
                {"line": 5, "kind": "assert_invalid", "outcome": "pass", "label": ""},
                {"line": 9, "kind": "assert_malformed", "outcome": "fail", "label": ""}
            ]
        }]
    }"#;

    #[test]
    fn no_change_is_clean() {
        let base = results(BASE);
        let pr = results(BASE);
        assert!(compare(&base, &pr).is_empty());
    }

    #[test]
    fn pass_to_fail_is_a_regression() {
        let base = results(BASE);
        let pr = results(
            r#"{"files":[{"file":"/pr/x/a.wast","directives":[
                {"line":1,"kind":"module","outcome":"pass","label":""},
                {"line":5,"kind":"assert_invalid","outcome":"fail","label":"regressed"},
                {"line":9,"kind":"assert_malformed","outcome":"fail","label":""}
            ]}]}"#,
        );
        let regs = compare(&base, &pr);
        assert_eq!(regs.len(), 1);
        assert!(matches!(regs[0].kind, RegressionKind::PassToFail));
        assert_eq!(regs[0].key.line, 5);
    }

    #[test]
    fn pass_to_skip_is_a_regression() {
        let base = results(BASE);
        let pr = results(
            r#"{"files":[{"file":"/pr/x/a.wast","directives":[
                {"line":1,"kind":"module","outcome":"skip","label":""},
                {"line":5,"kind":"assert_invalid","outcome":"pass","label":""},
                {"line":9,"kind":"assert_malformed","outcome":"fail","label":""}
            ]}]}"#,
        );
        let regs = compare(&base, &pr);
        assert_eq!(regs.len(), 1);
        assert!(matches!(regs[0].kind, RegressionKind::PassToSkip));
        assert_eq!(regs[0].key.line, 1);
    }

    #[test]
    fn disappeared_covered_case_is_a_regression() {
        let base = results(BASE);
        // The module at line 1 is dropped entirely.
        let pr = results(
            r#"{"files":[{"file":"/pr/x/a.wast","directives":[
                {"line":5,"kind":"assert_invalid","outcome":"pass","label":""},
                {"line":9,"kind":"assert_malformed","outcome":"fail","label":""}
            ]}]}"#,
        );
        let regs = compare(&base, &pr);
        assert_eq!(regs.len(), 1);
        assert!(matches!(regs[0].kind, RegressionKind::Disappeared));
        assert_eq!(regs[0].key.line, 1);
    }

    #[test]
    fn whole_file_disappearance_is_a_regression() {
        let base = results(BASE);
        let pr = results(r#"{"files":[]}"#);
        let regs = compare(&base, &pr);
        // All three covered directives vanished.
        assert_eq!(regs.len(), 3);
        assert!(regs
            .iter()
            .all(|r| matches!(r.kind, RegressionKind::Disappeared)));
    }

    #[test]
    fn improvements_and_new_cases_do_not_fail() {
        let base = results(BASE);
        // The malformed case improves fail→pass; a brand-new passing case appears.
        let pr = results(
            r#"{"files":[{"file":"/pr/x/a.wast","directives":[
                {"line":1,"kind":"module","outcome":"pass","label":""},
                {"line":5,"kind":"assert_invalid","outcome":"pass","label":""},
                {"line":9,"kind":"assert_malformed","outcome":"pass","label":"fixed"},
                {"line":20,"kind":"module","outcome":"pass","label":"new"}
            ]}]}"#,
        );
        assert!(compare(&base, &pr).is_empty());
    }

    #[test]
    fn identity_is_stable_across_checkout_paths() {
        // Base and PR use different absolute directories; basename keying must
        // still match them up.
        let base = results(
            r#"{"files":[{"file":"/home/runner/base/testsuite/m.wast","directives":[
                {"line":3,"kind":"module","outcome":"pass","label":""}
            ]}]}"#,
        );
        let pr = results(
            r#"{"files":[{"file":"/home/runner/pr/testsuite/m.wast","directives":[
                {"line":3,"kind":"module","outcome":"fail","label":"broke"}
            ]}]}"#,
        );
        let regs = compare(&base, &pr);
        assert_eq!(regs.len(), 1);
        assert!(matches!(regs[0].kind, RegressionKind::PassToFail));
    }
}
