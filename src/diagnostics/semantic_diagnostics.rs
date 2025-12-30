use crate::symbols::SymbolTable;
use crate::utils::find_containing_function;
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

/// Provide semantic diagnostics for undefined references
pub fn provide_semantic_diagnostics(
    tree: &Tree,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    walk_tree_for_undefined_references(tree.root_node(), source, symbols, &mut diagnostics);
    diagnostics
}

#[derive(Debug, PartialEq)]
enum ReferenceContext {
    Branch, // br, br_if, br_table
    Call,   // call
    Local,  // local.get, local.set, local.tee
    Global, // global.get, global.set
    Table,  // table operations
    Type,   // type use
    Unknown,
}

/// Recursively walk the tree looking for undefined references
fn walk_tree_for_undefined_references(
    node: Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Determine if this node is a reference instruction
    let context = determine_reference_context(&node, source);

    if context != ReferenceContext::Unknown {
        // Check for undefined references in this instruction
        check_references(&node, source, symbols, diagnostics, &context);

        // For branch instructions, still recurse to find nested instructions (like local.get inside br_if)
        // For other instructions, don't recurse to avoid checking the same identifier multiple times
        if context != ReferenceContext::Branch {
            return;
        }
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_undefined_references(child, source, symbols, diagnostics);
    }
}

/// Determine the reference context from a node
fn determine_reference_context(node: &Node, source: &str) -> ReferenceContext {
    let kind = node.kind();

    // Only check instr_plain nodes, not expr1_plain to avoid duplicates
    // (expr1_plain contains instr_plain, so we'd check the same instruction twice)
    if kind == "instr_plain" {
        let text = &source[node.byte_range()];

        if text.starts_with("br") || text.contains(" br") {
            return ReferenceContext::Branch;
        } else if text.contains("call") && !text.contains("call_indirect") {
            return ReferenceContext::Call;
        } else if text.contains("local.") {
            return ReferenceContext::Local;
        } else if text.contains("global.") {
            return ReferenceContext::Global;
        } else if text.contains("table.") {
            return ReferenceContext::Table;
        }
    }

    // Check for type use context
    if kind == "type_use" {
        return ReferenceContext::Type;
    }

    ReferenceContext::Unknown
}

/// Check if references in this instruction are defined
fn check_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
    context: &ReferenceContext,
) {
    find_undefined_identifiers(node, source, symbols, diagnostics, context);
}

/// Recursively find identifier nodes and check if they're defined
fn find_undefined_identifiers(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
    context: &ReferenceContext,
) {
    if node.kind() == "identifier" {
        let identifier_name = &source[node.byte_range()];

        // Only check identifiers that start with $
        if !identifier_name.starts_with('$') {
            return;
        }

        // Find the containing function for this reference (needed for locals and labels)
        let start_point = node.start_position();
        let position = Position {
            line: start_point.row as u32,
            character: start_point.column as u32,
        };

        let is_defined = match context {
            ReferenceContext::Branch => {
                // Check if label exists in containing function
                if let Some(func) = find_containing_function(symbols, position) {
                    func.blocks.iter().any(|block| {
                        format!("${}", block.label) == identifier_name
                            || block.label == identifier_name
                    })
                } else {
                    false
                }
            }
            ReferenceContext::Call => {
                // Check if function exists
                symbols.get_function_by_name(identifier_name).is_some()
            }
            ReferenceContext::Local => {
                // Check if local or parameter exists in containing function
                if let Some(func) = find_containing_function(symbols, position) {
                    func.parameters
                        .iter()
                        .any(|p| p.name.as_ref() == Some(&identifier_name.to_string()))
                        || func
                            .locals
                            .iter()
                            .any(|l| l.name.as_ref() == Some(&identifier_name.to_string()))
                } else {
                    false
                }
            }
            ReferenceContext::Global => {
                // Check if global exists
                symbols.get_global_by_name(identifier_name).is_some()
            }
            ReferenceContext::Table => {
                // Check if table exists
                symbols.get_table_by_name(identifier_name).is_some()
            }
            ReferenceContext::Type => {
                // Check if type exists
                symbols.get_type_by_name(identifier_name).is_some()
            }
            ReferenceContext::Unknown => true, // Don't flag unknowns
        };

        if !is_defined {
            let diagnostic = create_undefined_reference_diagnostic(node, identifier_name, context);
            diagnostics.push(diagnostic);
        }
        return;
    }

    // For branch instructions, only check the index node (the label argument),
    // not identifiers in nested expressions (like local.get inside br_if condition)
    if *context == ReferenceContext::Branch && node.kind() == "expr" {
        // Don't recurse into expr nodes for branch instructions
        // The label is in the index node which is a sibling, not a child of expr
        return;
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_undefined_identifiers(&child, source, symbols, diagnostics, context);
    }
}

