//! Shared diagnostic logic for both native and WASM builds.
//!
//! This module provides platform-agnostic diagnostic functions that work with
//! both tree-sitter (native) and web-tree-sitter (WASM) through the ts_facade abstraction.

mod references;
mod semantic;
mod stack;
mod termination;
pub mod tree_sitter;

pub use references::*;
pub use semantic::*;
pub use stack::*;
pub use termination::*;
pub use tree_sitter::provide_tree_sitter_diagnostics;
