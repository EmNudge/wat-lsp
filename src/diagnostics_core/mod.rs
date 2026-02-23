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
