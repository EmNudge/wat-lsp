//! Shared diagnostic logic for both native and WASM builds.
//!
//! This module provides platform-agnostic diagnostic functions that work with
//! both tree-sitter (native) and web-tree-sitter (WASM) through the ts_facade abstraction.

pub mod arity;
pub mod folded_checks;
pub mod memory_checks;
pub mod module_checks;
pub mod references;
mod semantic;
mod stack;
pub mod subtype;
mod termination;
pub mod tree_sitter;

pub use module_checks::{check_block_label_mismatch, validate_module_structure};
pub use references::*;
pub use semantic::*;
pub use stack::*;
pub use subtype::validate_subtype_hierarchy;
pub use termination::*;
pub use tree_sitter::provide_tree_sitter_diagnostics;
