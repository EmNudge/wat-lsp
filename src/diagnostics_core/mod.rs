//! Shared diagnostic logic for both native and WASM builds.
//!
//! This module provides platform-agnostic diagnostic functions that work with
//! both tree-sitter (native) and web-tree-sitter (WASM) through the ts_facade abstraction.

pub(crate) mod alignment_checks;
pub(crate) mod arity;
pub(crate) mod folded_checks;
pub(crate) mod gc_checks;
pub(crate) mod local_init;
pub(crate) mod memory_checks;
pub(crate) mod module_checks;
pub(crate) mod references;
mod semantic;
pub(crate) mod simd_checks;
pub(crate) mod subtype;
mod termination;
pub(crate) mod tree_sitter;
pub(crate) mod tree_walk;
mod type_check;

pub(crate) use semantic::track_stack_in_instr_list;
pub(crate) use termination::sequence_always_terminates;
// Re-export used by wasm/api.rs (appears unused under native-only compilation)
#[cfg(all(feature = "wasm", not(feature = "native")))]
pub(crate) use tree_sitter::provide_tree_sitter_diagnostics;

use crate::core::types::Diagnostic;
use crate::symbols::SymbolTable;

/// Remove exact-duplicate diagnostics in place, preserving first-seen order.
///
/// The independent syntax and semantic passes (and, on native, the wast
/// validator) can each surface the same problem at the same location. Two
/// diagnostics are considered duplicates when their range, severity, code, and
/// message all match.
///
/// The WASM `provideDiagnostics` path calls this directly. The native LSP path
/// deduplicates equivalently in [`crate::diagnostics::merge_all_diagnostics`]
/// (which operates on the `tower_lsp` diagnostic type after conversion), so this
/// core helper is only wired into the WASM build; it is still unit-tested on
/// native.
/// A comparable, hashable identity for a core diagnostic: range, severity, code,
/// and message. Two diagnostics with the same key are treated as exact duplicates.
#[cfg(any(feature = "wasm", test))]
type DiagnosticKey = ((u32, u32, u32, u32), u8, Option<&'static str>, String);

#[cfg(any(feature = "wasm", test))]
pub(crate) fn dedup_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    use std::collections::HashSet;

    let mut seen: HashSet<DiagnosticKey> = HashSet::new();

    diagnostics.retain(|d| {
        let key: DiagnosticKey = (
            (
                d.range.start.line,
                d.range.start.character,
                d.range.end.line,
                d.range.end.character,
            ),
            d.severity as u8,
            d.code,
            d.message.clone(),
        );
        seen.insert(key)
    });
}

/// Collect all semantic diagnostics (tree walk + subtype + module structure).
/// Shared between native and WASM diagnostic pipelines.
#[cfg(feature = "native")]
pub(crate) fn collect_all_semantic_diagnostics(
    root: ::tree_sitter::Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let config = tree_walk::DiagnosticConfig::from_symbols(symbols);
    let mut diagnostics = Vec::new();
    tree_walk::walk_tree_for_diagnostics(root, source, symbols, &config, &mut diagnostics);
    diagnostics.extend(subtype::validate_subtype_hierarchy(symbols));
    diagnostics.extend(module_checks::validate_module_structure(
        &root, source, symbols,
    ));
    diagnostics
}

#[cfg(all(feature = "wasm", not(feature = "native")))]
pub(crate) fn collect_all_semantic_diagnostics(
    root: crate::ts_facade::Node,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let config = tree_walk::DiagnosticConfig::from_symbols(symbols);
    let mut diagnostics = Vec::new();
    // module_checks borrows root; walk_tree moves it (WASM Node is not Copy)
    diagnostics.extend(module_checks::validate_module_structure(
        &root, source, symbols,
    ));
    tree_walk::walk_tree_for_diagnostics(root, source, symbols, &config, &mut diagnostics);
    diagnostics.extend(subtype::validate_subtype_hierarchy(symbols));
    diagnostics
}

#[cfg(test)]
mod dedup_tests {
    use super::dedup_diagnostics;
    use crate::core::types::{Diagnostic, Position, Range};

    fn range(l: u32, c: u32) -> Range {
        Range {
            start: Position {
                line: l,
                character: c,
            },
            end: Position {
                line: l,
                character: c + 1,
            },
        }
    }

    #[test]
    fn removes_exact_duplicates() {
        let mut diags = vec![
            Diagnostic::error(range(1, 0), "type mismatch").with_code("type-mismatch"),
            Diagnostic::error(range(1, 0), "type mismatch").with_code("type-mismatch"),
        ];
        dedup_diagnostics(&mut diags);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn keeps_distinct_message_or_range_or_code() {
        let mut diags = vec![
            // Same range + message, different code -> kept.
            Diagnostic::error(range(1, 0), "type mismatch").with_code("type-mismatch"),
            Diagnostic::error(range(1, 0), "type mismatch").with_code("arity"),
            // Same code + message, different range -> kept.
            Diagnostic::error(range(2, 0), "type mismatch").with_code("type-mismatch"),
            // Same range + code, different message -> kept.
            Diagnostic::error(range(1, 0), "other message").with_code("type-mismatch"),
        ];
        dedup_diagnostics(&mut diags);
        assert_eq!(diags.len(), 4);
    }

    #[test]
    fn preserves_first_seen_order() {
        let mut diags = vec![
            Diagnostic::error(range(3, 0), "c"),
            Diagnostic::error(range(1, 0), "a"),
            Diagnostic::error(range(3, 0), "c"), // dup of first
            Diagnostic::error(range(2, 0), "b"),
        ];
        dedup_diagnostics(&mut diags);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(msgs, ["c", "a", "b"]);
    }
}
