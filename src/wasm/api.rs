//! wasm-bindgen API for browser usage.
//!
//! Provides the WatLSP class that can be used from JavaScript.

use wasm_bindgen::prelude::*;

use crate::core::types::{HoverResult, Position, Range};
use crate::symbols::SymbolTable;
use crate::wast_parser;

/// Initialize panic hook for better error messages in the browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// WAT Language Server for browser use
#[wasm_bindgen]
pub struct WatLSP {
    document: String,
    symbols: Option<SymbolTable>,
    ready: bool,
}

#[wasm_bindgen]
impl WatLSP {
    /// Create a new WAT LSP instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            document: String::new(),
            symbols: None,
            ready: false,
        }
    }

    /// Initialize the LSP. Returns true if successful.
    pub async fn initialize(&mut self) -> bool {
        self.ready = true;
        true
    }

    /// Check if the LSP is ready
    #[wasm_bindgen(getter)]
    pub fn ready(&self) -> bool {
        self.ready
    }

    /// Parse a WAT document and build symbol table
    pub fn parse(&mut self, document: &str) {
        self.document = document.to_string();

        match wast_parser::parse_document(document) {
            Ok(symbols) => {
                self.symbols = Some(symbols);
            }
            Err(_) => {
                // Parse failed - keep empty symbol table
                if self.symbols.is_none() {
                    self.symbols = Some(SymbolTable::new());
                }
            }
        }
    }

    /// Get diagnostics (syntax errors) for the current document
    #[wasm_bindgen(js_name = provideDiagnostics)]
    pub fn provide_diagnostics(&self) -> JsValue {
        let diagnostics = validate_wat(&self.document);
        let js_array = js_sys::Array::new();
        for diag in diagnostics {
            js_array.push(&diagnostic_to_js(&diag));
        }
        js_array.into()
    }

    /// Provide hover information at the given position
    #[wasm_bindgen(js_name = provideHover)]
    pub fn provide_hover(&self, line: u32, col: u32) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return JsValue::NULL,
        };

        let position = Position::new(line, col);

        // Get word at position
        let word = match get_word_at_position(&self.document, position) {
            Some(w) => w,
            None => return JsValue::NULL,
        };

        // Check if it's an instruction
        if let Some(doc) = get_instruction_doc(&word) {
            return hover_to_js(&HoverResult::new(doc));
        }

        // Check if it's a symbol reference
        if word.starts_with('$') {
            if let Some(hover) = provide_symbol_hover(&word, symbols, &self.document, position) {
                return hover_to_js(&hover);
            }
        }

        // Check numeric index
        if let Ok(index) = word.parse::<usize>() {
            if let Some(hover) = provide_index_hover(index, symbols, &self.document, position) {
                return hover_to_js(&hover);
            }
        }

        JsValue::NULL
    }

    /// Provide go-to-definition at the given position
    #[wasm_bindgen(js_name = provideDefinition)]
    pub fn provide_definition(&self, line: u32, col: u32) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return JsValue::NULL,
        };

        let position = Position::new(line, col);

        // Get word at position
        let word = match get_word_at_position(&self.document, position) {
            Some(w) => w,
            None => return JsValue::NULL,
        };

        // Find definition for symbols
        if word.starts_with('$') {
            if let Some(range) = find_symbol_definition(&word, symbols, &self.document, position) {
                return definition_to_js(&range);
            }
            // Fallback: if no range but symbol exists, use line number
            if let Some(func) = symbols.get_function_by_name(&word) {
                return definition_to_js(&Range::from_coords(func.line, 0, func.line, 0));
            }
            if let Some(global) = symbols.get_global_by_name(&word) {
                return definition_to_js(&Range::from_coords(global.line, 0, global.line, 0));
            }
        }

        // Find definition for numeric indices
        if let Ok(index) = word.parse::<usize>() {
            if let Some(range) = find_index_definition(index, symbols, &self.document, position) {
                return definition_to_js(&range);
            }
        }

        JsValue::NULL
    }

    /// Debug: get info about a word at position
    #[wasm_bindgen(js_name = debugWordAt)]
    pub fn debug_word_at(&self, line: u32, col: u32) -> JsValue {
        let position = Position::new(line, col);
        let word = get_word_at_position(&self.document, position);

        let obj = js_sys::Object::new();
        js_sys::Reflect::set(
            &obj,
            &"word".into(),
            &word.clone().map(|w| w.into()).unwrap_or(JsValue::NULL),
        )
        .ok();

        if let (Some(word), Some(symbols)) = (word, &self.symbols) {
            let func = symbols.get_function_by_name(&word);
            let has_func = func.is_some();
            let func_range = func.and_then(|f| f.range);
            let func_line = func.map(|f| f.line);

            js_sys::Reflect::set(&obj, &"hasFunction".into(), &has_func.into()).ok();
            js_sys::Reflect::set(
                &obj,
                &"functionRange".into(),
                &format!("{:?}", func_range).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"functionLine".into(),
                &func_line.map(|l| l.into()).unwrap_or(JsValue::NULL),
            )
            .ok();
        }

        obj.into()
    }

    /// Provide find-references at the given position
    #[wasm_bindgen(js_name = provideReferences)]
    pub fn provide_references(&self, line: u32, col: u32, include_declaration: bool) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return js_sys::Array::new().into(),
        };

        let position = Position::new(line, col);

        // Get word at position
        let word = match get_word_at_position(&self.document, position) {
            Some(w) => w,
            None => return js_sys::Array::new().into(),
        };

        let mut refs = Vec::new();

        // Find references for symbols
        if word.starts_with('$') {
            refs = find_symbol_references(&word, symbols, &self.document, include_declaration);
        }

        // Convert to JS array
        let js_array = js_sys::Array::new();
        for range in refs {
            js_array.push(&reference_to_js(&range));
        }
        js_array.into()
    }

    /// Get symbol table as HTML for debugging
    #[wasm_bindgen(js_name = getSymbolTableHTML)]
    pub fn get_symbol_table_html(&self) -> String {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return "<p>No symbols</p>".to_string(),
        };

        let mut html = String::new();

        // Functions
        if !symbols.functions.is_empty() {
            html.push_str("<h4>Functions</h4><ul>");
            for func in &symbols.functions {
                let name = func.name.as_deref().unwrap_or("(anonymous)");
                let params: Vec<String> = func
                    .parameters
                    .iter()
                    .map(|p| format!("{}", p.param_type))
                    .collect();
                let results: Vec<String> = func.results.iter().map(|r| format!("{}", r)).collect();
                html.push_str(&format!(
                    "<li>{} ({}): ({}) -> ({})</li>",
                    name,
                    func.index,
                    params.join(", "),
                    results.join(", ")
                ));
            }
            html.push_str("</ul>");
        }

        // Globals
        if !symbols.globals.is_empty() {
            html.push_str("<h4>Globals</h4><ul>");
            for global in &symbols.globals {
                let name = global.name.as_deref().unwrap_or("(anonymous)");
                let mutability = if global.is_mutable { "mut " } else { "" };
                html.push_str(&format!(
                    "<li>{} ({}): {}{}</li>",
                    name, global.index, mutability, global.var_type
                ));
            }
            html.push_str("</ul>");
        }

        if html.is_empty() {
            "<p>No symbols found</p>".to_string()
        } else {
            html
        }
    }
}

