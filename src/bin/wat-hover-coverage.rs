//! wat-hover-coverage: CI check that every hoverable token in playground examples
//! and documentation has associated hover documentation.
//!
//! Scans `.wat` files and WAT code blocks in `.md` files, extracts word tokens
//! using the same `is_word_char` logic as the hover handler, and verifies each
//! token has an entry in `docs::get_instruction_doc()`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use wat_lsp_rust::docs::get_instruction_doc;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::utils::is_word_char;

// ---------------------------------------------------------------------------
// Comment / string region collection
// ---------------------------------------------------------------------------

/// Byte range that should be skipped (comment or string literal).
struct SkipRange {
    start: usize,
    end: usize,
}

/// Recursively collect byte ranges of comment, string, annotation, and error nodes.
fn collect_skip_ranges(node: tree_sitter::Node, ranges: &mut Vec<SkipRange>) {
    let kind = node.kind();
    if kind == "comment_line"
        || kind == "comment_block"
        || kind == "comment_line_annot"
        || kind == "comment_block_annot"
        || kind == "string"
        || kind == "annotation"
        || kind == "custom_annotation"
        || kind == "ERROR"
    {
        ranges.push(SkipRange {
            start: node.start_byte(),
            end: node.end_byte(),
        });
        return; // don't recurse into children
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_skip_ranges(child, ranges);
    }
}

/// Check whether a byte offset falls inside any skip range.
fn is_in_skip_range(offset: usize, ranges: &[SkipRange]) -> bool {
    ranges.iter().any(|r| r.start <= offset && offset < r.end)
}

// ---------------------------------------------------------------------------
// Numeric literal detection
// ---------------------------------------------------------------------------

/// Check if a token is a numeric literal (integer, float, hex, underscored).
/// WAT numeric literals start with a digit or optional `-`/`+` followed by a digit.
/// Examples: 42, -10, 3.14, -0.0, 0xFF, 1_000, 1.5e2, 1.7976931348623157e+308
fn is_numeric_literal(word: &str) -> bool {
    let s = word.strip_prefix(['-', '+']).unwrap_or(word);
    if s.is_empty() {
        return false;
    }
    let first = s.as_bytes()[0];
    first.is_ascii_digit()
}

// ---------------------------------------------------------------------------
// Token extraction
// ---------------------------------------------------------------------------

/// Location where a token was first seen.
#[derive(Clone)]
struct TokenLocation {
    file: String,
    line: usize, // 1-based
}

/// Extract hoverable tokens from WAT source, checking each against the skip
/// ranges derived from tree-sitter.  Returns a map from token → first location.
fn extract_tokens(
    source: &str,
    filename: &str,
    skip_ranges: &[SkipRange],
    seen: &mut BTreeMap<String, TokenLocation>,
) {
    // Precompute line start byte offsets to handle CRLF correctly.
    // source.lines() strips \r, so manually tracking byte_offset += line.len() + 1
    // would drift on Windows (CRLF = 2 bytes, not 1).
    let mut line_starts: Vec<usize> = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }

    for (line_idx, line) in source.lines().enumerate() {
        let line_start = line_starts[line_idx];
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if is_word_char(chars[i]) {
                let start = i;
                while i < chars.len() && is_word_char(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();

                // Compute byte offset of the word start within the source
                let char_byte = line.char_indices().nth(start).map_or(0, |(b, _)| b);
                let word_byte_offset = line_start + char_byte;

                // Skip if inside a comment or string
                if is_in_skip_range(word_byte_offset, skip_ranges) {
                    continue;
                }

                // Skip $-prefixed identifiers (handled by symbol hover)
                if word.starts_with('$') {
                    continue;
                }

                // Skip numeric literals: integers, floats, hex, underscored
                // A token starting with a digit or `-` followed by a digit is numeric
                if is_numeric_literal(&word) {
                    continue;
                }

                // Skip float special values (inf, nan, nan:0x...)
                if word == "inf" || word.starts_with("nan") {
                    continue;
                }

                // Record first occurrence of this token
                seen.entry(word).or_insert_with(|| TokenLocation {
                    file: filename.to_string(),
                    line: line_idx + 1,
                });
            } else {
                i += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WAT code-block extraction from markdown
// ---------------------------------------------------------------------------

/// Extract WAT code blocks from a markdown file.
/// Returns a list of (code_content, start_line_in_md) pairs.
fn extract_wat_blocks(md_source: &str) -> Vec<(String, usize)> {
    let mut blocks = Vec::new();
    let mut in_wat_block = false;
    let mut current_block = String::new();
    let mut block_start_line = 0;

    for (line_idx, line) in md_source.lines().enumerate() {
        let trimmed = line.trim();
        if !in_wat_block && (trimmed == "```wat" || trimmed.starts_with("```wat ")) {
            in_wat_block = true;
            current_block.clear();
            block_start_line = line_idx + 1; // next line is first code line
        } else if in_wat_block && trimmed == "```" {
            in_wat_block = false;
            if !current_block.is_empty() {
                blocks.push((current_block.clone(), block_start_line));
            }
        } else if in_wat_block {
            // Strip lines starting with # (hidden context lines used in docs)
            if !trimmed.starts_with('#') {
                current_block.push_str(line);
                current_block.push('\n');
            }
        }
    }
    blocks
}

// ---------------------------------------------------------------------------
// File scanning
// ---------------------------------------------------------------------------

fn scan_wat_file(
    path: &Path,
    parser: &mut tree_sitter::Parser,
    seen: &mut BTreeMap<String, TokenLocation>,
) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: could not read {}: {}", path.display(), e);
            return;
        }
    };
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => {
            eprintln!("warning: could not parse {}", path.display());
            return;
        }
    };
    let mut skip_ranges = Vec::new();
    collect_skip_ranges(tree.root_node(), &mut skip_ranges);
    extract_tokens(&source, &path.display().to_string(), &skip_ranges, seen);
}

