//! Parser implementation using the wast crate.
//!
//! This parser is used for WASM builds since tree-sitter has C dependencies
//! that don't compile to WASM. The wast crate is pure Rust and works in WASM.

use crate::core::types::Range;
use crate::symbols::*;

/// Parse a WAT document and extract symbols using the wast crate.
pub fn parse_document(source: &str) -> Result<SymbolTable, String> {
    let buf =
        wast::parser::ParseBuffer::new(source).map_err(|e| format!("Parse buffer error: {}", e))?;

    let wat: wast::Wat = wast::parser::parse(&buf).map_err(|e| format!("Parse error: {}", e))?;

    extract_symbols(&wat, source)
}

/// Extract symbols from parsed WAT AST
fn extract_symbols(wat: &wast::Wat, source: &str) -> Result<SymbolTable, String> {
    let mut symbols = SymbolTable::new();

    // Get the module (ignore components for now)
    let module = match wat {
        wast::Wat::Module(m) => m,
        wast::Wat::Component(_) => return Ok(symbols),
    };

    // Get fields based on module kind
    let fields = match &module.kind {
        wast::core::ModuleKind::Text(fields) => fields,
        wast::core::ModuleKind::Binary(_) => return Ok(symbols),
    };

    // Track indices
    let mut func_index = 0usize;
    let mut global_index = 0usize;
    let mut table_index = 0usize;
    let mut memory_index = 0usize;
    let mut type_index = 0usize;
    let mut tag_index = 0usize;

    // Process module fields
    for field in fields.iter() {
        match field {
            wast::core::ModuleField::Import(import) => {
                let name = import.item.id.map(|id| format!("${}", id.name()));
                let range = import.item.id.map(|id| id_to_range(id, source));
                let line = span_to_line(import.span, source);

                match &import.item.kind {
                    wast::core::ItemKind::Func(_) | wast::core::ItemKind::FuncExact(_) => {
                        symbols.add_function(Function {
                            name,
                            index: func_index,
                            parameters: Vec::new(),
                            results: Vec::new(),
                            locals: Vec::new(),
                            blocks: Vec::new(),
                            line,
                            end_line: line,
                            start_byte: import.span.offset(),
                            end_byte: import.span.offset(),
                            range,
                        });
                        func_index += 1;
                    }
                    wast::core::ItemKind::Global(g) => {
                        symbols.add_global(Global {
                            name,
                            index: global_index,
                            var_type: extract_value_type(&g.ty),
                            is_mutable: g.mutable,
                            initial_value: None,
                            line,
                            range,
                        });
                        global_index += 1;
                    }
                    wast::core::ItemKind::Table(_) => {
                        symbols.add_table(Table {
                            name,
                            index: table_index,
                            ref_type: ValueType::Funcref,
                            limits: (0, None),
                            line,
                            range,
                        });
                        table_index += 1;
                    }
                    wast::core::ItemKind::Memory(_) => {
                        symbols.add_memory(Memory {
                            name,
                            index: memory_index,
                            limits: (0, None),
                            line,
                            range,
                        });
                        memory_index += 1;
                    }
                    wast::core::ItemKind::Tag(_) => {
                        symbols.add_tag(Tag {
                            name,
                            index: tag_index,
                            params: Vec::new(),
                            line,
                            range,
                        });
                        tag_index += 1;
                    }
                }
            }
            wast::core::ModuleField::Func(func) => {
                let name = func.id.map(|id| format!("${}", id.name()));
                let line = span_to_line(func.span, source);
                let end_line = find_func_end_line(func.span, source);

                // Extract parameters and results from type annotation
                let (parameters, results) = extract_func_type_info(func, source);

                // Extract locals
                let locals = extract_func_locals(func, source);

                symbols.add_function(Function {
                    name,
                    index: func_index,
                    parameters,
                    results,
                    locals,
                    blocks: Vec::new(),
                    line,
                    end_line,
                    start_byte: func.span.offset(),
                    end_byte: func.span.offset(),
                    range: func.id.map(|id| id_to_range(id, source)),
                });
                func_index += 1;
            }
            wast::core::ModuleField::Global(global) => {
                symbols.add_global(Global {
                    name: global.id.map(|id| format!("${}", id.name())),
                    index: global_index,
                    var_type: extract_value_type(&global.ty.ty),
                    is_mutable: global.ty.mutable,
                    initial_value: None,
                    line: span_to_line(global.span, source),
                    range: global.id.map(|id| id_to_range(id, source)),
                });
                global_index += 1;
            }
            wast::core::ModuleField::Table(table) => {
                symbols.add_table(Table {
                    name: table.id.map(|id| format!("${}", id.name())),
                    index: table_index,
                    ref_type: ValueType::Funcref,
                    limits: (0, None),
                    line: span_to_line(table.span, source),
                    range: table.id.map(|id| id_to_range(id, source)),
                });
                table_index += 1;
            }
            wast::core::ModuleField::Memory(memory) => {
                symbols.add_memory(Memory {
                    name: memory.id.map(|id| format!("${}", id.name())),
                    index: memory_index,
                    limits: (0, None),
                    line: span_to_line(memory.span, source),
                    range: memory.id.map(|id| id_to_range(id, source)),
                });
                memory_index += 1;
            }
            wast::core::ModuleField::Type(type_def) => {
                symbols.add_type(TypeDef {
                    name: type_def.id.map(|id| format!("${}", id.name())),
                    index: type_index,
                    kind: TypeKind::Func {
                        params: Vec::new(),
                        results: Vec::new(),
                    },
                    line: span_to_line(type_def.span, source),
                    range: type_def.id.map(|id| id_to_range(id, source)),
                });
                type_index += 1;
            }
            wast::core::ModuleField::Tag(tag) => {
                symbols.add_tag(Tag {
                    name: tag.id.map(|id| format!("${}", id.name())),
                    index: tag_index,
                    params: Vec::new(),
                    line: span_to_line(tag.span, source),
                    range: tag.id.map(|id| id_to_range(id, source)),
                });
                tag_index += 1;
            }
            _ => {}
        }
    }

    Ok(symbols)
}

