// Shared call-info extraction (available for both native and WASM)
pub mod call_info;

// Shared signature building logic (available for both native and WASM)
pub mod signature_core;

#[cfg(feature = "native")]
use crate::symbols::*;
#[cfg(feature = "native")]
use crate::utils::{get_line_at_position, node_at_position, utf16_offset_to_byte_offset};
#[cfg(feature = "native")]
use tower_lsp::lsp_types::*;
#[cfg(feature = "native")]
use tree_sitter::Tree;

#[cfg(feature = "native")]
use call_info::{find_function_call, find_function_call_ast, CallType};
#[cfg(feature = "native")]
use signature_core::{provide_call_ref_signature_core, provide_direct_call_signature_core};

#[cfg(all(test, feature = "native"))]
mod tests;

#[cfg(feature = "native")]
pub fn provide_signature_help(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
) -> Option<SignatureHelp> {
    // Try AST-based approach first
    let call_info = if let Some(node) = node_at_position(tree, document, position.into()) {
        find_function_call_ast(&node, document)
    } else {
        None
    };

    // Fall back to string-based approach for incomplete code
    let call_info = call_info.or_else(|| {
        let line = get_line_at_position(document, position.line as usize)?;
        let line_prefix = &line[..utf16_offset_to_byte_offset(line, position.character)];
        find_function_call(line_prefix)
    })?;

    match call_info.call_type {
        CallType::Direct => {
            let info = provide_direct_call_signature_core(symbols, &call_info)?;
            Some(signature_info_to_lsp(info))
        }
        CallType::CallRef | CallType::ReturnCallRef => {
            let info = provide_call_ref_signature_core(symbols, &call_info)?;
            Some(signature_info_to_lsp(info))
        }
    }
}

/// Convert protocol-independent signature help into LSP SignatureHelp.
#[cfg(feature = "native")]
fn signature_info_to_lsp(info: signature_core::SignatureHelpInfo) -> SignatureHelp {
    let sig = info.signature;
    let parameters: Vec<ParameterInformation> = sig
        .parameters
        .into_iter()
        .map(|p| ParameterInformation {
            label: ParameterLabel::Simple(p.label),
            documentation: p.documentation.map(Documentation::String),
        })
        .collect();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: sig.label,
            documentation: sig.documentation.map(Documentation::String),
            parameters: Some(parameters),
            active_parameter: Some(sig.active_parameter),
        }],
        active_signature: Some(0),
        active_parameter: Some(sig.active_parameter),
    }
}
