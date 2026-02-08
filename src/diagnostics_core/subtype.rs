//! Subtype hierarchy validation for GC types.
//!
//! Validates that `sub` declarations correctly reference parent types,
//! respects `final` constraints, and checks structural compatibility.

use crate::core::types::Diagnostic;
use crate::symbols::{SymbolTable, TypeDef, TypeKind};

/// Validate subtype hierarchy in the symbol table.
/// Returns diagnostics for invalid parent references, final type violations,
/// and structural incompatibilities.
pub fn validate_subtype_hierarchy(symbols: &SymbolTable) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for type_def in &symbols.types {
        let parent_ref = match &type_def.parent {
            Some(p) => p,
            None => continue,
        };

        let parent = resolve_parent(symbols, parent_ref);

        let parent = match parent {
            Some(p) => p,
            None => {
                if let Some(range) = type_def.range {
                    diagnostics.push(Diagnostic::error(
                        range,
                        format!("Undefined parent type '{}'", parent_ref),
                    ));
                }
                continue;
            }
        };

        // Check if parent is final
        if parent.is_final {
            let fallback = parent.index.to_string();
            let parent_name = parent.name.as_deref().unwrap_or(&fallback);
            if let Some(range) = type_def.range {
                diagnostics.push(Diagnostic::error(
                    range,
                    format!("Cannot extend final type '{}'", parent_name),
                ));
            }
            continue;
        }

        // Check structural compatibility
        check_structural_compatibility(type_def, parent, &mut diagnostics);
    }

    diagnostics
}

/// Resolve a parent reference (either "$name" or numeric index) to a TypeDef.
fn resolve_parent<'a>(symbols: &'a SymbolTable, parent_ref: &str) -> Option<&'a TypeDef> {
    if parent_ref.starts_with('$') {
        symbols.get_type_by_name(parent_ref)
    } else {
        parent_ref
            .parse::<usize>()
            .ok()
            .and_then(|idx| symbols.get_type_by_index(idx))
    }
}

/// Check structural compatibility between a child type and its declared parent.
fn check_structural_compatibility(
    child: &TypeDef,
    parent: &TypeDef,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let range = match child.range {
        Some(r) => r,
        None => return,
    };

    match (&child.kind, &parent.kind) {
        (
            TypeKind::Struct {
                fields: child_fields,
            },
            TypeKind::Struct {
                fields: parent_fields,
            },
        ) => {
            // Child must have at least as many fields as parent
            if child_fields.len() < parent_fields.len() {
                diagnostics.push(Diagnostic::error(
                    range,
                    format!(
                        "Struct subtype has {} fields but parent requires at least {}",
                        child_fields.len(),
                        parent_fields.len()
                    ),
                ));
                return;
            }
            // First N fields must match parent's types
            for (i, (_, parent_type, _parent_mut)) in parent_fields.iter().enumerate() {
                let (_, child_type, _child_mut) = &child_fields[i];
                if child_type != parent_type {
                    diagnostics.push(Diagnostic::error(
                        range,
                        format!(
                            "Struct field {} type mismatch: expected {:?}, got {:?}",
                            i, parent_type, child_type
                        ),
                    ));
                }
            }
        }
        (
            TypeKind::Array {
                element_type: child_elem,
                ..
            },
            TypeKind::Array {
                element_type: parent_elem,
                ..
            },
        ) => {
            if child_elem != parent_elem {
                diagnostics.push(Diagnostic::error(
                    range,
                    format!(
                        "Array element type mismatch: expected {:?}, got {:?}",
                        parent_elem, child_elem
                    ),
                ));
            }
        }
        (
            TypeKind::Func {
                params: child_params,
                results: child_results,
            },
            TypeKind::Func {
                params: parent_params,
                results: parent_results,
            },
        ) => {
            if child_params != parent_params || child_results != parent_results {
                diagnostics.push(Diagnostic::error(
                    range,
                    "Function subtype signature must match parent exactly".to_string(),
                ));
            }
        }
        _ => {
            // Kind mismatch (e.g., struct extending array)
            let child_kind_name = type_kind_name(&child.kind);
            let parent_kind_name = type_kind_name(&parent.kind);
            diagnostics.push(Diagnostic::error(
                range,
                format!(
                    "Type kind mismatch: {} cannot extend {}",
                    child_kind_name, parent_kind_name
                ),
            ));
        }
    }
}

fn type_kind_name(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Func { .. } => "func",
        TypeKind::Struct { .. } => "struct",
        TypeKind::Array { .. } => "array",
    }
}
