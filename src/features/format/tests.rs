//! Tests for the whitespace formatter.
//!
//! Two properties are treated as non-negotiable and asserted broadly, because a
//! formatter that violates either is worse than no formatter at all:
//!
//! * **Idempotency** — `format(format(x)) == format(x)`. A formatter that keeps
//!   drifting on every save produces spurious diffs and destroys trust.
//! * **Token preservation** — stripping all whitespace must yield the same byte
//!   string before and after. This catches the class of bug that silently drops
//!   or reorders code, which a compile check alone would not catch for a
//!   document that was already invalid.

use super::*;

/// Format with default options (2-space indentation).
fn fmt(source: &str) -> String {
    format_whitespace(source, &FormatOptions::default())
}

/// Every non-whitespace byte, for token-preservation assertions.
fn tokens(source: &str) -> String {
    source
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
        .collect()
}

/// Assert the two invariants every input must satisfy.
fn assert_stable(source: &str) {
    let once = fmt(source);
    let twice = fmt(&once);
    assert_eq!(
        once, twice,
        "formatter is not idempotent.\n--- first pass ---\n{once}\n--- second pass ---\n{twice}"
    );
    assert_eq!(
        tokens(source),
        tokens(&once),
        "formatter changed the token stream.\n--- input ---\n{source}\n--- output ---\n{once}"
    );
}

// ============================================================================
// Indentation
// ============================================================================

#[test]
fn indents_nested_forms() {
    let source = "(module\n(func $f (result i32)\ni32.const 1))\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f (result i32)\n    i32.const 1))\n"
    );
}

#[test]
fn leading_closing_brackets_dedent_the_line() {
    // The line that closes `i32.add` is indented one level shallower than the
    // form's body, because it belongs to the enclosing level.
    let source = "(module\n(func $f\n(i32.add\n(i32.const 1)\n)\n)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    (i32.add\n      (i32.const 1)\n    )\n  )\n)\n"
    );
}

#[test]
fn multiple_leading_closing_brackets_dedent_by_their_count() {
    let source = "(module\n(func $f\n(i32.add\n(i32.const 1)\n))\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    (i32.add\n      (i32.const 1)\n  ))\n)\n"
    );
}

#[test]
fn trailing_closing_brackets_do_not_dedent_the_line() {
    // `(i32.const 2))))` is still inside `i32.add`; only *leading* brackets
    // affect the line's own indentation.
    let source = "(module\n(func $f\n(i32.add\n(i32.const 1)\n(i32.const 2))))\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    (i32.add\n      (i32.const 1)\n      (i32.const 2))))\n"
    );
}

#[test]
fn tab_indentation_is_honoured() {
    let options = FormatOptions {
        tab_size: 4,
        insert_spaces: false,
        ..Default::default()
    };
    let source = "(module\n(func $f)\n)\n";
    assert_eq!(
        format_whitespace(source, &options),
        "(module\n\t(func $f)\n)\n"
    );
}

#[test]
fn tab_size_is_clamped_to_a_usable_range() {
    // Zero would make indentation meaningless and huge values would allocate
    // absurdly; both are clamped rather than rejected.
    let zero = FormatOptions {
        tab_size: 0,
        ..Default::default()
    };
    assert_eq!(zero.sanitized().tab_size, 1);

    let huge = FormatOptions {
        tab_size: usize::MAX,
        ..Default::default()
    };
    assert_eq!(huge.sanitized().tab_size, 16);
}

// ============================================================================
// Comments
// ============================================================================

#[test]
fn block_comment_is_never_split_into_bracket_plus_comment() {
    // Regression guard: a naive scanner sees `(` followed by `;` and emits
    // `( ; ... ;)`, which is not valid WAT.
    let source = "(module\n(; comment ;)\n(func $f)\n)\n";
    let output = fmt(source);
    assert!(
        output.contains("(; comment ;)"),
        "block comment was rewritten:\n{output}"
    );
    assert!(
        !output.contains("( ;"),
        "block comment was split:\n{output}"
    );
    assert_stable(source);
}

#[test]
fn brackets_inside_block_comments_do_not_affect_depth() {
    let source = "(module\n(; ((( ))) ;)\n(func $f)\n)\n";
    assert_eq!(fmt(source), "(module\n  (; ((( ))) ;)\n  (func $f)\n)\n");
}

#[test]
fn nested_block_comments_are_tracked_by_depth() {
    let source = "(module\n(; outer (; inner ;) still outer ;)\n(func $f)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (; outer (; inner ;) still outer ;)\n  (func $f)\n)\n"
    );
    assert_stable(source);
}

