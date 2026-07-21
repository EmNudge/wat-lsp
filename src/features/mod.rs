// LSP feature modules
// Each module implements a specific Language Server Protocol capability

// Shared test utilities
#[cfg(all(test, feature = "native"))]
pub mod test_utils;

// Completion - provides code completion suggestions
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod completion;

// Definition - shared core logic for go-to-definition
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod definition_core;

// Definition - native LSP wrapper
#[cfg(feature = "native")]
pub mod definition;

// Document symbols - shared core logic for symbol outline
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod document_symbols_core;

// Document symbols - native LSP wrapper
#[cfg(feature = "native")]
pub mod document_symbols;

// Folding - code folding ranges
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod folding;

// Hover - provides hover documentation
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod hover;

// References - shared core logic for find-all-references
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod references_core;

// References - native LSP wrapper
#[cfg(feature = "native")]
pub mod references;

// Semantic tokens - server-driven syntax highlighting classification
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod semantic_tokens;

// Signature - signature help for function calls
// The call_info submodule is shared; the LSP wrapper is native-only
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod signature;

// Symbols - document symbol extraction
pub mod symbols;
