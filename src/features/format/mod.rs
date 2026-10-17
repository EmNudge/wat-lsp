//! WAT formatting.
//!
//! This module owns two things:
//!
//! * [`format_whitespace`] — a conservative, token-preserving formatter that only
//!   changes indentation, trailing whitespace, and runs of blank lines. Token
//!   order and inter-token spacing are left exactly as written, so the pass can
//!   never change a document's meaning and is idempotent by construction.
//! * [`provide_formatting`] — the feature entry point that applies the whitespace
//!   pass and reports the resulting edit as a protocol-independent [`TextEdit`].
//!
//! # Why not reformat tokens
//!
//! WAT has four equivalent spellings for the same construct (folded, linear,
//! plain, and abbreviated forms) and the grammar treats comments and annotations
//! as tree-sitter `extras` — unowned nodes the AST cannot reattach to a parent.
//! Reflowing tokens therefore requires reconstructing comment ownership from
//! scratch. That is deliberately out of scope here.
//!
//! # Two spellings of a block
//!
//! A block can be written with brackets (`(block ...)`, `(loop ...)`) or
//! bracket-free (`block ... end`, `loop ... end`). Both are legal and compile to
//! identical binaries, and `wasm-tools print` emits the bracket-free spelling by
//! default for every instruction sequence, so real input frequently contains it.
//!
//! Bracket depth alone cannot see the bracket-free spelling, so
//! [`scan_linear_blocks`] recognises its leading keywords. The keyword list is
//! deliberately small — one entry per `block_*` rule in the grammar — and a test
//! cross-checks it against `grammar.js` so a future block instruction cannot be
//! added silently.
//!
//! Deriving the levels from the syntax tree instead was measured against all 65
//! bundled examples and produced structural mistakes in 22 of them: comments are
//! tree-sitter `extras` whose ownership is unstable, and `annotation` bodies are
//! leaves with no child nodes to inspect.
//!
//! # Deliberately unchanged
//!
//! * Inter-token spacing (`(a  b   c)` keeps its double and triple spaces).
//! * Line breaks between tokens.
//! * Comment text, including the indentation of continuation lines inside
//!   multi-line block comments.
//!
//! A follow-up can normalise inter-token spacing and wrap long folded
//! expressions once it has a way to reattach comments to the node they belong
//! to.

use crate::core::text::TextIndex;
use crate::core::types::{Position, Range};

/// Formatting options, mirroring the LSP `FormattingOptions` fields this feature
/// consumes. Kept protocol-independent so the WASM build shares one code path.
///
/// The three `Option<bool>` fields follow the protocol's guidance that a client
/// omits what it does not care about. `None` therefore means "use the formatter's
/// default", which is `true` for all three: stripping trailing whitespace and
/// ending the file with exactly one newline are the behaviours every formatter
/// in this space has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    /// Spaces per indentation level. Ignored when `insert_spaces` is false.
    pub tab_size: usize,
    /// Use spaces (`true`) or tabs (`false`) for indentation.
    pub insert_spaces: bool,
    /// Strip trailing whitespace from every line.
    pub trim_trailing_whitespace: bool,
    /// Ensure the file ends with a newline.
    pub insert_final_newline: bool,
    /// Collapse newlines after the final one, so the file ends with exactly one.
    /// Only meaningful when `insert_final_newline` is true.
    pub trim_final_newlines: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            tab_size: 2,
            insert_spaces: true,
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            trim_final_newlines: true,
        }
    }
}

impl FormatOptions {
    /// Clamp externally supplied options into the range the formatter supports.
    ///
    /// A client may send `tab_size: 0` (which would make indentation
    /// meaningless) or an absurdly large value that would allocate megabytes per
    /// level. Both are clamped rather than rejected so a malformed request
    /// cannot fail the feature or exhaust memory.
    pub fn sanitized(self) -> Self {
        Self {
            tab_size: self.tab_size.clamp(1, 16),
            ..self
        }
    }
}