#[test]
fn multi_line_block_comment_continuation_is_verbatim() {
    // The author's alignment inside the comment survives; only the first line
    // receives formatter indentation.
    let source = "(module\n(; first\n     second\n  ;)\n(func $f)\n)\n";
    let output = fmt(source);
    assert!(
        output.contains("\n     second\n"),
        "continuation indentation was rewritten:\n{output}"
    );
    assert_eq!(
        output,
        "(module\n  (; first\n     second\n  ;)\n  (func $f)\n)\n"
    );
    assert_stable(source);
}

#[test]
fn semicolons_inside_strings_do_not_start_a_comment() {
    let source = "(module\n(data (i32.const 0) \"a;b ;; c\")\n)\n";
    let output = fmt(source);
    assert!(
        output.contains("\"a;b ;; c\""),
        "string literal was truncated:\n{output}"
    );
    assert_stable(source);
}

#[test]
fn brackets_inside_strings_do_not_affect_depth() {
    let source = "(module\n(data (i32.const 0) \"((( \")\n(func $f)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (data (i32.const 0) \"((( \")\n  (func $f)\n)\n"
    );
}

#[test]
fn escaped_quotes_do_not_end_the_string() {
    let source = "(module\n(data (i32.const 0) \"a\\\"b ;; x\")\n)\n";
    let output = fmt(source);
    assert!(
        output.contains("\"a\\\"b ;; x\""),
        "escaped quote handling is wrong:\n{output}"
    );
}

#[test]
fn whole_line_comment_follows_indentation_without_banner_special_casing() {
    // A banner is not forced to column zero: the formatter has no business
    // overriding the author's section layout.
    let source = "(module\n(func $f\n;; ---- section ----\nnop)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    ;; ---- section ----\n    nop)\n)\n"
    );
}

#[test]
fn trailing_comment_keeps_one_space_separator() {
    let source = "(module\n(func $f)    ;; note\n)\n";
    assert_eq!(fmt(source), "(module\n  (func $f) ;; note\n)\n");
}

#[test]
fn comment_text_after_the_marker_is_untouched() {
    let source = "(module\n;;  spaced   text\n)\n";
    assert_eq!(fmt(source), "(module\n  ;;  spaced   text\n)\n");
}

// ============================================================================
// Blank lines and whitespace
// ============================================================================

#[test]
fn blank_line_runs_collapse_to_one() {
    let source = "(module\n\n\n\n(func $f)\n)\n";
    assert_eq!(fmt(source), "(module\n\n  (func $f)\n)\n");
}

#[test]
fn leading_blank_lines_are_removed() {
    let source = "\n\n(module\n)\n";
    assert_eq!(fmt(source), "(module\n)\n");
}

#[test]
fn trailing_blank_lines_are_removed() {
    let source = "(module\n)\n\n\n";
    assert_eq!(fmt(source), "(module\n)\n");
}

#[test]
fn trailing_whitespace_is_stripped() {
    let source = "(module   \n(func $f)  \n)   \n";
    assert_eq!(fmt(source), "(module\n  (func $f)\n)\n");
}

#[test]
fn crlf_line_endings_are_normalized() {
    let source = "(module\r\n(func $f)\r\n)\r\n";
    assert_eq!(fmt(source), "(module\n  (func $f)\n)\n");
}

#[test]
fn empty_input_stays_empty() {
    assert_eq!(fmt(""), "");
    assert_eq!(fmt("\n\n"), "");
}

// ============================================================================
// Client formatting options
// ============================================================================

fn fmt_with(source: &str, options: FormatOptions) -> String {
    format_whitespace(source, &options)
}

#[test]
fn trim_trailing_whitespace_defaults_to_on() {
    let source = "(module   \n)\n";
    assert_eq!(fmt(source), "(module\n)\n");
}

#[test]
fn trim_trailing_whitespace_off_keeps_trailing_spaces() {
    let options = FormatOptions {
        trim_trailing_whitespace: false,
        ..Default::default()
    };
    let source = "(module   \n)\n";
    assert_eq!(fmt_with(source, options), "(module   \n)\n");
}

#[test]
fn insert_final_newline_defaults_to_on() {
    let source = "(module\n)";
    assert_eq!(fmt(source), "(module\n)\n");
}

#[test]
fn insert_final_newline_off_drops_the_final_newline() {
    let options = FormatOptions {
        insert_final_newline: false,
        ..Default::default()
    };
    assert_eq!(fmt_with("(module\n)\n", options), "(module\n)");
}

