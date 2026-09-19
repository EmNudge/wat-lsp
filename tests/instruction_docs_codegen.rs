//! Unit tests for the build-time instruction/annotation docs codegen parser.
//!
//! The parser lives in `build_support/instruction_docs.rs` and is `include!`d
//! both here and from `build.rs`, so codegen and this test share one parsed
//! instruction set. These tests pin down the validation behavior (duplicate
//! detection, empty-body rejection, escaping) that keeps the build from silently
//! producing wrong or empty output.

#![allow(dead_code)]

include!("../build_support/instruction_docs.rs");

fn names(entries: &[DocEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn parses_multiple_entries_sorted() {
    let md = "\
# Title

## i32.sub

Subtract.

---

## i32.add

Add two values.

---
";
    let entries = parse_docs(md).expect("valid docs should parse");
    assert_eq!(names(&entries), vec!["i32.add", "i32.sub"]);
    assert_eq!(entries[0].body, "Add two values.");
}

#[test]
fn detects_duplicate_entries() {
    let md = "\
## i32.add

First definition.

---

## i32.add

Second definition.

---
";
    let err = parse_docs(md).expect_err("duplicate names must be rejected");
    assert!(err.contains("duplicate"), "message was: {err}");
    assert!(err.contains("i32.add"), "message was: {err}");
    // Should point at the first definition line for actionable output.
    assert!(err.contains("first defined at line"), "message was: {err}");
}

#[test]
fn rejects_entry_with_empty_body() {
    let md = "\
## i32.add

---
";
    let err = parse_docs(md).expect_err("empty body must be rejected");
    assert!(err.contains("no documentation body"), "message was: {err}");
    assert!(err.contains("i32.add"), "message was: {err}");
}

#[test]
fn last_entry_without_trailing_separator_is_captured() {
    let md = "\
## only.one

A description with no trailing separator.
";
    let entries = parse_docs(md).expect("valid docs should parse");
    assert_eq!(names(&entries), vec!["only.one"]);
    assert_eq!(entries[0].body, "A description with no trailing separator.");
}

#[test]
fn template_inside_code_fence_is_not_an_entry() {
    // Mirrors the real instructions.md header: a `## name` line inside a fenced
    // code block documenting the format must not be parsed as a real entry.
    let md = "\
# Docs

Format:

```
## instruction.name
Description.
---
```

## real.instr

The real one.

---
";
    let entries = parse_docs(md).expect("valid docs should parse");
    assert_eq!(names(&entries), vec!["real.instr"]);
}

#[test]
fn preserves_code_fences_and_strips_hidden_lines() {
    let md = "\
## example.instr

Does a thing.

Example:
```wat
# hidden setup line
(example.instr)
```

---
";
    let entries = parse_docs(md).expect("valid docs should parse");
    let body = &entries[0].body;
    assert!(body.contains("```wat"), "fence should be preserved: {body}");
    assert!(body.contains("(example.instr)"), "body was: {body}");
    assert!(
        !body.contains("hidden setup line"),
        "hidden `# ` lines should be stripped: {body}"
    );
}

#[test]
fn escapes_special_characters_for_rust_literal() {
    // Backslash, quote, tab and carriage-return must all be escaped so the
    // generated source compiles and round-trips.
    let raw = "a\"b\\c\td\r";
    let escaped = escape_rust_string(raw);
    assert_eq!(escaped, "a\\\"b\\\\c\\td\\r");
}

#[test]
fn generated_code_is_well_formed_and_deterministic() {
    let md = "\
## b.instr

Beta \"quoted\" and back\\slash.

---

## a.instr

Alpha.

---
";
    let entries = parse_docs(md).unwrap();
    let code = generate_rust_code(&entries, "TEST_DOCS");
    // Sorted order in the emitted table.
    let a_idx = code.find("\"a.instr\"").unwrap();
    let b_idx = code.find("\"b.instr\"").unwrap();
    assert!(a_idx < b_idx, "entries should be emitted sorted");
    assert!(code.contains("static TEST_DOCS: [(&str, &str); 2]"));
    // Special characters escaped in the literal.
    assert!(code.contains("Beta \\\"quoted\\\""));
    assert!(code.contains("back\\\\slash"));
    // Deterministic across runs.
    assert_eq!(code, generate_rust_code(&entries, "TEST_DOCS"));
}

#[test]
fn real_instructions_md_parses_without_duplicates() {
    // Guard the actual shipped docs: the build would otherwise silently keep
    // only the last of any duplicate, so pin this here too.
    let content = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/packages/docs/instructions.md"
    ))
    .expect("instructions.md should exist");
    let entries = parse_docs(&content).expect("instructions.md must be well-formed");
    assert!(entries.len() > 100, "expected many instructions");
    // Sorted + unique invariant the generated binary-search table relies on.
    for pair in entries.windows(2) {
        assert!(
            pair[0].name < pair[1].name,
            "entries must be strictly sorted/unique: {} !< {}",
            pair[0].name,
            pair[1].name
        );
    }
}

#[test]
fn real_annotations_md_parses_without_duplicates() {
    let content = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/packages/docs/annotations.md"
    ))
    .expect("annotations.md should exist");
    let entries = parse_docs(&content).expect("annotations.md must be well-formed");
    assert!(!entries.is_empty());
    for pair in entries.windows(2) {
        assert!(pair[0].name < pair[1].name);
    }
}
