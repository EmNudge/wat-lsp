use crate::symbols::*;
use tower_lsp::lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

#[cfg(test)]
mod tests;

/// Provide document symbols for the outline view.
/// Returns a hierarchical list of symbols where functions contain their locals/params as children.
pub fn provide_document_symbols(symbols: &SymbolTable) -> Vec<DocumentSymbol> {
    let mut result = Vec::new();

    // Add types
    for type_def in &symbols.types {
        result.push(type_to_document_symbol(type_def));
    }

    // Add functions with their children (params, locals, blocks)
    for func in &symbols.functions {
        result.push(function_to_document_symbol(func));
    }

    // Add globals
    for global in &symbols.globals {
        result.push(global_to_document_symbol(global));
    }

    // Add tables
    for table in &symbols.tables {
        result.push(table_to_document_symbol(table));
    }

    // Add memories
    for memory in &symbols.memories {
        result.push(memory_to_document_symbol(memory));
    }

    // Add tags
    for tag in &symbols.tags {
        result.push(tag_to_document_symbol(tag));
    }

    // Add data segments
    for data in &symbols.data_segments {
        result.push(data_to_document_symbol(data));
    }

    // Add element segments
    for elem in &symbols.elem_segments {
        result.push(elem_to_document_symbol(elem));
    }

    result
}

fn core_range_to_lsp(range: &crate::core::types::Range) -> Range {
    Range {
        start: Position {
            line: range.start.line,
            character: range.start.character,
        },
        end: Position {
            line: range.end.line,
            character: range.end.character,
        },
    }
}

fn make_range_from_line(line: u32) -> Range {
    Range {
        start: Position { line, character: 0 },
        end: Position { line, character: 0 },
    }
}

fn type_to_document_symbol(type_def: &TypeDef) -> DocumentSymbol {
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

    let range = type_def
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(type_def.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::INTERFACE,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn function_to_document_symbol(func: &Function) -> DocumentSymbol {
    let name = func
        .name
        .clone()
        .unwrap_or_else(|| format!("(func {})", func.index));

    // Build function signature for detail
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
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| Range {
            start: Position {
                line: func.line,
                character: 0,
            },
            end: Position {
                line: func.end_line,
                character: 0,
            },
        });

    // Build children: parameters, locals, and block labels
    let mut children = Vec::new();

    // Add parameters
    for param in &func.parameters {
        children.push(param_to_document_symbol(param));
    }

    // Add locals
    for local in &func.locals {
        children.push(local_to_document_symbol(local));
    }

    // Add block labels
    for block in &func.blocks {
        children.push(block_to_document_symbol(block));
    }

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::FUNCTION,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

fn param_to_document_symbol(param: &Parameter) -> DocumentSymbol {
    let name = param
        .name
        .clone()
        .unwrap_or_else(|| format!("(param {})", param.index));

    let range = param
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(0));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(param.param_type.to_string()),
        kind: SymbolKind::VARIABLE,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn local_to_document_symbol(local: &Variable) -> DocumentSymbol {
    let name = local
        .name
        .clone()
        .unwrap_or_else(|| format!("(local {})", local.index));

    let range = local
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(0));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(local.var_type.to_string()),
        kind: SymbolKind::VARIABLE,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn block_to_document_symbol(block: &BlockLabel) -> DocumentSymbol {
    let range = block
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(block.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name: block.label.clone(),
        detail: Some(block.block_type.clone()),
        kind: SymbolKind::KEY,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn global_to_document_symbol(global: &Global) -> DocumentSymbol {
    let name = global
        .name
        .clone()
        .unwrap_or_else(|| format!("(global {})", global.index));

    let detail = format!(
        "{}{}",
        if global.is_mutable { "mut " } else { "" },
        global.var_type
    );

    let range = global
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(global.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::CONSTANT,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn table_to_document_symbol(table: &Table) -> DocumentSymbol {
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

    let range = table
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(table.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::ARRAY,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn memory_to_document_symbol(memory: &Memory) -> DocumentSymbol {
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

    let range = memory
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(memory.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::OBJECT,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn tag_to_document_symbol(tag: &Tag) -> DocumentSymbol {
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

    let range = tag
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(tag.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::EVENT,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn data_to_document_symbol(data: &DataSegment) -> DocumentSymbol {
    let name = data
        .name
        .clone()
        .unwrap_or_else(|| format!("(data {})", data.index));

    let detail = format!("{} bytes", data.byte_length);

    let range = data
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(data.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::STRING,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn elem_to_document_symbol(elem: &ElemSegment) -> DocumentSymbol {
    let name = elem
        .name
        .clone()
        .unwrap_or_else(|| format!("(elem {})", elem.index));

    let detail = format!("{} functions", elem.func_names.len());

    let range = elem
        .range
        .as_ref()
        .map(core_range_to_lsp)
        .unwrap_or_else(|| make_range_from_line(elem.line));

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail: Some(detail),
        kind: SymbolKind::ARRAY,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}
