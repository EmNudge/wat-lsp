// Library exports for testing and WASM builds

// ============================================================================
// Macros for native/WASM platform abstraction (must be before module declarations)
// ============================================================================

/// Bind `node.kind()` to a `&str` variable, handling the native (`&str`) vs
/// WASM (`String` → `.as_str()`) difference in one line.
///
/// Usage: `node_kind!(ck = child);` expands to the equivalent of:
/// ```ignore
/// let ck = child.kind();
/// #[cfg(all(feature = "wasm", not(feature = "native")))]
/// let ck = ck.as_str();
/// ```
macro_rules! node_kind {
    ($name:ident = $node:expr) => {
        let $name = $node.kind();
        #[cfg(all(feature = "wasm", not(feature = "native")))]
        let $name = $name.as_str();
    };
}

// Core types (protocol-independent) - must be first as other modules depend on it
pub mod core;

// Instruction metadata (shared between native and WASM for stack tracking)
pub mod instruction_metadata;

// Documentation access (instruction docs generated at build time)
pub mod docs;

// Wast-based parser (native-only: superseded by tree-sitter for WASM builds)
#[cfg(feature = "native")]
pub mod wast_parser;

// Tree-sitter facade (unified interface for native and WASM)
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod ts_facade;

// Shared diagnostics core (unified diagnostic logic for native and WASM)
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod diagnostics_core;

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

#[cfg(any(feature = "native", feature = "wasm"))]
pub use features::completion;

#[cfg(feature = "native")]
pub use features::definition;

#[cfg(feature = "native")]
pub use features::document_symbols;

#[cfg(feature = "native")]
pub use features::references;

#[cfg(any(feature = "native", feature = "wasm"))]
pub use features::signature;

#[cfg(any(feature = "native", feature = "wasm"))]
pub use features::folding;

// Diagnostics (native only)
#[cfg(feature = "native")]
pub mod diagnostics;

// Platform-specific modules
#[cfg(feature = "native")]
pub mod native;

// WASM entry point (only when native is not enabled, as they have incompatible tree-sitter APIs)
#[cfg(all(feature = "wasm", not(feature = "native")))]
pub mod wasm;

// Shared test utilities (available for benchmarks and integration tests, not compiled into WASM)
#[cfg(any(test, feature = "native"))]
pub mod test_utils;
