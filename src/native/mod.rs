//! Native LSP adapters and utilities.
//!
//! This module contains code that's only needed for the native LSP server,
//! including type conversions between core types and tower-lsp types.

pub mod adapters;

pub use adapters::*;
