// Library exports for testing and WASM builds

// Core types (protocol-independent) - must be first as other modules depend on it
pub mod core;

// Symbol table (shared between native and WASM)
pub mod symbols;

// Wast-based parser (works in WASM, always available)
pub mod wast_parser;

// Native-only modules (depend on tower-lsp)
#[cfg(feature = "native")]
pub mod parser;

#[cfg(feature = "native")]
pub mod tree_sitter_bindings;

#[cfg(feature = "native")]
pub mod completion;

#[cfg(feature = "native")]
pub mod definition;

#[cfg(feature = "native")]
pub mod diagnostics;

#[cfg(feature = "native")]
pub mod hover;

#[cfg(feature = "native")]
pub mod references;

#[cfg(feature = "native")]
pub mod signature;

#[cfg(feature = "native")]
pub mod utils;

#[cfg(feature = "native")]
pub mod native;

// WASM entry point
#[cfg(feature = "wasm")]
pub mod wasm;
