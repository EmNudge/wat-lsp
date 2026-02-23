//! Subtype hierarchy validation for GC types.
//!
//! Validates that `sub` declarations correctly reference parent types,
//! respects `final` constraints, and checks structural compatibility.

use crate::core::types::Diagnostic;
use crate::symbols::{SymbolTable, TypeDef, TypeKind};

use super::type_check::types_compatible_with_symbols;

/// Validate subtype hierarchy in the symbol table.
/// Returns diagnostics for invalid parent references, final type violations,
/// and structural incompatibilities.
pub(crate) fn validate_subtype_hierarchy(symbols: &SymbolTable) -> Vec<Diagnostic> {
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
        check_structural_compatibility(type_def, parent, symbols, &mut diagnostics);
    }

    diagnostics
}

/// Resolve a parent reference (either "$name" or numeric index) to a TypeDef.
fn resolve_parent<'a>(symbols: &'a SymbolTable, parent_ref: &str) -> Option<&'a TypeDef> {
    symbols.resolve_type(parent_ref)
}

/// Check structural compatibility between a child type and its declared parent.
fn check_structural_compatibility(
    child: &TypeDef,
    parent: &TypeDef,
    symbols: &SymbolTable,
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
            // First N fields must match parent's types with proper variance
            for (i, (_, parent_type, parent_mut)) in parent_fields.iter().enumerate() {
                let (_, child_type, child_mut) = &child_fields[i];
                if *parent_mut {
                    // Mutable fields: invariant (must be exact match both ways)
                    if child_type != parent_type {
                        diagnostics.push(Diagnostic::error(
                            range,
                            format!(
                                "Struct field {} type mismatch: expected {:?}, got {:?}",
                                i, parent_type, child_type
                            ),
                        ));
                    }
                    // Mutable fields must both be mutable
                    if child_mut != parent_mut {
                        diagnostics.push(Diagnostic::error(
                            range,
                            format!("Struct field {} mutability mismatch", i),
                        ));
                    }
                } else {
                    // Immutable fields: covariant (child must be subtype of parent)
                    if *child_mut {
                        diagnostics.push(Diagnostic::error(
                            range,
                            format!("Struct field {} mutability mismatch", i),
                        ));
                    }
                    if !types_compatible_with_symbols(child_type, parent_type, symbols) {
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
        }
        (
            TypeKind::Array {
                element_type: child_elem,
                mutable: child_mut,
            },
            TypeKind::Array {
                element_type: parent_elem,
                mutable: parent_mut,
            },
        ) => {
            if *parent_mut {
                // Mutable array: invariant
                if child_elem != parent_elem {
                    diagnostics.push(Diagnostic::error(
                        range,
                        format!(
                            "Array element type mismatch: expected {:?}, got {:?}",
                            parent_elem, child_elem
                        ),
                    ));
                }
                if child_mut != parent_mut {
                    diagnostics.push(Diagnostic::error(range, "Array mutability mismatch"));
                }
            } else {
                // Immutable array: covariant
                if *child_mut {
                    diagnostics.push(Diagnostic::error(range, "Array mutability mismatch"));
                }
                if !types_compatible_with_symbols(child_elem, parent_elem, symbols) {
                    diagnostics.push(Diagnostic::error(
                        range,
                        format!(
                            "Array element type mismatch: expected {:?}, got {:?}",
                            parent_elem, child_elem
                        ),
                    ));
                }
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
            // Function subtyping: params are contravariant, results are covariant
            let params_ok = child_params.len() == parent_params.len()
                && child_params.iter().zip(parent_params.iter()).all(|(c, p)| {
                    // Contravariant: parent param must be subtype of child param
                    types_compatible_with_symbols(p, c, symbols)
                });
            let results_ok = child_results.len() == parent_results.len()
                && child_results
                    .iter()
                    .zip(parent_results.iter())
                    .all(|(c, p)| {
                        // Covariant: child result must be subtype of parent result
                        types_compatible_with_symbols(c, p, symbols)
                    });
            if !params_ok || !results_ok {
                diagnostics.push(Diagnostic::error(
                    range,
                    "Function subtype signature must match parent exactly",
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
