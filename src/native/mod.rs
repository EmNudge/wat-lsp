//! Native LSP server.
//!
//! This module contains code that's only needed for the native LSP server.

mod documents;
mod positions;
pub mod server;

pub use server::Backend;