impl Default for WatLSP {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions

fn get_word_at_position(document: &str, position: Position) -> Option<String> {
    let lines: Vec<&str> = document.lines().collect();
    let line = lines.get(position.line as usize)?;

    let col = position.character as usize;
    if col > line.len() {
        return None;
    }

    // Find word boundaries
    let chars: Vec<char> = line.chars().collect();
    let mut start = col;
    let mut end = col;

    // Expand left
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }

    // Expand right
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }

    Some(chars[start..end].iter().collect())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || c == '.'
}

fn hover_to_js(hover: &HoverResult) -> JsValue {
    let obj = js_sys::Object::new();

    // Create contents object
    let contents = js_sys::Object::new();
    js_sys::Reflect::set(&contents, &"kind".into(), &"markdown".into()).ok();
    js_sys::Reflect::set(&contents, &"value".into(), &hover.contents.clone().into()).ok();
    js_sys::Reflect::set(&obj, &"contents".into(), &contents).ok();

    // Add range if present
    if let Some(range) = &hover.range {
        js_sys::Reflect::set(&obj, &"range".into(), &range_to_js(range)).ok();
    }

    obj.into()
}

fn range_to_js(range: &Range) -> JsValue {
    let obj = js_sys::Object::new();

    let start = js_sys::Object::new();
    js_sys::Reflect::set(&start, &"line".into(), &range.start.line.into()).ok();
    js_sys::Reflect::set(&start, &"character".into(), &range.start.character.into()).ok();

    let end = js_sys::Object::new();
    js_sys::Reflect::set(&end, &"line".into(), &range.end.line.into()).ok();
    js_sys::Reflect::set(&end, &"character".into(), &range.end.character.into()).ok();

    js_sys::Reflect::set(&obj, &"start".into(), &start).ok();
    js_sys::Reflect::set(&obj, &"end".into(), &end).ok();

    obj.into()
}

fn definition_to_js(range: &Range) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"range".into(), &range_to_js(range)).ok();
    obj.into()
}

fn reference_to_js(range: &Range) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"range".into(), &range_to_js(range)).ok();
    obj.into()
}

