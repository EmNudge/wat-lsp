//! Protocol-independent signature help logic shared between native and WASM.
//!
//! Both native (tower-lsp) and WASM (wasm-bindgen) builds convert
//! `SignatureHelpInfo` into their respective output types.

use crate::symbols::{SymbolTable, TypeKind};
use crate::utils::format_function_signature;

use super::call_info::{CallInfo, CallType};

/// Protocol-independent parameter information.
pub struct ParameterInfo {
    pub label: String,
    pub documentation: Option<String>,
}

/// Protocol-independent signature information.
pub struct SignatureInfo {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<ParameterInfo>,
    pub active_parameter: u32,
}

/// Protocol-independent signature help result.
pub struct SignatureHelpInfo {
    pub signature: SignatureInfo,
}

/// Build signature help for a direct function call (call $func).
pub fn provide_direct_call_signature_core(
    symbols: &SymbolTable,
    call_info: &CallInfo,
) -> Option<SignatureHelpInfo> {
    let func = if call_info.name.starts_with('$') {
        symbols.get_function_by_name(&call_info.name)?
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_function_by_index(index)?
    } else {
        return None;
    };

    let label = format_function_signature(func);

    let parameters: Vec<ParameterInfo> = func
        .parameters
        .iter()
        .map(|param| {
            let param_label = if let Some(ref name) = param.name {
                format!("({} {})", name, param.param_type)
            } else {
                format!("(param {})", param.param_type)
            };
            ParameterInfo {
                label: param_label,
                documentation: None,
            }
        })
        .collect();

    let active_parameter = call_info.arg_text.matches(',').count() as u32;
    let clamped = active_parameter.min(func.parameters.len() as u32);

    Some(SignatureHelpInfo {
        signature: SignatureInfo {
            label,
            documentation: func.doc_comment.clone(),
            parameters,
            active_parameter: clamped,
        },
    })
}

/// Build signature help for an indirect call via typed function reference (call_ref $type).
pub fn provide_call_ref_signature_core(
    symbols: &SymbolTable,
    call_info: &CallInfo,
) -> Option<SignatureHelpInfo> {
    let type_def = if call_info.name.starts_with('$') {
        symbols.get_type_by_name(&call_info.name)?
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_type_by_index(index)?
    } else {
        return None;
    };

    let (params, results) = match &type_def.kind {
        TypeKind::Func { params, results } => (params, results),
        _ => return None,
    };

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

    let mut parameters: Vec<ParameterInfo> = params
        .iter()
        .enumerate()
        .map(|(i, param_type)| ParameterInfo {
            label: format!("(param{} {})", i, param_type),
            documentation: None,
        })
        .collect();

    parameters.push(ParameterInfo {
        label: "(funcref)".to_string(),
        documentation: Some("Function reference to call".to_string()),
    });

    let active_parameter = call_info.arg_text.matches(',').count() as u32;
    let param_count = parameters.len() as u32;
    let clamped = active_parameter.min(param_count);

    Some(SignatureHelpInfo {
        signature: SignatureInfo {
            label,
            documentation: Some(format!(
                "Indirect call via typed function reference. The last argument must be a function reference of type {}.",
                type_name
            )),
            parameters,
            active_parameter: clamped,
        },
    })
}