/// Convert wast Span to line number
fn span_to_line(span: wast::token::Span, source: &str) -> u32 {
    let (line, _col) = span.linecol_in(source);
    line as u32
}

/// Find the end line of a function by counting matching parentheses from the span offset
fn find_func_end_line(span: wast::token::Span, source: &str) -> u32 {
    let start_offset = span.offset();
    let bytes = source.as_bytes();
    let mut paren_depth = 0;
    let mut end_offset = start_offset;

    for (i, &byte) in bytes.iter().enumerate().skip(start_offset) {
        match byte {
            b'(' => paren_depth += 1,
            b')' => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    end_offset = i;
                    break;
                }
            }
            _ => {}
        }
    }

    // Count newlines up to end_offset
    let end_line = source[..=end_offset.min(source.len() - 1)]
        .chars()
        .filter(|&c| c == '\n')
        .count();
    end_line as u32
}

/// Convert Id to Range
fn id_to_range(id: wast::token::Id, source: &str) -> Range {
    let (line, col) = id.span().linecol_in(source);
    let name_len = id.name().len() + 1; // +1 for the $ prefix
    Range::from_coords(
        line as u32,
        col as u32,
        line as u32,
        (col + name_len) as u32,
    )
}

/// Extract function parameters and results from type info
fn extract_func_type_info(
    func: &wast::core::Func,
    source: &str,
) -> (Vec<Parameter>, Vec<ValueType>) {
    let mut params = Vec::new();
    let mut results = Vec::new();

    if let wast::core::FuncKind::Inline { .. } = &func.kind {
        // Get inline type if present
        if let Some(inline) = &func.ty.inline {
            for (index, param) in inline.params.iter().enumerate() {
                // param is a tuple: (Option<Id>, Option<NameAnnotation>, ValType)
                let name = param.0.map(|id| format!("${}", id.name()));
                let range = param.0.map(|id| id_to_range(id, source));
                let val_type = extract_value_type(&param.2);

                params.push(Parameter {
                    name,
                    param_type: val_type,
                    index,
                    range,
                });
            }

            for result in &inline.results {
                results.push(extract_value_type(result));
            }
        }
    }

    (params, results)
}

/// Extract function locals
fn extract_func_locals(func: &wast::core::Func, source: &str) -> Vec<Variable> {
    let mut locals = Vec::new();

    if let wast::core::FuncKind::Inline {
        locals: func_locals,
        ..
    } = &func.kind
    {
        for (index, local) in func_locals.iter().enumerate() {
            let name = local.id.map(|id| format!("${}", id.name()));
            let range = local.id.map(|id| id_to_range(id, source));

            locals.push(Variable {
                name,
                var_type: extract_value_type(&local.ty),
                is_mutable: true,
                initial_value: None,
                index,
                range,
            });
        }
    }

    locals
}

/// Convert wast ValType to our ValueType.
/// Uses the centralized From implementation in symbols.rs.
#[inline]
fn extract_value_type(val_type: &wast::core::ValType) -> ValueType {
    ValueType::from(val_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_module() {
        let source = r#"
            (module
                (func $add (param $a i32) (param $b i32) (result i32)
                    local.get $a
                    local.get $b
                    i32.add
                )
            )
        "#;

        let result = parse_document(source);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let symbols = result.unwrap();
        assert_eq!(symbols.functions.len(), 1);

        let func = &symbols.functions[0];
        assert_eq!(func.name, Some("$add".to_string()));
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.results.len(), 1);
    }

    #[test]
    fn test_parse_globals() {
        let source = r#"
            (module
                (global $counter (mut i32) (i32.const 0))
                (global $max i32 (i32.const 100))
            )
        "#;

        let result = parse_document(source);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let symbols = result.unwrap();
        assert_eq!(symbols.globals.len(), 2);
    }
}