fn provide_symbol_hover(
    word: &str,
    symbols: &SymbolTable,
    _document: &str,
    position: Position,
) -> Option<HoverResult> {
    // Check functions
    if let Some(func) = symbols.get_function_by_name(word) {
        let params: Vec<String> = func
            .parameters
            .iter()
            .map(|p| {
                if let Some(name) = &p.name {
                    format!("{} {}", name, p.param_type)
                } else {
                    format!("{}", p.param_type)
                }
            })
            .collect();
        let results: Vec<String> = func.results.iter().map(|r| format!("{}", r)).collect();

        return Some(HoverResult::new(format!(
            "### Function `{}`\n\n**Index:** {}\n\n**Signature:** ({}) -> ({})",
            word,
            func.index,
            params.join(", "),
            results.join(", ")
        )));
    }

    // Check globals
    if let Some(global) = symbols.get_global_by_name(word) {
        let mutability = if global.is_mutable {
            "(mutable)"
        } else {
            "(immutable)"
        };
        return Some(HoverResult::new(format!(
            "### Global `{}`\n\n**Index:** {}\n\n**Type:** {} {}",
            word, global.index, global.var_type, mutability
        )));
    }

    // Check locals within containing function
    if let Some(func) = find_containing_function(symbols, position) {
        // Check parameters
        for param in &func.parameters {
            if param.name.as_ref() == Some(&word.to_string()) {
                return Some(HoverResult::new(format!(
                    "### Parameter `{}`\n\n**Index:** {}\n\n**Type:** {}",
                    word, param.index, param.param_type
                )));
            }
        }

        // Check locals
        for local in &func.locals {
            if local.name.as_ref() == Some(&word.to_string()) {
                return Some(HoverResult::new(format!(
                    "### Local `{}`\n\n**Index:** {}\n\n**Type:** {}",
                    word,
                    local.index + func.parameters.len(),
                    local.var_type
                )));
            }
        }
    }

    None
}

fn provide_index_hover(
    index: usize,
    symbols: &SymbolTable,
    document: &str,
    position: Position,
) -> Option<HoverResult> {
    // Determine context from line
    let lines: Vec<&str> = document.lines().collect();
    let line = lines.get(position.line as usize)?;

    if line.contains("call") {
        // Function index
        if let Some(func) = symbols.get_function_by_index(index) {
            let name = func.name.as_deref().unwrap_or("(anonymous)");
            return Some(HoverResult::new(format!(
                "### Function {}\n\n**Name:** {}",
                index, name
            )));
        }
    } else if line.contains("global") {
        // Global index
        if let Some(global) = symbols.get_global_by_index(index) {
            let name = global.name.as_deref().unwrap_or("(anonymous)");
            return Some(HoverResult::new(format!(
                "### Global {}\n\n**Name:** {}\n\n**Type:** {}",
                index, name, global.var_type
            )));
        }
    } else if line.contains("local") {
        // Local index within function
        if let Some(func) = find_containing_function(symbols, position) {
            let total_params = func.parameters.len();
            if index < total_params {
                let param = &func.parameters[index];
                let name = param.name.as_deref().unwrap_or("(anonymous)");
                return Some(HoverResult::new(format!(
                    "### Parameter {}\n\n**Name:** {}\n\n**Type:** {}",
                    index, name, param.param_type
                )));
            } else {
                let local_idx = index - total_params;
                if let Some(local) = func.locals.get(local_idx) {
                    let name = local.name.as_deref().unwrap_or("(anonymous)");
                    return Some(HoverResult::new(format!(
                        "### Local {}\n\n**Name:** {}\n\n**Type:** {}",
                        index, name, local.var_type
                    )));
                }
            }
        }
    }

    None
}

fn find_symbol_definition(
    word: &str,
    symbols: &SymbolTable,
    _document: &str,
    position: Position,
) -> Option<Range> {
    // Check functions
    if let Some(func) = symbols.get_function_by_name(word) {
        return func.range;
    }

    // Check globals
    if let Some(global) = symbols.get_global_by_name(word) {
        return global.range;
    }

    // Check locals within containing function
    if let Some(func) = find_containing_function(symbols, position) {
        for param in &func.parameters {
            if param.name.as_ref() == Some(&word.to_string()) {
                return param.range;
            }
        }
        for local in &func.locals {
            if local.name.as_ref() == Some(&word.to_string()) {
                return local.range;
            }
        }
    }

    // Check tables
    if let Some(table) = symbols.get_table_by_name(word) {
        return table.range;
    }

    // Check memories
    if let Some(memory) = symbols.get_memory_by_name(word) {
        return memory.range;
    }

    // Check types
    if let Some(type_def) = symbols.get_type_by_name(word) {
        return type_def.range;
    }

    // Check tags
    if let Some(tag) = symbols.get_tag_by_name(word) {
        return tag.range;
    }

    None
}