/// The outcome of a formatting request. Protocol-independent; each binding
/// converts it to its own representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattingResult {
    /// Edits to apply. Empty when the document is already formatted, which lets
    /// callers avoid a no-op edit that would dirty the buffer.
    pub edits: Vec<TextEdit>,
}

/// A protocol-independent replacement of a document range.
///
/// `range` is expressed in the same coordinate space as [`crate::core::types`],
/// i.e. line plus UTF-16 code-unit column, matching LSP positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[cfg(all(test, feature = "native"))]
mod tests;

/// Maximum number of consecutive blank lines preserved in the output.
const MAX_BLANK_LINES: usize = 1;

/// Per-line scan result. Bracket depth and comment boundaries are the primary
/// signal; linear block keywords are the exception described on
/// [`scan_linear_blocks`].
#[derive(Debug, Default, Clone, Copy)]
struct LineScan {
    /// Byte offset where a `;;` line comment starts, if one exists outside
    /// strings and block comments.
    line_comment_at: Option<usize>,
    /// Opening brackets contributed by this line, excluding those inside block
    /// comments or strings.
    opens: usize,
    /// Closing brackets contributed by this line, same exclusions.
    closes: usize,
    /// Run of closing brackets at the very start of the line (ignoring
    /// whitespace). Used to dedent a line that begins by closing a form.
    leading_closes: usize,
    /// Levels opened by bracket-free block forms starting this line.
    block_opens: usize,
    /// Levels closed by bracket-free block forms starting this line.
    block_closes: usize,
    /// Levels closed and reopened by a sibling separator (`else`, `catch`).
    block_switches: usize,
    /// Whether the line begins inside a block comment carried over from a
    /// previous line.
    starts_in_block_comment: bool,
    /// Block comment nesting depth at end of line, carried to the next line.
    block_depth_end: usize,
    /// Block keywords that head a folded form; the bracket already indents the
    /// line, so this count is subtracted from the line's own indent.
    folded_heads: usize,
    /// Whether the last code byte on the line was an opening bracket. A keyword at
    /// the head of the next line is then a folded head rather than a bracket-free
    /// block.
    ends_with_open_bracket: bool,
}

/// Keywords that open a level in the bracket-free spelling of a block.
///
/// The spelled-with-brackets forms (`(block ...)`, `(loop ...)`) open a bracket
/// and are therefore already counted by [`scan_line`]; listing them here would
/// double-count. Only the forms that use a trailing `end` appear.
///
/// Keep this in sync with the `block_*` rules in
/// `grammars/tree-sitter-wat/grammar.js`; `block_keywords_cover_grammar` in the
/// test module fails when a rule is added without a matching entry.
const LINEAR_BLOCK_OPENERS: [&str; 5] = ["block", "loop", "if", "try", "try_table"];

/// Keywords that close a level opened by [`LINEAR_BLOCK_OPENERS`].
const LINEAR_BLOCK_CLOSERS: [&str; 1] = ["end"];

/// Keywords that close one level and immediately open its sibling, so the line
/// they start dedents by one and the body that follows re-indents.
const LINEAR_BLOCK_SWITCHERS: [&str; 4] = ["else", "catch", "catch_all", "delegate"];

/// What the leading run of block keywords on a line contributes.
#[derive(Debug, Default, Clone, Copy)]
struct LinearBlocks {
    /// Levels opened by bracket-free block keywords.
    opens: usize,
    /// Levels closed by `end`.
    closes: usize,
    /// Levels closed and reopened by a sibling separator (`else`, `catch`).
    switches: usize,
    /// Block keywords that head a **folded** form, i.e. preceded by an opening
    /// bracket. They open a level for their body, but the bracket already supplies
    /// the line's own indentation, so the line itself must not gain one.
    folded_heads: usize,
}

