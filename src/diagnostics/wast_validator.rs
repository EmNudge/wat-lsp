use tower_lsp::lsp_types::*;

/// Validate WAT text using the wast crate for semantic errors
pub fn validate_wat(source: &str) -> Vec<Diagnostic> {
    if source.trim().is_empty() {
        return vec![];
    }

    // Parse with wast
    let buf = match wast::parser::ParseBuffer::new(source) {
        Ok(buf) => buf,
        Err(e) => return vec![wast_error_to_diagnostic(&e, source)],
    };

    match wast::parser::parse::<wast::Wat>(&buf) {
        Ok(_) => vec![], // Valid WAT
        Err(e) => vec![wast_error_to_diagnostic(&e, source)],
    }
}

fn wast_error_to_diagnostic(error: &wast::Error, source: &str) -> Diagnostic {
    let span = error.span();
    let (line, col) = span.linecol_in(source);

    Diagnostic {
        range: Range {
            start: Position {
                line: line as u32,
                character: col as u32,
            },
            end: Position {
                line: line as u32,
                character: (col + 1) as u32, // Extend by 1 char
            },
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
}
