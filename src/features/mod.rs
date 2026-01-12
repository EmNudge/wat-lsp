// LSP feature modules
// Each module implements a specific Language Server Protocol capability

// Completion - provides code completion suggestions
#[cfg(feature = "native")]
pub mod completion;

// Definition - implements go-to-definition
#[cfg(feature = "native")]
pub mod definition;

// Hover - provides hover documentation
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod hover;

// References - find all references to a symbol
#[cfg(feature = "native")]
pub mod references;

// Signature - signature help for function calls
#[cfg(feature = "native")]
pub mod signature;

// Symbols - document symbol extraction
pub mod symbols;