/// Count block keywords at the start of a line's code.
///
/// `bracket_open_above` reports whether an opening bracket was still unclosed when
/// the line began, which makes the first keyword on the line a folded head.
///
/// `wasm-tools print` spells every instruction sequence in the bracket-free form,
/// so recognising it is the common case rather than an edge case. Detection looks
/// only at the leading run of words, which keeps operands and identifiers out of
/// the analysis: `local.get $block` and the text of a `;;` comment can never be
/// mistaken for a form opener. A word carrying a trailing `)` (`end)`, `else)`)
/// still matches, because a bracket-free block is frequently closed flush against
/// the form that contains it.
///
/// # Folded heads
///
/// These three shapes all mean "one block, body indented one level in":
///
/// ```text
/// (block        <- keyword follows `(` on the same line
///   nop)
///
/// (             <- keyword follows a `(` left open by the previous line
/// block
///   nop)
///
/// block         <- no bracket anywhere near, the bracket-free spelling
///   nop
/// end
/// ```
///
/// The first two carry a bracket, so bracket depth already indents the line. They
/// still open a level, which is why [`LinearBlocks::opens`] is incremented and
/// [`LinearBlocks::folded_heads`] records that the line must not be indented
/// again.
fn scan_linear_blocks(code: &str, bracket_open_above: bool) -> LinearBlocks {
    let mut result = LinearBlocks::default();
    // Bracket depth accumulated from words already consumed on this line.
    let mut depth_from_line = 0usize;

    for raw in code.split_ascii_whitespace() {
        let word = raw.trim_end_matches(')');
        let is_keyword = LINEAR_BLOCK_OPENERS.contains(&word)
            || LINEAR_BLOCK_CLOSERS.contains(&word)
            || LINEAR_BLOCK_SWITCHERS.contains(&word);
        if !is_keyword {
            // The leading run of block keywords has ended.
            break;
        }

        if LINEAR_BLOCK_OPENERS.contains(&word) {
            result.opens += 1;
            if bracket_open_above || depth_from_line > 0 {
                result.folded_heads += 1;
            }
        } else if LINEAR_BLOCK_CLOSERS.contains(&word) {
            result.closes += 1;
        } else {
            result.switches += 1;
        }

        // A word that lost its closing bracket terminated the form it belonged to,
        // so nothing after it on this line can be a leading block.
        if word.len() != raw.len() {
            break;
        }

        // Fold in the brackets this word carried so the next iteration knows
        // whether the following keyword is still inside one.
        depth_from_line = depth_from_line
            .saturating_add(raw.matches('(').count())
            .saturating_sub(raw.matches(')').count());
    }

    result
}

