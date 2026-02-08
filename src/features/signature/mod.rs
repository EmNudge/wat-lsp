// Shared call-info extraction (available for both native and WASM)
pub mod call_info;

#[cfg(feature = "native")]
use crate::symbols::*;
#[cfg(feature = "native")]
use crate::utils::{
    format_function_signature, get_line_at_position, node_at_position, utf16_offset_to_byte_offset,
};
#[cfg(feature = "native")]
use tower_lsp::lsp_types::*;
#[cfg(feature = "native")]
use tree_sitter::Tree;

#[cfg(feature = "native")]
use call_info::{find_function_call, find_function_call_ast, CallType};

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
        CallType::Direct => provide_direct_call_signature(symbols, &call_info),
        CallType::CallRef | CallType::ReturnCallRef => {
            provide_call_ref_signature(symbols, &call_info)
        }
    }
}

/// Provide signature help for direct function calls (call $func)
#[cfg(feature = "native")]
fn provide_direct_call_signature(
    symbols: &SymbolTable,
    call_info: &call_info::CallInfo,
) -> Option<SignatureHelp> {
    // Look up the function in the symbol table
    let func = if call_info.name.starts_with('$') {
        symbols.get_function_by_name(&call_info.name)?
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_function_by_index(index)?
    } else {
        return None;
    };

    // Build signature information
    let label = format_function_signature(func);

    let mut parameters = Vec::new();
    for param in &func.parameters {
        let param_label = if let Some(ref name) = param.name {
            format!("({} {})", name, param.param_type)
        } else {
            format!("(param {})", param.param_type)
        };
        parameters.push(ParameterInformation {
            label: ParameterLabel::Simple(param_label),
            documentation: None,
        });
    }

    // Determine which parameter we're currently on based on comma count
    let active_parameter = call_info.arg_text.matches(',').count() as u32;

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(parameters),
            active_parameter: Some(active_parameter.min(func.parameters.len() as u32)),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_parameter.min(func.parameters.len() as u32)),
    })
}

/// Provide signature help for indirect calls via typed function references (call_ref $type)
#[cfg(feature = "native")]
fn provide_call_ref_signature(
    symbols: &SymbolTable,
    call_info: &call_info::CallInfo,
) -> Option<SignatureHelp> {
    // Look up the type in the symbol table
    let type_def = if call_info.name.starts_with('$') {
        symbols.get_type_by_name(&call_info.name)?
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_type_by_index(index)?
    } else {
        return None;
    };

    // The type must be a function type
    let (params, results) = match &type_def.kind {
        TypeKind::Func { params, results } => (params, results),
        _ => return None, // Not a function type
    };

    // Build signature label
    let call_kind = match call_info.call_type {
        CallType::CallRef => "call_ref",
        CallType::ReturnCallRef => "return_call_ref",
        _ => "call_ref",
    };
    let type_index_str = type_def.index.to_string();
    let type_name = type_def.name.as_deref().unwrap_or(&type_index_str);
    let params_str = params
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let results_str = results
        .iter()
        .map(|r| r.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let label = format!(
        "({} {}) (param {}) (result {}) + funcref",
        call_kind,
        type_name,
        if params_str.is_empty() {
            "none"
        } else {
            &params_str
        },
        if results_str.is_empty() {
            "none"
        } else {
            &results_str
        }
    );

    // Build parameter information
    let mut parameters = Vec::new();
    for (i, param_type) in params.iter().enumerate() {
        parameters.push(ParameterInformation {
            label: ParameterLabel::Simple(format!("(param{} {})", i, param_type)),
            documentation: None,
        });
    }
    // The last argument is always the funcref
    parameters.push(ParameterInformation {
        label: ParameterLabel::Simple("(funcref)".to_string()),
        documentation: Some(Documentation::String(
            "Function reference to call".to_string(),
        )),
    });

    // Determine which parameter we're currently on
    let active_parameter = call_info.arg_text.matches(',').count() as u32;
    let param_count = parameters.len() as u32;

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: Some(Documentation::String(format!(
                "Indirect call via typed function reference. The last argument must be a function reference of type {}.",
                type_name
            ))),
            parameters: Some(parameters),
            active_parameter: Some(active_parameter.min(param_count)),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_parameter.min(params.len() as u32 + 1)),
    })
}
