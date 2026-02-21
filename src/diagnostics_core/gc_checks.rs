//! GC type validation checks for struct field access instructions.
//!
//! Validates `struct.get`, `struct.get_s`, `struct.get_u`, and `struct.set`
//! instructions have valid field indices and appropriate packed type usage.

use crate::core::types::Diagnostic;
use crate::symbols::{SymbolTable, TypeKind, ValueType};
use crate::utils::node_to_range;

#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

// TODO: ref.cast/ref.test/br_on_cast heap type compatibility checks are deferred.
// The type checker currently uses `Unknown` for all GC stack values, so we cannot
// validate that cast source/target are in the same hierarchy at the stack level.
// The concrete ref subtyping in type_check.rs prepares the infrastructure for when
// typed GC refs flow through the stack.

/// Check struct field access instructions for valid field indices and packed type usage.
///
/// Runs on `instr_plain` nodes containing `struct.get/get_s/get_u/set`.
/// Returns diagnostics for:
/// - Field index out of bounds
/// - Named field not found in struct
/// - `struct.get_s`/`struct.get_u` on non-packed field type
/// - Type is not a struct (e.g., array used with struct.get)
pub fn check_struct_field_access(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let text = &source[node.byte_range()];
    let first_token = text.split_whitespace().next().unwrap_or("");

    if !matches!(
        first_token,
        "struct.get" | "struct.get_s" | "struct.get_u" | "struct.set"
    ) {
        return Vec::new();
    }

    let (type_ref, field_ref) = match extract_two_indices(node, source) {
        Some(pair) => pair,
        None => return Vec::new(),
    };

    // Resolve type
    let type_def = match resolve_type_def(&type_ref, symbols) {
        Some(td) => td,
        None => return Vec::new(), // Undefined type caught by reference checker
    };

    let fields = match &type_def.kind {
        TypeKind::Struct { fields } => fields,
        _ => {
            return vec![Diagnostic::error(
                node_to_range(node),
                format!(
                    "struct.get/set used on non-struct type {}",
                    type_ref_display(&type_ref, type_def.index)
                ),
            )];
        }
    };

    let mut diagnostics = Vec::new();

    // Resolve field index
    let field_idx = match resolve_field_index(&field_ref, fields) {
        Some(idx) => idx,
        None => {
            if field_ref.starts_with('$') {
                diagnostics.push(Diagnostic::error(
                    node_to_range(node),
                    format!("unknown field {}", field_ref),
                ));
            } else if let Ok(idx) = field_ref.parse::<usize>() {
                diagnostics.push(Diagnostic::error(
                    node_to_range(node),
                    format!("unknown field {}", idx),
                ));
            }
            return diagnostics;
        }
    };

    // Check packed type for struct.get_s / struct.get_u
    if first_token == "struct.get_s" || first_token == "struct.get_u" {
        let (_, field_type, _) = &fields[field_idx];
        if !is_packed_type(field_type) {
            diagnostics.push(Diagnostic::error(
                node_to_range(node),
                format!(
                    "{} requires a packed storage type (i8 or i16), but field {} has type {}",
                    first_token, field_idx, field_type
                ),
            ));
        }
    }

    // Check mutability for struct.set
    if first_token == "struct.set" {
        let (_, _, mutable) = &fields[field_idx];
        if !mutable {
            diagnostics.push(Diagnostic::error(
                node_to_range(node),
                "immutable field".to_string(),
            ));
        }
    }

    diagnostics
}

/// Extract two index children from a struct instruction node.
///
/// struct.get/set instructions have the form: `struct.get $type $field`
/// where both are `index` nodes in the grammar.
fn extract_two_indices(node: &Node, source: &str) -> Option<(String, String)> {
    let mut indices = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        node_kind!(kind_ref = child);

        if kind_ref == "index" {
            // Index node may contain identifier or nat child
            let index_text = extract_index_value(&child, source);
            indices.push(index_text);
            if indices.len() == 2 {
                break;
            }
        }
    }

    if indices.len() == 2 {
        Some((indices.remove(0), indices.remove(0)))
    } else {
        None
    }
}

/// Extract the value from an index node — either a `$name` identifier or numeric `nat`.
fn extract_index_value(node: &Node, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind_ref = child);

        if kind_ref == "identifier" || kind_ref == "nat" {
            return source[child.byte_range()].trim().to_string();
        }
    }
    // Fallback: use the node text directly
    source[node.byte_range()].trim().to_string()
}

/// Resolve a type reference (either `$name` or numeric index) to a TypeDef.
fn resolve_type_def<'a>(
    type_ref: &str,
    symbols: &'a SymbolTable,
) -> Option<&'a crate::symbols::TypeDef> {
    if type_ref.starts_with('$') {
        symbols.get_type_by_name(type_ref)
    } else {
        type_ref
            .parse::<usize>()
            .ok()
            .and_then(|idx| symbols.get_type_by_index(idx))
    }
}

/// Resolve a field reference to an index within the struct fields.
///
/// Handles both named (`$name`) and numeric references.
fn resolve_field_index(
    field_ref: &str,
    fields: &[(Option<String>, ValueType, bool)],
) -> Option<usize> {
    if field_ref.starts_with('$') {
        // Named field lookup
        fields
            .iter()
            .position(|(name, _, _)| name.as_deref() == Some(field_ref))
    } else {
        // Numeric index
        let idx = field_ref.parse::<usize>().ok()?;
        if idx < fields.len() {
            Some(idx)
        } else {
            None
        }
    }
}

