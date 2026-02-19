//! Shared diagnostic logic for both native and WASM builds.
//!
//! This module provides platform-agnostic diagnostic functions that work with
//! both tree-sitter (native) and web-tree-sitter (WASM) through the ts_facade abstraction.

pub(crate) mod alignment_checks;
pub(crate) mod arity;
pub(crate) mod folded_checks;
pub(crate) mod gc_checks;
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
