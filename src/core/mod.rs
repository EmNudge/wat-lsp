//! Core types and utilities shared between native and WASM builds.
//!
//! This module contains protocol-independent types that can be used
//! without depending on tower-lsp or other LSP-specific crates.

pub mod types;

pub use types::*;
