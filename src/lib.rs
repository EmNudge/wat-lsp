// Library exports for testing and WASM builds

// Core types (protocol-independent) - must be first as other modules depend on it
pub mod core;

// Symbol table (shared between native and WASM)
pub mod symbols;

// Wast-based parser (works in WASM, always available)
pub mod wast_parser;

// Tree-sitter facade (unified interface for native and WASM)
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod ts_facade;

// Tree-sitter bindings (native only - used by ts_facade)
#[cfg(feature = "native")]
pub mod tree_sitter_bindings;

// Parser module (uses tree-sitter via ts_facade)
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod parser;

// Utilities (uses tree-sitter via ts_facade)
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod utils;

// Symbol lookup utilities (shared between native and WASM)
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod symbol_lookup;

// Hover support (uses tree-sitter via ts_facade)
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod hover;

// Native-only modules (depend on tower-lsp or other native deps)
#[cfg(feature = "native")]
pub mod completion;

#[cfg(feature = "native")]
pub mod definition;

#[cfg(feature = "native")]
pub mod diagnostics;

#[cfg(feature = "native")]
pub mod references;

#[cfg(feature = "native")]
pub mod signature;

#[cfg(feature = "native")]
pub mod native;

// WASM entry point
#[cfg(feature = "wasm")]
pub mod wasm;