/// Create a diagnostic for an undefined reference
fn create_undefined_reference_diagnostic(
    node: &Node,
    identifier_name: &str,
    context: &ReferenceContext,
) -> Diagnostic {
    let range = node_to_range(node);

    let message = match context {
        ReferenceContext::Branch => format!("Undefined label '{}'", identifier_name),
        ReferenceContext::Call => format!("Undefined function '{}'", identifier_name),
        ReferenceContext::Local => format!("Undefined local or parameter '{}'", identifier_name),
        ReferenceContext::Global => format!("Undefined global '{}'", identifier_name),
        ReferenceContext::Table => format!("Undefined table '{}'", identifier_name),
        ReferenceContext::Type => format!("Undefined type '{}'", identifier_name),
        ReferenceContext::Unknown => format!("Undefined reference '{}'", identifier_name),
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("wat-lsp".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Convert a tree-sitter node to an LSP range
fn node_to_range(node: &Node) -> Range {
    let start_point = node.start_position();
    let end_point = node.end_position();

    Range {
        start: Position {
            line: start_point.row as u32,
            character: start_point.column as u32,
        },
        end: Position {
            line: end_point.row as u32,
            character: end_point.column as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_document;
    use crate::tree_sitter_bindings::create_parser;

    #[test]
    fn test_valid_label_reference() {
        let document = r#"(func $test
  (block $myblock
    (br $myblock)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid label reference should have no diagnostics"
        );
    }

    #[test]
    fn test_undefined_label_reference() {
        let document = r#"(func $test
  (block $myblock
    (br $undefined)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            1,
            "Undefined label should produce one diagnostic"
        );
        assert!(diagnostics[0].message.contains("Undefined label"));
        assert!(diagnostics[0].message.contains("$undefined"));
    }

    #[test]
    fn test_nested_blocks_valid_reference() {
        let document = r#"(func $test
  (block $outer
    (block $inner
      (br $outer)
      (br $inner))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid nested label references should have no diagnostics"
        );
    }

    #[test]
    fn test_numeric_label_reference() {
        // Numeric references (0, 1, 2) are valid and refer to block depth
        // We should not produce diagnostics for these
        let document = r#"(func $test
  (block
    (br 0)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Numeric label references should not produce diagnostics"
        );
    }

    #[test]
    fn test_undefined_function_call() {
        let document = r#"(func $defined
  (nop))

(func $test
  (call $defined)
  (call $undefined))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            1,
            "Undefined function should produce one diagnostic"
        );
        assert!(diagnostics[0].message.contains("Undefined function"));
        assert!(diagnostics[0].message.contains("$undefined"));
    }

    #[test]
    fn test_undefined_local() {
        let document = r#"(func $test (param $x i32) (local $y i32)
  (local.get $x)
  (local.get $y)
  (local.set $undefined))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            1,
            "Undefined local should produce one diagnostic"
        );
        assert!(diagnostics[0].message.contains("Undefined local"));
        assert!(diagnostics[0].message.contains("$undefined"));
    }

    #[test]
    fn test_undefined_global() {
        let document = r#"(global $g i32 (i32.const 42))

(func $test
  (global.get $g)
  (global.set $undefined (i32.const 0)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            1,
            "Undefined global should produce one diagnostic"
        );
        assert!(diagnostics[0].message.contains("Undefined global"));
        assert!(diagnostics[0].message.contains("$undefined"));
    }

    #[test]
    fn test_all_valid_references() {
        let document = r#"(global $g i32 (i32.const 0))

(func $helper
  (nop))

(func $test (param $p i32) (local $l i32)
  (block $label
    (local.get $p)
    (local.set $l)
    (global.get $g)
    (call $helper)
    (br $label)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "All valid references should produce no diagnostics"
        );
    }

    #[test]
    fn test_br_if_with_local_get() {
        // Regression test: local.get inside br_if condition should not be flagged as undefined label
        let document = r#"(func $test (param $n i32)
    (local $i i32)
    (block $break
      (br_if $break
        (i32.gt_s (local.get $i) (local.get $n)))))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics.len(),
            0,
            "Valid local.get inside br_if should not produce diagnostics"
        );
    }
}
