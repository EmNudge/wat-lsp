//! Shared document symbols core — protocol-independent logic for both native and WASM.
//!
//! Converts a `SymbolTable` into a `Vec<DocumentSymbolInfo>` that can be
//! rendered by either the native LSP (via `Into<DocumentSymbol>`) or the
//! WASM API (via JS object conversion).

use crate::core::types::{DocumentSymbolInfo, Range, SymbolKindCore};
use crate::symbols::*;

/// Produce a hierarchical outline of all symbols in the document.
pub fn provide_document_symbols_core(symbols: &SymbolTable) -> Vec<DocumentSymbolInfo> {
    let mut result = Vec::new();

    for type_def in &symbols.types {
        result.push(type_to_symbol(type_def));
    }
    for func in &symbols.functions {
        result.push(function_to_symbol(func));
    }
    for global in &symbols.globals {
        result.push(global_to_symbol(global));
    }
    for table in &symbols.tables {
        result.push(table_to_symbol(table));
    }
    for memory in &symbols.memories {
        result.push(memory_to_symbol(memory));
    }
    for tag in &symbols.tags {
        result.push(tag_to_symbol(tag));
    }
    for data in &symbols.data_segments {
        result.push(data_to_symbol(data));
    }
    for elem in &symbols.elem_segments {
        result.push(elem_to_symbol(elem));
    }

    result
}

fn range_or_line(range: &Option<Range>, line: u32) -> Range {
    range.unwrap_or_else(|| Range::from_coords(line, 0, line, 0))
}

fn type_to_symbol(type_def: &TypeDef) -> DocumentSymbolInfo {
    let name = type_def
        .name
        .clone()
        .unwrap_or_else(|| format!("(type {})", type_def.index));

    let detail = match &type_def.kind {
        TypeKind::Func { params, results } => {
            let params_str = params
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let results_str = results
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            if results_str.is_empty() {
                format!("(func ({}))", params_str)
            } else {
                format!("(func ({}) -> ({}))", params_str, results_str)
            }
        }
        TypeKind::Struct { fields } => format!("(struct {} fields)", fields.len()),
        TypeKind::Array { element_type, .. } => format!("(array {})", element_type),
    };

    DocumentSymbolInfo {
        name,
        detail: Some(detail),
        kind: SymbolKindCore::Interface,
        range: range_or_line(&type_def.range, type_def.line),
        children: None,
    }
}

fn function_to_symbol(func: &Function) -> DocumentSymbolInfo {
    let name = func
        .name
        .clone()
        .unwrap_or_else(|| format!("(func {})", func.index));

    let params_str = func
        .parameters
        .iter()
        .map(|p| {
            if let Some(ref name) = p.name {
                format!("{} {}", name, p.param_type)
            } else {
                p.param_type.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let results_str = func
        .results
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let detail = if results_str.is_empty() {
        format!("({})", params_str)
    } else {
        format!("({}) -> ({})", params_str, results_str)
    };

    let range = func
        .range
        .unwrap_or_else(|| Range::from_coords(func.line, 0, func.end_line, 0));

    let mut children = Vec::new();
    for param in &func.parameters {
        children.push(param_to_symbol(param));
    }
    for local in &func.locals {
        children.push(local_to_symbol(local));
    }
    for block in &func.blocks {
        children.push(block_to_symbol(block));
    }

    DocumentSymbolInfo {
        name,
        detail: Some(detail),
        kind: SymbolKindCore::Function,
        range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

fn param_to_symbol(param: &Parameter) -> DocumentSymbolInfo {
    let name = param
        .name
        .clone()
        .unwrap_or_else(|| format!("(param {})", param.index));

    DocumentSymbolInfo {
        name,
        detail: Some(param.param_type.to_string()),
        kind: SymbolKindCore::Variable,
        range: range_or_line(&param.range, 0),
        children: None,
    }
}

fn local_to_symbol(local: &Variable) -> DocumentSymbolInfo {
    let name = local
        .name
        .clone()
        .unwrap_or_else(|| format!("(local {})", local.index));

    DocumentSymbolInfo {
        name,
        detail: Some(local.var_type.to_string()),
        kind: SymbolKindCore::Variable,
        range: range_or_line(&local.range, 0),
        children: None,
    }
}

fn block_to_symbol(block: &BlockLabel) -> DocumentSymbolInfo {
    DocumentSymbolInfo {
        name: block.label.clone(),
        detail: Some(block.block_type.clone()),
        kind: SymbolKindCore::Key,
        range: range_or_line(&block.range, block.line),
        children: None,
    }
}

fn global_to_symbol(global: &Global) -> DocumentSymbolInfo {
    let name = global
        .name
        .clone()
        .unwrap_or_else(|| format!("(global {})", global.index));

    let detail = format!(
        "{}{}",
        if global.is_mutable { "mut " } else { "" },
        global.var_type
    );

    DocumentSymbolInfo {
        name,
        detail: Some(detail),
        kind: SymbolKindCore::Constant,
        range: range_or_line(&global.range, global.line),
        children: None,
    }
}

fn table_to_symbol(table: &Table) -> DocumentSymbolInfo {
    let name = table
        .name
        .clone()
        .unwrap_or_else(|| format!("(table {})", table.index));

    let detail = format!(
        "{} {} {}",
        table.limits.0,
        table.limits.1.map_or("".to_string(), |m| m.to_string()),
        table.ref_type
    );

    DocumentSymbolInfo {
        name,
        detail: Some(detail),
        kind: SymbolKindCore::Array,
        range: range_or_line(&table.range, table.line),
        children: None,
    }
}

fn memory_to_symbol(memory: &Memory) -> DocumentSymbolInfo {
    let name = memory
        .name
        .clone()
        .unwrap_or_else(|| format!("(memory {})", memory.index));

    let detail = format!(
        "{}{}",
        memory.limits.0,
        memory
            .limits
            .1
            .map_or("".to_string(), |m| format!(" {}", m))
    );

    DocumentSymbolInfo {
        name,
        detail: Some(detail),
        kind: SymbolKindCore::Object,
        range: range_or_line(&memory.range, memory.line),
        children: None,
    }
}

fn tag_to_symbol(tag: &Tag) -> DocumentSymbolInfo {
    let name = tag
        .name
        .clone()
        .unwrap_or_else(|| format!("(tag {})", tag.index));

    let detail = if tag.params.is_empty() {
        "()".to_string()
    } else {
        format!(
            "({})",
            tag.params
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )
    };

    DocumentSymbolInfo {
        name,
        detail: Some(detail),
        kind: SymbolKindCore::Event,
        range: range_or_line(&tag.range, tag.line),
        children: None,
    }
}

fn data_to_symbol(data: &DataSegment) -> DocumentSymbolInfo {
    let name = data
        .name
        .clone()
        .unwrap_or_else(|| format!("(data {})", data.index));

    DocumentSymbolInfo {
        name,
        detail: Some(format!("{} bytes", data.byte_length)),
        kind: SymbolKindCore::String,
        range: range_or_line(&data.range, data.line),
        children: None,
    }
}

fn elem_to_symbol(elem: &ElemSegment) -> DocumentSymbolInfo {
    let name = elem
        .name
        .clone()
        .unwrap_or_else(|| format!("(elem {})", elem.index));

    DocumentSymbolInfo {
        name,
        detail: Some(format!("{} functions", elem.func_names.len())),
        kind: SymbolKindCore::Array,
        range: range_or_line(&elem.range, elem.line),
        children: None,
    }
}
