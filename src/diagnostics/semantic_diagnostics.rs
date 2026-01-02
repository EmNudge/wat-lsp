use crate::diagnostics::instruction_metadata::get_instruction_arity_map;
use crate::symbols::SymbolTable;
use crate::utils::find_containing_function;
use std::sync::OnceLock;
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

/// Provide semantic diagnostics for undefined references and parameter count validation
pub fn provide_semantic_diagnostics(
    tree: &Tree,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    walk_tree_for_undefined_references(tree.root_node(), source, symbols, &mut diagnostics);
    walk_tree_for_parameter_counts(tree.root_node(), source, symbols, &mut diagnostics);
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
    Tag,    // tag use (throw)
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
        } else if text.contains("throw") || text.contains("rethrow") {
            return ReferenceContext::Tag;
        }
    }

    // Check for type use context or tag use context
    if kind == "type_use" {
        return ReferenceContext::Type;
    }

    // Check for tag reference nodes if they exist (depending on grammar)
    // Actually `throw` instruction params are handled inside instr_plain logic above.

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
            ReferenceContext::Tag => {
                // Check if tag exists
                symbols.get_tag_by_name(identifier_name).is_some()
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
        ReferenceContext::Tag => format!("Undefined tag '{}'", identifier_name),
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

// Lazy static initialization for instruction arity map
static INSTRUCTION_ARITY: OnceLock<
    std::collections::HashMap<
        &'static str,
        crate::diagnostics::instruction_metadata::InstructionArity,
    >,
> = OnceLock::new();

fn get_arity_map() -> &'static std::collections::HashMap<
    &'static str,
    crate::diagnostics::instruction_metadata::InstructionArity,
> {
    INSTRUCTION_ARITY.get_or_init(get_instruction_arity_map)
}

/// Recursively walk the tree looking for instructions with incorrect parameter counts
fn walk_tree_for_parameter_counts(
    node: Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Validate linear format instructions
    if node.kind() == "instr_plain" {
        check_instruction_parameter_count(&node, source, diagnostics);
        // Don't recurse - handled
        return;
    }

    // Validate folded format instructions (e.g., (i32.add expr expr))
    if node.kind() == "expr1_plain" {
        check_folded_instruction_parameter_count(&node, source, symbols, diagnostics);
        // Still recurse to check nested expressions
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_parameter_counts(child, source, symbols, diagnostics);
    }
}

/// Check if an instruction has the correct number of parameters
fn check_instruction_parameter_count(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.is_empty() {
        return;
    }

    let first_child = &children[0];
    let instr_kind = first_child.kind();

    match instr_kind {
        "op_index" | "op_index_opt" | "op_gc" | "op_exception" => {
            // Instructions like local.get, struct.new, throw etc.
            // op_gc and op_exception are new categories I added to grammar logic
            // Assuming grammar structure wraps them. If not, text lookup works.
            let instr_name = source[first_child.byte_range()].trim();
            // Count 'index' nodes or 'ref_type' nodes as parameters
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| c.kind() == "index" || c.kind() == "ref_type") // Some ref instructions take types
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        "op_const" => {
            // Constant instructions like i32.const, f64.const
            let mut op_const_cursor = first_child.walk();
            let op_const_children: Vec<_> = first_child.children(&mut op_const_cursor).collect();

            if op_const_children.is_empty() {
                return;
            }

            let instr_name = source[op_const_children[0].byte_range()].trim();
            // Count int/float children as parameters
            let param_count = op_const_children
                .iter()
                .skip(1)
                .filter(|c| matches!(c.kind(), "int" | "float"))
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        "op_nullary" => {
            // Instructions with no parameters
            let instr_name = source[first_child.byte_range()].trim();
            // These should have no parameters in linear format
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| matches!(c.kind(), "index" | "expr"))
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        // Fallback for new instruction types if not covered above but are plain instructions
        k if k.starts_with("op_") => {
            let instr_name = source[first_child.byte_range()].trim();
            let param_count = children
                .iter()
                .skip(1)
                .filter(|c| c.kind() == "index")
                .count();
            validate_instruction_arity(instr_name, param_count, node, diagnostics);
        }
        _ => {
            // Other instruction types
        }
    }
}

/// Check if a folded instruction (expr1_plain) has the correct number of operands
fn check_folded_instruction_parameter_count(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.is_empty() {
        return;
    }

    // First child should be instr_plain
    let first_child = &children[0];
    if first_child.kind() != "instr_plain" {
        return;
    }

    // Get the instruction name from instr_plain
    let mut instr_cursor = first_child.walk();
    let instr_children: Vec<_> = first_child.children(&mut instr_cursor).collect();
    if instr_children.is_empty() {
        return;
    }

    let instr_kind = instr_children[0].kind();
    let instr_name = match instr_kind {
        "op_const" => {
            let mut const_cursor = instr_children[0].walk();
            let const_children: Vec<_> = instr_children[0].children(&mut const_cursor).collect();
            if const_children.is_empty() {
                return;
            }
            source[const_children[0].byte_range()].trim()
        }
        _ => source[instr_children[0].byte_range()].trim(),
    };

    // Count expr children (these are the operands)
    let operand_count = children
        .iter()
        .skip(1)
        .filter(|c| c.kind() == "expr")
        .count();

    // Validate operand count
    let arity_map = get_arity_map();
    if let Some(arity) = arity_map.get(instr_name) {
        use crate::diagnostics::instruction_metadata::OperandMode;

        match arity.operand_mode {
            OperandMode::Fixed(expected) => {
                // Only report error if there are TOO MANY operands.
                // Having fewer operands is valid in WAT because remaining operands
                // can come from the implicit stack (linear or partially folded style).
                // For example: (br_if $loop) is valid when condition is on the stack.
                if operand_count > expected {
                    let diagnostic = create_operand_count_diagnostic(
                        node,
                        instr_name,
                        operand_count,
                        &format!("at most {}", arity.expected_operands_message()),
                    );
                    diagnostics.push(diagnostic);
                }
            }
            OperandMode::Dynamic => {
                // Perform dynamic validation based on instruction type
                validate_dynamic_operands(
                    node,
                    instr_name,
                    operand_count,
                    symbols,
                    source,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_dynamic_operands(
    node: &Node,
    instr_name: &str,
    operand_count: usize,
    symbols: &SymbolTable,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match instr_name {
        "struct.new" => {
            // Expects 1 param (type index) + N operands (where N = fields)
            // But instr_plain contains the type index.
            // We need to extract the type index from instr_plain children.
            // (expr1_plain (instr_plain struct.new $T) (expr...) (expr...))
            // Extract $T from instr_plain.
            if let Some(type_name) = extract_instruction_type_param(node, source) {
                if let Some(type_def) = symbols.get_type_by_name(&type_name) {
                    use crate::symbols::TypeKind;
                    if let TypeKind::Struct { fields } = &type_def.kind {
                        let expected = fields.len();
                        if operand_count != expected {
                            let msg =
                                format!("{} operands (fields of struct {})", expected, type_name);
                            let diagnostic = create_operand_count_diagnostic(
                                node,
                                instr_name,
                                operand_count,
                                &msg,
                            );
                            diagnostics.push(diagnostic);
                        }
                    }
                }
            }
        }
        "array.new_fixed" => {
            // (array.new_fixed $T length (arg)*)
            // length should match operand count
            // We need to parse length immediate.
            // This is complex because immediate might be part of instr_plain or separate.
            // Assuming simplified check: just verify type existence?
            // Or extract length immediate.
        }
        "throw" => {
            // (throw $tag (arg)*)
            if let Some(tag_name) = extract_instruction_type_param(node, source) {
                if let Some(tag) = symbols.get_tag_by_name(&tag_name) {
                    let expected = tag.params.len();
                    if operand_count != expected {
                        let msg = format!("{} operands (params of tag {})", expected, tag_name);
                        let diagnostic =
                            create_operand_count_diagnostic(node, instr_name, operand_count, &msg);
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }
        "call" => {
            // (call $func (arg)*)
            // The number of operands should match the function's parameter count
            if let Some(func_name) = extract_instruction_type_param(node, source) {
                if let Some(func) = symbols.get_function_by_name(&func_name) {
                    let expected = func.parameters.len();
                    if operand_count != expected {
                        let msg =
                            format!("{} operands (params of function {})", expected, func_name);
                        let diagnostic =
                            create_operand_count_diagnostic(node, instr_name, operand_count, &msg);
                        diagnostics.push(diagnostic);
                    }
                } else if let Ok(idx) = func_name.parse::<usize>() {
                    // Numeric function index
                    if let Some(func) = symbols.get_function_by_index(idx) {
                        let expected = func.parameters.len();
                        if operand_count != expected {
                            let func_display = func.name.as_deref().unwrap_or(&func_name);
                            let msg = format!(
                                "{} operands (params of function {})",
                                expected, func_display
                            );
                            let diagnostic = create_operand_count_diagnostic(
                                node,
                                instr_name,
                                operand_count,
                                &msg,
                            );
                            diagnostics.push(diagnostic);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn extract_instruction_type_param(expr_node: &Node, source: &str) -> Option<String> {
    // expr1_plain -> instr_plain -> (op_... $index)
    let mut cursor = expr_node.walk();
    for child in expr_node.children(&mut cursor) {
        if child.kind() == "instr_plain" {
            let mut instr_cursor = child.walk();
            for instr_child in child.children(&mut instr_cursor) {
                // Look for 'index' or identifier child
                let mut param_cursor = instr_child.walk();
                for param in instr_child.children(&mut param_cursor) {
                    if param.kind() == "index" || param.kind() == "identifier" {
                        return Some(source[param.byte_range()].trim().to_string());
                    }
                }
            }
        }
    }
    None
}

/// Validate instruction arity and create diagnostic if incorrect
fn validate_instruction_arity(
    instr_name: &str,
    param_count: usize,
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arity_map = get_arity_map();

    if let Some(arity) = arity_map.get(instr_name) {
        if !arity.is_valid(param_count) {
            let diagnostic = create_parameter_count_diagnostic(
                node,
                instr_name,
                param_count,
                &arity.expected_message(),
            );
            diagnostics.push(diagnostic);
        }
    }
}

/// Create a diagnostic for incorrect parameter count
fn create_parameter_count_diagnostic(
    node: &Node,
    instr_name: &str,
    actual_count: usize,
    expected_message: &str,
) -> Diagnostic {
    let range = node_to_range(node);

    let param_word = if actual_count == 1 {
        "parameter"
    } else {
        "parameters"
    };

    let message = format!(
        "Instruction '{}' expects {} parameter(s), but got {} {}",
        instr_name, expected_message, actual_count, param_word
    );

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

/// Create a diagnostic for incorrect operand count in folded expressions
fn create_operand_count_diagnostic(
    node: &Node,
    instr_name: &str,
    actual_count: usize,
    expected_message: &str,
) -> Diagnostic {
    let range = node_to_range(node);

    let message = format!(
        "Instruction '{}' expects {}, but got {}",
        instr_name, expected_message, actual_count
    );

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
  local.get $x
  local.get $y
  local.set $undefined)"#;

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
  nop)

(func $test (param $p i32) (local $l i32)
  (block $label
    local.get $p
    local.set $l
    global.get $g
    call $helper
    br $label))"#;

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

    // Parameter count validation tests

    #[test]
    fn test_local_set_correct_param_count() {
        let document = r#"(func $test (local $x i32)
  local.set $x)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        // Should have no parameter count errors (local.set expects 1 param and has 1)
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid local.set with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_local_set_missing_param() {
        // Note: Completely missing parameters like (local.set) create syntax errors
        // This test is skipped as tree-sitter will flag it as an ERROR node
        // Our parameter validation only runs on syntactically valid AST nodes
    }

    #[test]
    fn test_i32_const_correct() {
        let document = r#"(func $test
  (i32.const 42))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid i32.const with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_i32_const_missing_value() {
        // Note: Completely missing parameters like (i32.const) create syntax errors
        // This test is skipped as tree-sitter will flag it as an ERROR node
        // Our parameter validation only runs on syntactically valid AST nodes
    }

    #[test]
    fn test_drop_linear_format() {
        // Linear format: drop consumes from stack (no explicit operands)
        let document = r#"(func $test
  drop)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid drop in linear format should have no diagnostics"
        );
    }

    #[test]
    fn test_i32_add_linear_format() {
        // Linear format: i32.add consumes from stack (no explicit operands)
        let document = r#"(func $test
  i32.add)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid i32.add in linear format should have no diagnostics"
        );
    }

    #[test]
    fn test_folded_expression_correct() {
        // Folded expressions with correct operand counts should NOT trigger errors
        let document = r#"(func $test
  (i32.add (i32.const 1) (i32.const 2)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Correct folded expressions should not produce errors"
        );
    }

    #[test]
    fn test_folded_expression_too_many_operands() {
        // i32.add with 3 operands instead of 2
        let document = r#"(func $test
  (i32.add (i32.const 1) (i32.const 2) (i32.const 3)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("i32.add") && d.message.contains("expects"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            1,
            "i32.add with 3 operands should produce one error"
        );
        assert!(operand_errors[0].message.contains("2 operands"));
        assert!(operand_errors[0].message.contains("got 3"));
    }

    #[test]
    fn test_folded_expression_partial_is_valid() {
        // i32.add with 1 inline operand - the other comes from the stack (mixed style)
        // This is valid WAT: (i32.const 5) (i32.add (i32.const 1)) means add 1 to stack top
        let document = r#"(func $test
  (i32.add (i32.const 1)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("i32.add") && d.message.contains("expects"))
            .collect();
        // Partial folding is valid - remaining operands come from the stack
        assert_eq!(
            operand_errors.len(),
            0,
            "i32.add with 1 operand is valid (other from stack)"
        );
    }

    #[test]
    fn test_folded_unary_op_correct() {
        // i32.eqz with 1 operand (correct)
        let document = r#"(func $test
  (i32.eqz (i32.const 42)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Correct unary operation should not produce errors"
        );
    }

    #[test]
    fn test_folded_unary_op_too_many() {
        // i32.eqz with 2 operands instead of 1
        let document = r#"(func $test
  (i32.eqz (i32.const 1) (i32.const 2)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let operand_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("i32.eqz") && d.message.contains("expects"))
            .collect();
        assert_eq!(
            operand_errors.len(),
            1,
            "i32.eqz with 2 operands should produce one error"
        );
        assert!(operand_errors[0].message.contains("1 operand"));
        assert!(operand_errors[0].message.contains("got 2"));
    }

    #[test]
    fn test_folded_nested_correct() {
        // Nested folded expressions with correct operand counts
        let document = r#"(func $test
  (i32.add
    (i32.mul (i32.const 2) (i32.const 3))
    (i32.const 4)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Correct nested folded expressions should not produce errors"
        );
    }

    #[test]
    fn test_global_get_correct() {
        let document = r#"(global $g i32 (i32.const 0))
(func $test
  (global.get $g))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid global.get with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_br_correct() {
        let document = r#"(func $test
  (block $myblock
    (br $myblock)))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid br with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_call_correct() {
        let document = r#"(func $helper (nop))
(func $test
  (call $helper))"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects"))
                .count(),
            0,
            "Valid call with 1 parameter should have no diagnostics"
        );
    }

    #[test]
    fn test_mixed_valid_invalid() {
        // Test a mix of valid instructions in linear format
        let document = r#"(func $test (local $x i32)
  local.set $x
  i32.const 42
  drop
  nop)"#;

        let mut parser = create_parser();
        let tree = parser.parse(document, None).unwrap();
        let symbols = parse_document(document).unwrap();

        let diagnostics = provide_semantic_diagnostics(&tree, document, &symbols);
        let param_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("expects"))
            .collect();
        assert_eq!(
            param_errors.len(),
            0,
            "All instructions have correct parameter counts, should have no errors"
        );
    }
}