/// Scan one line, tracking strings, `;;` line comments, and `(; ... ;)` block
/// comments.
///
/// WAT block comments nest arbitrarily deep, so depth is counted rather than
/// tracked as a boolean. Brackets, semicolons, and quotes inside a block comment
/// or a string literal never affect the counts.
fn scan_line(
    line: &str,
    block_depth_start: usize,
    previous_ended_with_open_bracket: bool,
) -> LineScan {
    let bytes = line.as_bytes();
    let mut block_depth = block_depth_start;
    let starts_in_block_comment = block_depth > 0;
    let mut in_string = false;
    let mut line_comment_at = None;
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut leading_closes = 0usize;
    // Whether the line has produced an opening bracket or an atom yet. Only
    // closing brackets seen before that point count as "leading".
    let mut seen_code = false;
    // Whether the most recent code byte was an opening bracket.
    let mut last_code_was_open = false;

    let mut i = 0usize;
    while i < bytes.len() {
        let byte = bytes[i];

        if block_depth > 0 {
            if byte == b';' && bytes.get(i + 1) == Some(&b')') {
                block_depth -= 1;
                i += 2;
            } else if byte == b'(' && bytes.get(i + 1) == Some(&b';') {
                block_depth += 1;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        if in_string {
            match byte {
                // Skip the escaped character so `\"` does not end the literal.
                b'\\' => i += 2,
                b'"' => {
                    in_string = false;
                    i += 1;
                }
                _ => i += 1,
            }
            continue;
        }

        match byte {
            b'"' => {
                in_string = true;
                seen_code = true;
                last_code_was_open = false;
                i += 1;
            }
            b'(' => {
                // `(;` opens a block comment rather than a form.
                if bytes.get(i + 1) == Some(&b';') {
                    block_depth += 1;
                    i += 2;
                    continue;
                }
                opens += 1;
                seen_code = true;
                last_code_was_open = true;
                i += 1;
            }
            b')' => {
                closes += 1;
                if !seen_code {
                    leading_closes += 1;
                }
                last_code_was_open = false;
                i += 1;
            }
            b';' => {
                if bytes.get(i + 1) == Some(&b';') {
                    line_comment_at = Some(i);
                    break;
                }
                seen_code = true;
                last_code_was_open = false;
                i += 1;
            }
            b' ' | b'\t' | b'\r' => i += 1,
            _ => {
                seen_code = true;
                last_code_was_open = false;
                i += 1;
            }
        }
    }

    // Bracket-free block forms carry no bracket, so bracket depth cannot see
    // them. Detect them from the leading keywords of the code, excluding any
    // trailing `;;` comment so comment text stays out of the analysis.
    let code = match line_comment_at {
        Some(at) => &line[..at],
        None => line,
    };
    let linear = scan_linear_blocks(code, previous_ended_with_open_bracket);

    LineScan {
        line_comment_at,
        opens,
        closes,
        leading_closes,
        block_opens: linear.opens,
        block_closes: linear.closes,
        block_switches: linear.switches,
        starts_in_block_comment,
        block_depth_end: block_depth,
        folded_heads: linear.folded_heads,
        ends_with_open_bracket: last_code_was_open,
    }
}

/// Number of newline sequences at the very end of `source`.
///
/// Counts `\n` and `\r\n` alike so a CRLF document reports the same trailing
/// count as its LF equivalent.
fn trailing_newline_count(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut count = 0;
    let mut i = bytes.len();
    while i > 0 {
        if bytes[i - 1] != b'\n' {
            break;
        }
        count += 1;
        i -= 1;
        if i > 0 && bytes[i - 1] == b'\r' {
            i -= 1;
        }
    }
    count
}

/// How many newlines the output should end with, honouring the client's
/// `insertFinalNewline` / `trimFinalNewlines` preferences.
fn desired_trailing_newlines(source: &str, options: &FormatOptions) -> usize {
    if options.insert_final_newline {
        if options.trim_final_newlines {
            1
        } else {
            // Keep whatever the author already had, but never end a non-empty
            // file without a newline when the client asked for one.
            trailing_newline_count(source).max(1)
        }
    } else if options.trim_final_newlines {
        0
    } else {
        trailing_newline_count(source)
    }
}

/// Normalize indentation, whitespace, and blank-line runs.
///
/// This pass is idempotent: running it on its own output produces identical
/// text. It never inserts, removes, or reorders tokens, and never alters comment
/// text, so the result always parses to the same syntax tree as the input.
pub fn format_whitespace(source: &str, options: &FormatOptions) -> String {
    let options = options.sanitized();
    let unit = if options.insert_spaces {
        " ".repeat(options.tab_size)
    } else {
        "\t".to_string()
    };

    let mut lines: Vec<String> = Vec::new();
    let mut depth: usize = 0;
    let mut block_depth: usize = 0;
    let mut blank_run: usize = 0;
    // Carried across lines so a folded form wrapped right after its `(` is not
    // mistaken for the bracket-free spelling.
    let mut previous_ended_with_open_bracket = false;

    for raw_line in source.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let scan = scan_line(line, block_depth, previous_ended_with_open_bracket);
        block_depth = scan.block_depth_end;

        // Continuation lines inside a multi-line block comment keep their own
        // relative layout, because re-indenting them without knowing where the
        // comment belongs would move text the author aligned deliberately.
        if scan.starts_in_block_comment {
            lines.push(if options.trim_trailing_whitespace {
                line.trim_end().to_string()
            } else {
                line.to_string()
            });
            blank_run = 0;
            continue;
        }

        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= MAX_BLANK_LINES && !lines.is_empty() {
                lines.push(String::new());
            }
            continue;
        }
        blank_run = 0;

        // A line belongs to the enclosing level when it starts by closing a
        // form (a bracket or `end`), or when it is a sibling separator
        // (`else`, `catch`) between two bodies of the same form. Brackets and
        // `end` are mutually exclusive on one line, so taking the larger of the
        // two counts never double-dedents.
        // A folded head opens a level for its body, but the bracket that precedes
        // it already indented this line, so the line itself is pulled back one
        // level per folded head.
        let dedent_by = scan
            .leading_closes
            .max(scan.block_closes)
            .saturating_add(scan.block_switches)
            .saturating_add(scan.folded_heads);
        let indent = unit.repeat(depth.saturating_sub(dedent_by));

        // Everything after the original indentation; the indent above replaces it.
        let body = line.trim_start();
        let body_offset = line.len() - body.len();

        let rendered = match scan.line_comment_at {
            Some(at) => {
                // `at` indexes the original line; rebase it onto `body`.
                let split = at.saturating_sub(body_offset);
                let code = body[..split].trim_end();
                let comment = &body[split..];
                let comment = if options.trim_trailing_whitespace {
                    comment.trim_end()
                } else {
                    comment
                };

                if code.is_empty() {
                    // Whole-line comment: follows the current indentation.
                    // Section banners are not forced back to column zero; that
                    // convention belongs to the document author.
                    format!("{indent}{comment}")
                } else {
                    // Trailing comment: exactly one space after the code, so a
                    // second pass cannot keep shrinking the gap.
                    format!("{indent}{code} {comment}")
                }
            }
            None => {
                let body = if options.trim_trailing_whitespace {
                    body.trim_end()
                } else {
                    body
                };
                format!("{indent}{body}")
            }
        };
        lines.push(rendered);

        // A folded head shares its level with the bracket in front of it, which
        // already incremented `depth`. Counting the keyword as well would push the
        // body and every following line down one extra level.
        debug_assert!(
            scan.folded_heads <= scan.block_opens,
            "folded heads cannot exceed the levels opened"
        );
        depth = depth
            .saturating_add(scan.opens)
            .saturating_sub(scan.closes)
            .saturating_add(scan.block_opens.saturating_sub(scan.folded_heads))
            .saturating_sub(scan.block_closes);

        previous_ended_with_open_bracket = scan.ends_with_open_bracket;
    }

    // Trailing blank lines are decided by the final-newline options below, so
    // drop whatever the loop accumulated.
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    if lines.is_empty() {
        return String::new();
    }

    let mut result = lines.join("\n");
    for _ in 0..desired_trailing_newlines(source, &options) {
        result.push('\n');
    }
    result
}

/// Produce the formatting edit for a document.
///
/// Returns an empty edit list when the source is already formatted, so clients
/// do not receive a no-op replacement that marks the buffer dirty.
pub fn provide_formatting(source: &str, options: &FormatOptions) -> FormattingResult {
    let formatted = format_whitespace(source, options);

    if formatted == source {
        return FormattingResult { edits: Vec::new() };
    }

    // Replace the whole document rather than diffing line by line. Every
    // formatter in this space does the same, and it avoids the coordinate
    // mapping errors partial edits invite once line counts shift.
    let index = TextIndex::new(source);
    let end: Position = index.byte_to_utf16(source.len());

    FormattingResult {
        edits: vec![TextEdit {
            range: Range::new(Position::new(0, 0), end),
            new_text: formatted,
        }],
    }
}
