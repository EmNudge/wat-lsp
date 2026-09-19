// Shared build-time parser for the instruction/annotation markdown docs.
//
// This file is the single source of truth for turning
// `packages/docs/instructions.md` and `packages/docs/annotations.md` into the
// generated Rust lookup tables. It is `include!`d from `build.rs` (for codegen)
// and from `tests/instruction_docs_codegen.rs` (for validation), so the parser
// and its validation are exercised by the normal `cargo test` run rather than
// only implicitly at build time.
//
// The parser validates its input and returns a descriptive error instead of
// silently emitting wrong or empty output when it sees:
//   * duplicate `## name` entries (the old `HashMap` silently kept only the
//     last one),
//   * an entry with no documentation body, or
//   * (at the call site) a missing input file.
//
// NOTE: keep this file free of `use` statements and crate-internal types so it
// can be `include!`d into both `build.rs` and an integration test without
// pulling in the rest of the crate.

/// A single parsed doc entry: the instruction/annotation name and its rendered
/// markdown body.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DocEntry {
    name: String,
    body: String,
}

/// Parse the docs markdown into a sorted, de-duplicated list of entries.
///
/// Returns `Err` with a human-readable message if the same name appears twice or
/// if an entry has an empty body. The result is sorted by name so codegen output
/// is deterministic and the generated table is directly binary-searchable.
fn parse_docs(content: &str) -> Result<Vec<DocEntry>, String> {
    // Normalize line endings to handle both Unix (\n) and Windows (\r\n).
    let normalized = content.replace("\r\n", "\n");

    let mut entries: Vec<DocEntry> = Vec::new();
    // Track the first line each name was seen on for a useful duplicate message.
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    let mut instruction_name: Option<String> = None;
    let mut name_line: usize = 0;
    let mut doc_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;

    // Save the current instruction (if any) into `entries`, validating it.
    fn save_instruction(
        name: &mut Option<String>,
        name_line: usize,
        lines: &mut Vec<String>,
        entries: &mut Vec<DocEntry>,
    ) -> Result<(), String> {
        if let Some(n) = name.take() {
            // Trim trailing empty lines.
            while lines.last().map(String::as_str) == Some("") {
                lines.pop();
            }
            if lines.is_empty() {
                return Err(format!(
                    "line {name_line}: entry \"{n}\" has no documentation body"
                ));
            }
            let body = lines.join("\n");
            entries.push(DocEntry { name: n, body });
            lines.clear();
        }
        Ok(())
    }

    for (idx, line) in normalized.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();

        // Code block boundaries.
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if instruction_name.is_some() {
                doc_lines.push(trimmed.to_string());
            }
            continue;
        }

        // Inside code blocks, preserve original indentation.
        if in_code_block {
            if instruction_name.is_some() {
                // Strip hidden context lines (# prefix) from hover display.
                if line.starts_with("# ") || line == "#" {
                    continue;
                }
                doc_lines.push(line.to_string());
            }
            continue;
        }

        // Section separator - save the current instruction and reset.
        if trimmed == "---" {
            save_instruction(
                &mut instruction_name,
                name_line,
                &mut doc_lines,
                &mut entries,
            )?;
            continue;
        }

        // New instruction header.
        if let Some(stripped) = trimmed.strip_prefix("## ") {
            save_instruction(
                &mut instruction_name,
                name_line,
                &mut doc_lines,
                &mut entries,
            )?;
            let name = stripped.trim().to_string();
            if let Some(&first) = seen.get(&name) {
                return Err(format!(
                    "line {line_no}: duplicate entry \"{name}\" (first defined at line {first})"
                ));
            }
            seen.insert(name.clone(), line_no);
            instruction_name = Some(name);
            name_line = line_no;
            continue;
        }

        // Skip the document title.
        if trimmed.starts_with("# ") {
            continue;
        }

        // Content line for the current instruction.
        if instruction_name.is_some() && (!trimmed.is_empty() || !doc_lines.is_empty()) {
            doc_lines.push(trimmed.to_string());
        }
    }

    // Don't forget the last instruction.
    save_instruction(
        &mut instruction_name,
        name_line,
        &mut doc_lines,
        &mut entries,
    )?;

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Escape a string for embedding inside a Rust `"..."` string literal.
fn escape_rust_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Render the parsed entries as a Rust source file defining `var_name` as a
/// sorted `[(&str, &str); N]` table.
fn generate_rust_code(entries: &[DocEntry], var_name: &str) -> String {
    let count = entries.len();

    let mut code = format!(
        "// This file is automatically generated by build.rs\n\
         // Do not edit manually\n\n\
         /// Sorted array of (name, doc) pairs — use binary search for lookup.\n\
         pub(super) static {var_name}: [(&str, &str); {count}] = [\n"
    );

    for entry in entries {
        let name = escape_rust_string(&entry.name);
        let body = escape_rust_string(&entry.body);
        code.push_str(&format!("    (\"{name}\", \"{body}\"),\n"));
    }

    code.push_str("];\n");
    code
}