fn scan_markdown_file(
    path: &Path,
    parser: &mut tree_sitter::Parser,
    seen: &mut BTreeMap<String, TokenLocation>,
) {
    let md_source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: could not read {}: {}", path.display(), e);
            return;
        }
    };
    let blocks = extract_wat_blocks(&md_source);
    let filename = path.display().to_string();

    for (code, _start_line) in &blocks {
        let tree = match parser.parse(code, None) {
            Some(t) => t,
            None => continue,
        };
        let mut skip_ranges = Vec::new();
        collect_skip_ranges(tree.root_node(), &mut skip_ranges);
        extract_tokens(code, &filename, &skip_ranges, seen);
    }
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

fn find_wat_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                files.extend(find_wat_files(&p));
            } else if p.extension().is_some_and(|e| e == "wat") {
                files.push(p);
            }
        }
    }
    files
}

fn find_md_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                files.extend(find_md_files(&p));
            } else if p.extension().is_some_and(|e| e == "md") {
                files.push(p);
            }
        }
    }
    files
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let mut parser = create_parser();
    let mut seen: BTreeMap<String, TokenLocation> = BTreeMap::new();

    // 1. Scan playground examples (valid + invalid)
    let examples_dir = Path::new("packages/playground/examples");
    let wat_files = find_wat_files(examples_dir);
    if wat_files.is_empty() {
        eprintln!("warning: no .wat files found in {}", examples_dir.display());
    }
    for path in &wat_files {
        scan_wat_file(path, &mut parser, &mut seen);
    }

    // 2. Scan documentation markdown files
    let docs_dir = Path::new("packages/docs/src/content/docs");
    let md_files = find_md_files(docs_dir);
    if md_files.is_empty() {
        eprintln!("warning: no .md files found in {}", docs_dir.display());
    }
    for path in &md_files {
        scan_markdown_file(path, &mut parser, &mut seen);
    }

    // 3. Check each unique token against instruction docs
    let mut missing: BTreeSet<String> = BTreeSet::new();
    for token in seen.keys() {
        if get_instruction_doc(token).is_none() {
            missing.insert(token.clone());
        }
    }

    if missing.is_empty() {
        let token_count = seen.len();
        eprintln!(
            "Hover coverage OK: all {} unique tokens have documentation",
            token_count
        );
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "Hover coverage FAILED: {} token(s) missing documentation:\n",
            missing.len()
        );
        for token in &missing {
            let loc = &seen[token];
            eprintln!("  {:<40} first seen at {}:{}", token, loc.file, loc.line);
        }
        eprintln!();
        ExitCode::from(1)
    }
}
