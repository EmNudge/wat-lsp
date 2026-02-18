//! Shared diagnostic logic for both native and WASM builds.
//!
//! This module provides platform-agnostic diagnostic functions that work with
//! both tree-sitter (native) and web-tree-sitter (WASM) through the ts_facade abstraction.

pub mod alignment_checks;
pub mod arity;
pub mod folded_checks;
pub mod gc_checks;
pub mod memory_checks;
pub mod module_checks;
pub mod references;
mod semantic;
pub mod simd_checks;
pub mod subtype;
mod termination;
pub mod tree_sitter;
pub mod tree_walk;
pub mod type_check;

pub use module_checks::{check_block_label_mismatch, validate_module_structure};
pub use semantic::track_stack_in_instr_list;
pub use subtype::validate_subtype_hierarchy;
pub(crate) use termination::sequence_always_terminates;
pub use tree_sitter::provide_tree_sitter_diagnostics;
