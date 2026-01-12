// Library exports for testing and WASM builds

// Core types (protocol-independent) - must be first as other modules depend on it
pub mod core;

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

// LSP feature modules (completion, definition, hover, references, signature, symbols)
pub mod features;

// Re-export feature modules at the top level for backward compatibility
pub use features::symbols;

#[cfg(any(feature = "native", feature = "wasm"))]
pub use features::hover;

#[cfg(feature = "native")]
pub use features::completion;

#[cfg(feature = "native")]
pub use features::definition;

#[cfg(feature = "native")]
pub use features::references;

#[cfg(feature = "native")]
pub use features::signature;

// Diagnostics (native only)
#[cfg(feature = "native")]
pub mod diagnostics;

// Platform-specific modules
#[cfg(feature = "native")]
pub mod native;

// WASM entry point
#[cfg(feature = "wasm")]
pub mod wasm;
