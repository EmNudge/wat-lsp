use tower_lsp::lsp_types::*;

/// Validate WAT text using the wast crate for semantic errors.
/// Tries single-module WAT parsing first, then falls back to WAST script format
/// for multi-module documents.
pub fn validate_wat(source: &str) -> Vec<Diagnostic> {
    if source.trim().is_empty() {
        return vec![];
    }

    // Parse with wast
    let buf = match wast::parser::ParseBuffer::new(source) {
        Ok(buf) => buf,
        Err(e) => return vec![wast_error_to_diagnostic(&e, source)],
    };

    // Try single-module WAT first
    if wast::parser::parse::<wast::Wat>(&buf).is_ok() {
        return vec![];
    }

    // Fall back to WAST script format (multi-module)
    let buf = match wast::parser::ParseBuffer::new(source) {
        Ok(buf) => buf,
        Err(e) => return vec![wast_error_to_diagnostic(&e, source)],
    };

    match wast::parser::parse::<wast::Wast>(&buf) {
        Ok(_) => vec![], // Valid WAST script
        Err(e) => vec![wast_error_to_diagnostic(&e, source)],
    }
}

fn wast_error_to_diagnostic(error: &wast::Error, source: &str) -> Diagnostic {
    // Keep this adapter's result in byte columns, like the other diagnostic
    // producers. Native publication converts the merged result to UTF-16 once.
    let index = crate::core::text::TextIndex::new(source);
    let start = index.point_to_byte(index.byte_to_point(error.span().offset()));
    let width = source[start..]
        .chars()
        .next()
        .filter(|ch| *ch != '\r' && *ch != '\n')
        .map_or(0, char::len_utf8);

    Diagnostic {
        range: Range {
            start: index.byte_to_point(start).into(),
            end: index.byte_to_point(start + width).into(),
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("wast-validator".to_string()),
        message: error.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_module_no_errors() {
        let source = r#"(module (func $test (result i32) i32.const 42))"#;
        let diags = validate_wat(source);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_empty_source() {
        assert_eq!(validate_wat("").len(), 0);
    }

    #[test]
    fn test_ref_null_func_and_ref_is_null() {
        let source = r#"(module
  (func $test (result i32)
    ref.null func
    ref.is_null
  )
)"#;
        let diags = validate_wat(source);
        assert!(diags.is_empty(), "Expected no errors, got: {:?}", diags);
    }

    #[test]
    fn test_ref_func() {
        let source = r#"(module
  (func $target)
  (func $test (result funcref)
    ref.func $target
  )
)"#;
        let diags = validate_wat(source);
        assert!(diags.is_empty(), "Expected no errors, got: {:?}", diags);
    }

    #[test]
    fn test_elem_with_ref_expressions() {
        let source = r#"(module
  (func $f1)
  (table 2 funcref)
  (elem (i32.const 0) funcref (ref.func $f1) (ref.null func))
)"#;
        let diags = validate_wat(source);
        assert!(diags.is_empty(), "Expected no errors, got: {:?}", diags);
    }

    #[test]
    fn test_try_table_exception_handling() {
        let source = r#"
(module
  (tag $div_error (param i32))

  (func $safe_div (param $a i32) (param $b i32) (result i32)
    (block $caught (result i32)
      (try_table (result i32) (catch $div_error $caught)
        (if (i32.eqz (local.get $b))
          (then (throw $div_error (i32.const 400)))
        )
        (i32.div_s (local.get $a) (local.get $b))
      )
    )
  )

  (export "safeDiv" (func $safe_div))
)
"#;
        let diags = validate_wat(source);
        assert!(diags.is_empty(), "Expected no errors, got: {:?}", diags);
    }

    #[test]
    fn test_unicode_name_strings_no_false_errors() {
        // Name strings (quoted export/import names) are UTF-8 and may contain
        // arbitrary valid Unicode. The native `wast`-crate validator must accept
        // them without emitting false diagnostics. Each of these validates under
        // `wasm-tools validate --features all`.
        for source in [
            r#"(module (func (export "café_ñ_🎉_日本語")))"#,
            r#"(module (func (export "\u{1F389}\u{00e9}")))"#,
            r#"(module (import "wåsî" "función_🚀" (func)))"#,
            r#"(module (global (export "π_value") i32 (i32.const 0)))"#,
        ] {
            let diags = validate_wat(source);
            assert!(
                diags.is_empty(),
                "Expected no errors for {:?}, got: {:?}",
                source,
                diags
            );
        }
    }

    #[test]
    fn test_error_positioning_past_multibyte_name_string() {
        // An error following a multi-byte name string must not panic on byte
        // slicing and must land on the offending token. This pins the UTF-8-safe
        // byte/char math in `wast_error_to_diagnostic`.
        let source = r#"(module (func (export "café_🎉")) (func $x i32.const))"#;
        let diags = validate_wat(source);
        assert_eq!(
            diags.len(),
            1,
            "expected exactly one error, got: {:?}",
            diags
        );
        // The error is the incomplete `i32.const`, located after the Unicode name.
        assert!(
            diags[0].range.start.character > 0,
            "error should be positioned within the source, got {:?}",
            diags[0].range
        );
    }
}