#[test]
fn trim_final_newlines_defaults_to_on() {
    let source = "(module\n)\n\n\n";
    assert_eq!(fmt(source), "(module\n)\n");
}

#[test]
fn trim_final_newlines_off_preserves_the_author_count() {
    let options = FormatOptions {
        trim_final_newlines: false,
        ..Default::default()
    };
    assert_eq!(fmt_with("(module\n)\n\n\n", options), "(module\n)\n\n\n");
}

#[test]
fn trim_final_newlines_off_still_adds_a_missing_newline() {
    // `insertFinalNewline` is still on, so a file with no final newline gains
    // exactly one rather than staying without one.
    let options = FormatOptions {
        trim_final_newlines: false,
        ..Default::default()
    };
    assert_eq!(fmt_with("(module\n)", options), "(module\n)\n");
}

#[test]
fn options_remain_idempotent_in_every_combination() {
    let sources = [
        "(module   \n)\n\n\n",
        "(module\n)",
        "(module\n(func $f)   \n)\n",
        "(module\n(; c ;)   \n)\n",
    ];
    for source in sources {
        for trim_trailing_whitespace in [true, false] {
            for insert_final_newline in [true, false] {
                for trim_final_newlines in [true, false] {
                    let options = FormatOptions {
                        trim_trailing_whitespace,
                        insert_final_newline,
                        trim_final_newlines,
                        ..Default::default()
                    };
                    let once = format_whitespace(source, &options);
                    let twice = format_whitespace(&once, &options);
                    assert_eq!(
                        once, twice,
                        "not idempotent for {options:?}\n--- input ---\n{source:?}\n--- once ---\n{once:?}\n--- twice ---\n{twice:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn comment_only_document_is_preserved() {
    let source = ";; just a comment\n";
    assert_eq!(fmt(source), ";; just a comment\n");
    assert_stable(source);
}

// ============================================================================
// Token preservation
// ============================================================================

#[test]
fn inter_token_spacing_is_left_alone() {
    // Normalizing spaces between tokens is a layout decision that requires
    // understanding of the form; this pass deliberately does not make it.
    let source = "(module\n(func $f (param $a  i32)   (result i32) nop)\n)\n";
    let output = fmt(source);
    assert!(
        output.contains("(param $a  i32)   (result i32)"),
        "inter-token spacing was rewritten:\n{output}"
    );
    assert_stable(source);
}

#[test]
fn already_formatted_document_produces_no_edit() {
    let source = "(module\n  (func $f)\n)\n";
    let result = provide_formatting(source, &FormatOptions::default());
    assert!(
        result.edits.is_empty(),
        "formatted input produced an edit: {:?}",
        result.edits
    );
}

#[test]
fn unformatted_document_produces_one_whole_document_edit() {
    let source = "(module\n(func $f)\n)\n";
    let result = provide_formatting(source, &FormatOptions::default());

    assert_eq!(result.edits.len(), 1);
    let edit = &result.edits[0];
    assert_eq!(edit.range.start, Position::new(0, 0));
    assert_eq!(edit.new_text, "(module\n  (func $f)\n)\n");
}

#[test]
fn edit_range_end_lands_at_end_of_document() {
    // A trailing newline puts the end position at column 0 of the line after
    // the last one, which is what LSP expects for a whole-document replace.
    let source = "(module\n(func $f)\n)\n";
    let result = provide_formatting(source, &FormatOptions::default());
    let edit = &result.edits[0];

    assert_eq!(edit.range.start, Position::new(0, 0));
    assert_eq!(edit.range.end, Position::new(3, 0));
}

#[test]
fn edit_range_end_uses_utf16_units_not_bytes() {
    // The end position must be expressed in UTF-16 code units like every other
    // LSP position. A non-ASCII final line distinguishes UTF-16 from bytes.
    let source = "(module\n;; 注释\n)   ";
    let result = provide_formatting(source, &FormatOptions::default());
    let edit = &result.edits[0];

    // Final line is `)   ` — three trailing spaces make the source unformatted.
    // In UTF-16 the line is 4 units wide; in UTF-8 it is 4 bytes too, so the
    // multibyte line above is what proves the conversion runs.
    assert_eq!(edit.range.end, Position::new(2, 4));

    // Sanity-check the conversion on the multibyte line itself. `;; ` is 3
    // units, each CJK ideograph is 1 UTF-16 code unit but 3 UTF-8 bytes, so the
    // line is 5 UTF-16 units wide and 9 bytes wide.
    let index = TextIndex::new(source);
    let after_comment = source.find('\n').unwrap() + 1 + ";; 注释".len();
    assert_eq!(
        index.byte_to_utf16(after_comment),
        Position::new(1, 5),
        "UTF-8 byte width was used instead of UTF-16 units"
    );
}

// ============================================================================
// Idempotency and token preservation over a broad corpus
// ============================================================================

#[test]
fn invariants_hold_across_representative_inputs() {
    let cases = [
        // Folded form
        "(module (func $f (result i32) (i32.add (i32.const 1) (i32.const 2))))\n",
        // Linear form
        "(module\n(func $f (result i32)\ni32.const 1\ni32.const 2\ni32.add)\n)\n",
        // Plain/inline exports and type uses
        "(module\n(func $f (export \"f\") (param $x i32) (result i32) (local.get $x))\n)\n",
        // Block comments: inline, nested, and multi-line
        "(module (; inline ;) (func $f))\n",
        "(module (; a (; b ;) c ;) (func $f))\n",
        "(module\n(; l1\n   l2\n ;)\n(func $f)\n)\n",
        // Strings containing syntax-significant characters
        "(module\n(data (i32.const 0) \"(; ;) ;; \\\"\")\n)\n",
        // Unbalanced input, as happens while editing
        "(module\n(func $f\n",
        // Excess closing brackets in the input
        "(module\n)\n)\n)\n",
        // Annotations (a WAT-specific extra)
        "(module\n(@custom \"x\" \"y\")\n)\n",
        // Wast script-style multiple modules
        "(module)\n(module)\n",
    ];

    for source in cases {
        assert_stable(source);
    }
}

#[test]
fn unclosed_forms_do_not_panic_or_drift() {
    // Saving mid-edit is routine; the formatter must stay well behaved even
    // when the input is not valid WAT.
    let source = "(module\n(func $f\n(block\n(loop\n";
    assert_stable(source);
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    (block\n      (loop\n"
    );
}

#[test]
fn excess_closing_brackets_never_produce_negative_indentation() {
    let source = ")\n)\n)\n";
    let output = fmt(source);
    assert_eq!(output, ")\n)\n)\n");
    assert_stable(source);
}

// ============================================================================
// Linear (bracket-free) block forms
// ============================================================================

#[test]
fn linear_block_body_is_indented_between_block_and_end() {
    // `wasm-tools print` spells instruction sequences this way by default, so
    // this is the shape most real input arrives in.
    let source = "(module\n(func $f (result i32)\nblock $done\nloop $continue\nbr_if $done\nend\nend\nlocal.get $x)\n)\n";
    assert_eq!(
        fmt(source),
        concat!(
            "(module\n",
            "  (func $f (result i32)\n",
            "    block $done\n",
            "      loop $continue\n",
            "        br_if $done\n",
            "      end\n",
            "    end\n",
            "    local.get $x)\n",
            ")\n",
        )
    );
    assert_stable(source);
}

#[test]
fn linear_if_else_body_is_indented() {
    let source =
        "(module\n(func $f\nif (result i32)\ni32.const 1\nelse\ni32.const 2\nend\ndrop)\n)\n";
    assert_eq!(
        fmt(source),
        concat!(
            "(module\n",
            "  (func $f\n",
            "    if (result i32)\n",
            "      i32.const 1\n",
            "    else\n",
            "      i32.const 2\n",
            "    end\n",
            "    drop)\n",
            ")\n",
        )
    );
    assert_stable(source);
}

#[test]
fn linear_try_catch_body_is_indented() {
    let source = "(module\n(func $f\ntry\nnop\ncatch 0\nnop\ncatch_all\nnop\nend)\n)\n";
    assert_eq!(
        fmt(source),
        concat!(
            "(module\n",
            "  (func $f\n",
            "    try\n",
            "      nop\n",
            "    catch 0\n",
            "      nop\n",
            "    catch_all\n",
            "      nop\n",
            "    end)\n",
            ")\n",
        )
    );
    assert_stable(source);
}

#[test]
fn linear_end_with_label_is_recognized() {
    // `end $label` is legal; the label must not keep the line at the inner level.
    let source = "(module\n(func $f\nblock $b\nnop\nend $b)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    block $b\n      nop\n    end $b)\n)\n"
    );
    assert_stable(source);
}

#[test]
fn linear_end_flush_against_a_bracket_is_recognized() {
    // Closing a form flush against the enclosing bracket is common output from
    // `wasm-tools print`; the token then carries the bracket with it.
    let source = "(module\n(func $f\nblock $b\nnop\nend)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    block $b\n      nop\n    end)\n)\n"
    );
    assert_stable(source);
}

#[test]
fn linear_block_forms_stay_idempotent_and_token_identical() {
    // Whatever the indentation outcome, the two invariants must hold for linear
    // forms exactly as they do for folded ones.
    let cases = [
        "(module\n(func $f\nif (result i32)\ni32.const 1\nelse\ni32.const 2\nend\ndrop)\n)\n",
        "(module\n(func $f\ntry\nnop\ncatch 0\nnop\ncatch_all\nnop\nend)\n)\n",
        "(module\n(func $f\nblock $b\nnop\nend $b)\n)\n",
    ];
    for source in cases {
        assert_stable(source);
    }
}

#[test]
fn identifiers_and_comments_containing_keywords_do_not_affect_indentation() {
    // No token content is interpreted, so a `$block` identifier and the word
    // "block" inside a comment are inert.
    let source = "(module\n(func $f\n;; block inside a comment\nlocal.get $block\n)\n)\n";
    assert_eq!(
        fmt(source),
        concat!(
            "(module\n",
            "  (func $f\n",
            "    ;; block inside a comment\n",
            "    local.get $block\n",
            "  )\n",
            ")\n",
        )
    );
    assert_stable(source);
}

#[test]
fn folded_block_form_indents_by_brackets() {
    let source = "(module\n(func $f\n(block $b\nnop)\n)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func $f\n    (block $b\n      nop)\n  )\n)\n"
    );
    assert_stable(source);
}

// ============================================================================
// Keyword table guard
// ============================================================================

/// Extract the opening keyword of every `block_*` rule in the grammar.
///
/// A block rule is written as `block_xxx: $ => seq("keyword", ..., "end", ...)`.
/// The grammar is the single source of truth for which bracket-free block forms
/// exist, so the formatter's keyword table must stay a subset of it.
fn grammar_block_keywords() -> Vec<String> {
    let grammar = include_str!("../../../grammars/tree-sitter-wat/grammar.js");

    let mut keywords = Vec::new();
    for line in grammar.lines() {
        let trimmed = line.trim_start();
        // Rule headers look like `block_loop: $ =>` or `block_try_table: $ =>`.
        let Some(rest) = trimmed.strip_prefix("block_") else {
            continue;
        };
        if !rest.contains(": $ =>") {
            continue;
        }

        // The keyword is the first double-quoted string on the following lines.
        let start = grammar.find(trimmed).unwrap_or(0);
        let body = &grammar[start..];
        let Some(open) = body.find('"') else {
            continue;
        };
        let Some(close) = body[open + 1..].find('"') else {
            continue;
        };
        keywords.push(body[open + 1..open + 1 + close].to_string());
    }

    keywords.sort();
    keywords.dedup();
    keywords
}

#[test]
fn block_keywords_cover_grammar() {
    // If upstream adds a bracket-free block form, this fails and points at the
    // missing keyword instead of silently mis-indenting the new construct.
    let grammar = grammar_block_keywords();
    assert!(
        !grammar.is_empty(),
        "failed to extract any block_* rules from the grammar; the parser above needs updating"
    );

    let known: Vec<&str> = LINEAR_BLOCK_OPENERS.to_vec();
    let missing: Vec<&String> = grammar
        .iter()
        .filter(|k| !known.contains(&k.as_str()))
        .collect();

    assert!(
        missing.is_empty(),
        "grammar defines block forms the formatter does not recognise: {missing:?}\n\
         Add them to LINEAR_BLOCK_OPENERS in src/features/format/mod.rs."
    );
}

#[test]
fn block_keyword_table_has_no_stale_entries() {
    // The reverse direction: every keyword the formatter treats as a block
    // opener must still exist in the grammar. Catches removals and typos.
    let grammar = grammar_block_keywords();
    let stale: Vec<&&str> = LINEAR_BLOCK_OPENERS
        .iter()
        .filter(|k| !grammar.contains(&k.to_string()))
        .collect();

    assert!(
        stale.is_empty(),
        "formatter lists block openers the grammar no longer defines: {stale:?}"
    );
}

#[test]
fn folded_spellings_are_not_in_the_opener_table() {
    // `(block ...)` opens a bracket, so `scan_line` already counts it. Listing it
    // as a linear opener as well would double-count the level.
    //
    // The keyword sits after `(` here, so it never starts a line and therefore
    // never reaches `scan_linear_blocks`; the assertion below pins that down by
    // checking the body is exactly one level inside the form.
    for keyword in LINEAR_BLOCK_OPENERS {
        let source = format!("(module\n(func\n({keyword}\nnop)\n)\n)\n");
        let once = fmt(&source);
        let twice = fmt(&once);
        assert_eq!(once, twice, "folded `{keyword}` is not idempotent");
        assert_eq!(
            once,
            format!("(module\n  (func\n    ({keyword}\n      nop)\n  )\n)\n"),
            "folded `{keyword}` is double-counted"
        );
    }
}

// ============================================================================
// Folded forms wrapped after an open bracket
// ============================================================================

/// Leading spaces of the first line whose trimmed form starts with `needle`.
fn indent_of(formatted: &str, needle: &str) -> usize {
    formatted
        .lines()
        .find(|l| l.trim_start().starts_with(needle))
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or_else(|| panic!("no line starting with {needle:?}:\n{formatted}"))
}

#[test]
fn folded_form_wrapped_after_an_open_bracket_indents_like_the_single_line_form() {
    // `(block` and `(\nblock` are the same folded form. The keyword shares its
    // level with the bracket, and its body sits one level inside.
    //
    // Regression guard: the wrapped shape used to push the keyword and its whole
    // body down a level, and left the closing brackets over-indented.
    let wrapped = "(module\n(func\n(\nblock\nnop)\n)\n)\n";
    let single = "(module\n(func\n(block\nnop)\n)\n)\n";

    let wrapped_formatted = fmt(wrapped);
    let single_formatted = fmt(single);

    // The keyword aligns with the lone `(` rather than nesting inside it.
    let lone_bracket = wrapped_formatted
        .lines()
        .find(|l| l.trim() == "(")
        .map(|l| l.len() - l.trim_start().len())
        .expect("the lone bracket line survives");
    assert_eq!(
        indent_of(&wrapped_formatted, "block"),
        lone_bracket,
        "the keyword nested inside its own bracket:\n{wrapped_formatted}"
    );

    // The body is one level inside the keyword, matching the single-line form.
    assert_eq!(
        indent_of(&wrapped_formatted, "nop"),
        indent_of(&single_formatted, "nop"),
        "the wrapped form's body drifted:\n{wrapped_formatted}"
    );

    // Closing brackets land at the same levels as the single-line form.
    let wrapped_closers: Vec<usize> = wrapped_formatted
        .lines()
        .filter(|l| l.trim() == ")")
        .map(|l| l.len() - l.trim_start().len())
        .collect();
    let single_closers: Vec<usize> = single_formatted
        .lines()
        .filter(|l| l.trim() == ")")
        .map(|l| l.len() - l.trim_start().len())
        .collect();
    assert_eq!(
        wrapped_closers, single_closers,
        "closing brackets shifted:\n{wrapped_formatted}"
    );

    assert_stable(wrapped);
}

#[test]
fn bracket_free_block_is_unaffected_by_the_folded_head_rule() {
    // A keyword with no bracket in front of it is the bracket-free spelling and
    // still opens a level, so the fix above cannot leak into it.
    let source = "(module\n(func\nblock\nnop\nend\n)\n)\n";
    assert_eq!(
        fmt(source),
        "(module\n  (func\n    block\n      nop\n    end\n  )\n)\n"
    );
    assert_stable(source);
}

#[test]
fn wrapped_folded_state_does_not_leak_into_later_lines() {
    // Once the wrapped form closes, a bracket-free block later in the same file
    // must open its own level again.
    let source = "(module\n(func\n(\nblock\nnop)\n)\n(func\nblock\nnop\nend\n)\n)\n";
    let output = fmt(source);
    assert!(
        output.contains("\n    block\n      nop\n    end\n"),
        "the later bracket-free block lost its level:\n{output}"
    );
    assert_stable(source);
}

#[test]
fn blank_line_after_an_open_bracket_keeps_the_folded_head_state() {
    // A blank line does not close a bracket, so the keyword is still a folded head.
    let wrapped = "(module\n(func\n(\n\nblock\nnop)\n)\n)\n";
    let output = fmt(wrapped);
    assert_eq!(
        indent_of(&output, "block"),
        output
            .lines()
            .find(|l| l.trim() == "(")
            .map(|l| l.len() - l.trim_start().len())
            .expect("the lone bracket line survives"),
        "a blank line changed how the folded form was counted:\n{output}"
    );
    assert_stable(wrapped);
}