/// Check if a ValueType is a packed storage type (i8 or i16).
fn is_packed_type(ty: &ValueType) -> bool {
    matches!(ty, ValueType::I8 | ValueType::I16)
}

/// Display helper for type references.
fn type_ref_display(type_ref: &str, index: usize) -> String {
    if type_ref.starts_with('$') {
        type_ref.to_string()
    } else {
        index.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::ValueType;

    #[test]
    fn test_resolve_field_index_numeric() {
        let fields = vec![
            (Some("$x".to_string()), ValueType::I32, false),
            (Some("$y".to_string()), ValueType::I64, true),
        ];
        assert_eq!(resolve_field_index("0", &fields), Some(0));
        assert_eq!(resolve_field_index("1", &fields), Some(1));
        assert_eq!(resolve_field_index("2", &fields), None);
    }

    #[test]
    fn test_resolve_field_index_named() {
        let fields = vec![
            (Some("$x".to_string()), ValueType::I32, false),
            (Some("$y".to_string()), ValueType::I64, true),
        ];
        assert_eq!(resolve_field_index("$x", &fields), Some(0));
        assert_eq!(resolve_field_index("$y", &fields), Some(1));
        assert_eq!(resolve_field_index("$z", &fields), None);
    }

    #[test]
    fn test_resolve_field_index_unnamed() {
        let fields = vec![(None, ValueType::I32, false), (None, ValueType::F64, true)];
        assert_eq!(resolve_field_index("0", &fields), Some(0));
        assert_eq!(resolve_field_index("$x", &fields), None);
    }

    #[test]
    fn test_is_packed_type() {
        assert!(is_packed_type(&ValueType::I8));
        assert!(is_packed_type(&ValueType::I16));
        assert!(!is_packed_type(&ValueType::I32));
        assert!(!is_packed_type(&ValueType::I64));
        assert!(!is_packed_type(&ValueType::F32));
        assert!(!is_packed_type(&ValueType::Funcref));
    }

    #[cfg(feature = "native")]
    mod integration {
        use crate::parser::parse_document;
        use crate::tree_sitter_bindings::create_parser;

        fn get_diagnostics(source: &str) -> Vec<String> {
            let mut parser = create_parser();
            let tree = parser.parse(source, None).unwrap();
            let symbols = parse_document(source).expect("Failed to extract symbols");
            let diags = crate::diagnostics::provide_semantic_diagnostics(&tree, source, &symbols);
            diags.iter().map(|d| d.message.clone()).collect()
        }

        #[test]
        fn test_struct_field_out_of_bounds() {
            let source = r#"(module
                (type $point (struct (field $x i32) (field $y i32)))
                (func (param (ref $point))
                    local.get 0
                    struct.get $point 5
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                diags.iter().any(|d| d.contains("unknown field")),
                "Expected 'unknown field' diagnostic, got: {:?}",
                diags
            );
        }

        #[test]
        fn test_struct_get_s_non_packed() {
            let source = r#"(module
                (type $s (struct (field $f i32)))
                (func (param (ref $s))
                    local.get 0
                    struct.get_s $s 0
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                diags.iter().any(|d| d.contains("packed storage type")),
                "Expected 'packed storage type' diagnostic, got: {:?}",
                diags
            );
        }

        #[test]
        fn test_struct_get_s_packed_valid() {
            let source = r#"(module
                (type $s (struct (field $f i8)))
                (func (param (ref $s)) (result i32)
                    local.get 0
                    struct.get_s $s 0
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                !diags.iter().any(|d| d.contains("packed storage type")),
                "Expected no 'packed storage type' diagnostic, got: {:?}",
                diags
            );
        }

        #[test]
        fn test_struct_get_u_non_packed() {
            let source = r#"(module
                (type $s (struct (field $f f64)))
                (func (param (ref $s))
                    local.get 0
                    struct.get_u $s 0
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                diags.iter().any(|d| d.contains("packed storage type")),
                "Expected 'packed storage type' diagnostic, got: {:?}",
                diags
            );
        }

        #[test]
        fn test_valid_struct_field_access() {
            let source = r#"(module
                (type $point (struct (field $x i32) (field $y i32)))
                (func (param (ref $point)) (result i32)
                    local.get 0
                    struct.get $point 0
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                !diags.iter().any(|d| d.contains("unknown field")),
                "Expected no 'unknown field' diagnostic, got: {:?}",
                diags
            );
        }

        #[test]
        fn test_struct_get_on_array_type() {
            let source = r#"(module
                (type $arr (array i32))
                (func (param (ref $arr))
                    local.get 0
                    struct.get $arr 0
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                diags.iter().any(|d| d.contains("non-struct type")),
                "Expected 'non-struct type' diagnostic, got: {:?}",
                diags
            );
        }

        #[test]
        fn test_struct_field_named_not_found() {
            let source = r#"(module
                (type $point (struct (field $x i32) (field $y i32)))
                (func (param (ref $point))
                    local.get 0
                    struct.get $point $z
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                diags.iter().any(|d| d.contains("unknown field")),
                "Expected 'unknown field' diagnostic, got: {:?}",
                diags
            );
        }

        #[test]
        fn test_struct_set_field_out_of_bounds() {
            let source = r#"(module
                (type $s (struct (field $x (mut i32))))
                (func (param (ref $s))
                    local.get 0
                    i32.const 42
                    struct.set $s 5
                )
            )"#;
            let diags = get_diagnostics(source);
            assert!(
                diags.iter().any(|d| d.contains("unknown field")),
                "Expected 'unknown field' diagnostic, got: {:?}",
                diags
            );
        }
    }
}