fn find_index_definition(
    index: usize,
    symbols: &SymbolTable,
    document: &str,
    position: Position,
) -> Option<Range> {
    // Determine context from line
    let lines: Vec<&str> = document.lines().collect();
    let line = lines.get(position.line as usize)?;

    if line.contains("call") {
        symbols.get_function_by_index(index)?.range
    } else if line.contains("global") {
        symbols.get_global_by_index(index)?.range
    } else if line.contains("local") {
        if let Some(func) = find_containing_function(symbols, position) {
            let total_params = func.parameters.len();
            if index < total_params {
                func.parameters.get(index)?.range
            } else {
                func.locals.get(index - total_params)?.range
            }
        } else {
            None
        }
    } else {
        None
    }
}

fn find_symbol_references(
    word: &str,
    symbols: &SymbolTable,
    document: &str,
    include_declaration: bool,
) -> Vec<Range> {
    let mut refs = Vec::new();

    // Simple text search for the symbol name
    for (line_num, line) in document.lines().enumerate() {
        let mut col = 0;
        while let Some(pos) = line[col..].find(word) {
            let abs_pos = col + pos;
            let end_pos = abs_pos + word.len();

            // Check that it's a complete word (not part of a longer identifier)
            let before_ok =
                abs_pos == 0 || !is_word_char(line.chars().nth(abs_pos - 1).unwrap_or(' '));
            let after_ok =
                end_pos >= line.len() || !is_word_char(line.chars().nth(end_pos).unwrap_or(' '));

            if before_ok && after_ok {
                refs.push(Range::from_coords(
                    line_num as u32,
                    abs_pos as u32,
                    line_num as u32,
                    end_pos as u32,
                ));
            }

            col = abs_pos + 1;
        }
    }

    // Remove declaration if not included
    if !include_declaration && !refs.is_empty() {
        if let Some(def_range) =
            find_symbol_definition(word, symbols, document, Position::new(0, 0))
        {
            refs.retain(|r| {
                r.start.line != def_range.start.line
                    || r.start.character != def_range.start.character
            });
        }
    }

    refs
}

fn find_containing_function(
    symbols: &SymbolTable,
    position: Position,
) -> Option<&crate::symbols::Function> {
    symbols
        .functions
        .iter()
        .find(|func| position.line >= func.line && position.line <= func.end_line)
}

/// Diagnostic information for syntax errors
struct WasmDiagnostic {
    line: u32,
    character: u32,
    end_character: u32,
    message: String,
    severity: u32, // 1 = error, 2 = warning, 3 = info, 4 = hint
}

/// Validate WAT source and return diagnostics
fn validate_wat(source: &str) -> Vec<WasmDiagnostic> {
    if source.trim().is_empty() {
        return vec![];
    }

    let buf = match wast::parser::ParseBuffer::new(source) {
        Ok(buf) => buf,
        Err(e) => return vec![wast_error_to_diagnostic(&e, source)],
    };

    match wast::parser::parse::<wast::Wat>(&buf) {
        Ok(_) => vec![],
        Err(e) => vec![wast_error_to_diagnostic(&e, source)],
    }
}

fn wast_error_to_diagnostic(error: &wast::Error, source: &str) -> WasmDiagnostic {
    let span = error.span();
    let (line, col) = span.linecol_in(source);

    WasmDiagnostic {
        line: line as u32,
        character: col as u32,
        end_character: (col + 1) as u32,
        message: error.to_string(),
        severity: 1, // Error
    }
}

fn diagnostic_to_js(diag: &WasmDiagnostic) -> JsValue {
    let obj = js_sys::Object::new();

    let range = js_sys::Object::new();
    let start = js_sys::Object::new();
    js_sys::Reflect::set(&start, &"line".into(), &diag.line.into()).ok();
    js_sys::Reflect::set(&start, &"character".into(), &diag.character.into()).ok();
    let end = js_sys::Object::new();
    js_sys::Reflect::set(&end, &"line".into(), &diag.line.into()).ok();
    js_sys::Reflect::set(&end, &"character".into(), &diag.end_character.into()).ok();
    js_sys::Reflect::set(&range, &"start".into(), &start).ok();
    js_sys::Reflect::set(&range, &"end".into(), &end).ok();

    js_sys::Reflect::set(&obj, &"range".into(), &range).ok();
    js_sys::Reflect::set(&obj, &"message".into(), &diag.message.clone().into()).ok();
    js_sys::Reflect::set(&obj, &"severity".into(), &diag.severity.into()).ok();

    obj.into()
}

fn get_instruction_doc(word: &str) -> Option<String> {
    INSTRUCTION_DOCS.get(word).map(|s| s.to_string())
}

// Include the auto-generated instruction documentation
include!(concat!(env!("OUT_DIR"), "/instruction_docs.rs"));
