//! wasm-bindgen API for browser usage.
//!
//! Provides the WatLSP class that can be used from JavaScript.

use wasm_bindgen::prelude::*;

use crate::core::types::{HoverResult, Position, Range};
use crate::hover::provide_hover_core;
use crate::parser::parse_document_from_tree;
use crate::symbol_lookup::{find_symbol_definition_range, IndexContext};
use crate::symbols::SymbolTable;
use crate::ts_facade::{self, Language, Parser, Query, Tree};
use crate::utils::{get_word_at_position, is_word_char};

/// The highlights.scm query for syntax highlighting
const HIGHLIGHTS_QUERY: &str =
    include_str!("../../grammars/tree-sitter-wat/queries/highlights.scm");

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
    tree: Option<Tree>,
    parser: Option<Parser>,
    language: Option<Language>,
    highlight_query: Option<Query>,
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
            tree: None,
            parser: None,
            language: None,
            highlight_query: None,
            ready: false,
        }
    }

    /// Initialize the LSP (initializes tree-sitter). Returns true if successful.
    pub async fn initialize(&mut self) -> bool {
        // Initialize tree-sitter runtime
        if let Err(e) = ts_facade::init().await {
            web_sys::console::error_1(&format!("Failed to init tree-sitter: {:?}", e).into());
            return false;
        }

        // Create parser and get language
        match ts_facade::create_parser().await {
            Ok((parser, language)) => {
                // Create highlight query from the language
                match language.query(HIGHLIGHTS_QUERY) {
                    Ok(query) => {
                        self.highlight_query = Some(query);
                    }
                    Err(e) => {
                        web_sys::console::warn_1(
                            &format!("Failed to create highlight query: {}", e).into(),
                        );
                    }
                }

                self.parser = Some(parser);
                self.language = Some(language);
                self.ready = true;
                true
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Failed to create parser: {}", e).into());
                false
            }
        }
    }

    /// Check if the LSP is ready
    #[wasm_bindgen(getter)]
    pub fn ready(&self) -> bool {
        self.ready
    }

    /// Parse a WAT document and build symbol table using tree-sitter
    pub fn parse(&mut self, document: &str) {
        self.document = document.to_string();

        // Parse with tree-sitter if parser is available
        if let Some(parser) = &mut self.parser {
            if let Some(tree) = parser.parse(document, None) {
                // Extract symbols from tree
                match parse_document_from_tree(&tree, document) {
                    Ok(symbols) => {
                        self.symbols = Some(symbols);
                        self.tree = Some(tree);
                    }
                    Err(_) => {
                        // Symbol extraction failed, but tree is still valid
                        self.tree = Some(tree);
                        if self.symbols.is_none() {
                            self.symbols = Some(SymbolTable::new());
                        }
                    }
                }
            } else {
                // Parse failed
                self.tree = None;
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

    /// Provide hover information at the given position (uses tree-sitter based hover)
    #[wasm_bindgen(js_name = provideHover)]
    pub fn provide_hover(&self, line: u32, col: u32) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return JsValue::NULL,
        };

        let tree = match &self.tree {
            Some(t) => t,
            None => return JsValue::NULL,
        };

        let position = Position::new(line, col);

        // Use the shared tree-sitter based hover implementation
        match provide_hover_core(&self.document, symbols, tree, position) {
            Some(hover) => hover_to_js(&hover),
            None => JsValue::NULL,
        }
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
            if let Some(range) = find_symbol_definition_range(&word, symbols, position) {
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

    /// Provide semantic tokens for syntax highlighting
    /// Returns a flat array of u32 values in Monaco's delta-encoded format:
    /// [deltaLine, deltaStartChar, length, tokenType, tokenModifiers, ...]
    #[wasm_bindgen(js_name = provideSemanticTokens)]
    pub fn provide_semantic_tokens(&self) -> js_sys::Uint32Array {
        let empty = js_sys::Uint32Array::new_with_length(0);

        let tree = match &self.tree {
            Some(t) => t,
            None => return empty,
        };

        let query = match &self.highlight_query {
            Some(q) => q,
            None => return empty,
        };

        // Run the query on the root node
        let root = tree.root_node();
        let captures = query.captures(&root);

        // Collect all tokens with their positions
        let mut tokens: Vec<(u32, u32, u32, u32, u32)> = Vec::new();

        for capture in captures {
            let name = capture.name();
            let node = capture.node();

            let start_pos = node.start_position();
            let end_pos = node.end_position();

            // Calculate length (handle multi-line tokens)
            let length = if start_pos.row == end_pos.row {
                (end_pos.column - start_pos.column) as u32
            } else {
                // For multi-line tokens, just use the first line length
                // This is a simplification; proper handling would split tokens
                (node.end_byte() - node.start_byte()) as u32
            };

            // Map capture name to token type index
            let (token_type, token_modifiers) = capture_name_to_token(&name);

            tokens.push((
                start_pos.row as u32,
                start_pos.column as u32,
                length,
                token_type,
                token_modifiers,
            ));
        }

        // Sort tokens by position (line, then column)
        tokens.sort_by(|a, b| {
            if a.0 != b.0 {
                a.0.cmp(&b.0)
            } else {
                a.1.cmp(&b.1)
            }
        });

        // Delta-encode the tokens, skipping overlapping positions
        let mut result: Vec<u32> = Vec::with_capacity(tokens.len() * 5);
        let mut prev_line = 0u32;
        let mut prev_col = 0u32;
        let mut last_end_col = 0u32; // Track end of last token to detect overlaps

        for (line, col, length, token_type, token_modifiers) in tokens {
            // Skip tokens that overlap with the previous one on the same line
            if line == prev_line && col < last_end_col {
                continue;
            }

            let delta_line = line - prev_line;
            let delta_col = if delta_line == 0 { col - prev_col } else { col };

            result.push(delta_line);
            result.push(delta_col);
            result.push(length);
            result.push(token_type);
            result.push(token_modifiers);

            prev_line = line;
            prev_col = col;
            last_end_col = col + length;
        }

        js_sys::Uint32Array::from(&result[..])
    }

    /// Get the semantic token legend (token types and modifiers)
    #[wasm_bindgen(js_name = getSemanticTokensLegend)]
    pub fn get_semantic_tokens_legend(&self) -> JsValue {
        let obj = js_sys::Object::new();

        // Token types - must match the indices used in capture_name_to_token
        let types = js_sys::Array::new();
        types.push(&"comment".into()); // 0
        types.push(&"string".into()); // 1
        types.push(&"number".into()); // 2
        types.push(&"type".into()); // 3
        types.push(&"keyword".into()); // 4
        types.push(&"function".into()); // 5
        types.push(&"variable".into()); // 6
        types.push(&"operator".into()); // 7

        // Token modifiers - bit flags
        let modifiers = js_sys::Array::new();
        modifiers.push(&"definition".into()); // bit 0
        modifiers.push(&"builtin".into()); // bit 1
        modifiers.push(&"instruction".into()); // bit 2
        modifiers.push(&"parameter".into()); // bit 3
        modifiers.push(&"local".into()); // bit 4
        modifiers.push(&"control".into()); // bit 5

        js_sys::Reflect::set(&obj, &"tokenTypes".into(), &types).ok();
        js_sys::Reflect::set(&obj, &"tokenModifiers".into(), &modifiers).ok();

        obj.into()
    }
}

/// Map a capture name from highlights.scm to (tokenType index, tokenModifiers bitmask)
fn capture_name_to_token(name: &str) -> (u32, u32) {
    match name {
        // Comments
        "comment" => (0, 0),

        // Strings
        "string" => (1, 0),

        // Numbers
        "number" => (2, 0),

        // Types
        "type" | "type.builtin" => (3, 0b10), // builtin modifier

        // Keywords
        "keyword" => (4, 0),
        "keyword.control" => (4, 0b100000), // control modifier

        // Functions/Instructions
        "function" => (5, 0),
        "function.definition" => (5, 0b01), // definition modifier
        "function.instruction" => (5, 0b100), // instruction modifier

        // Variables
        "variable" => (6, 0),
        "variable.definition" => (6, 0b01), // definition modifier
        "variable.parameter" => (6, 0b1000), // parameter modifier
        "variable.local" => (6, 0b10000),   // local modifier

        // Default to keyword for unknown captures
        _ => (4, 0),
    }
}

impl Default for WatLSP {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions

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

fn find_index_definition(
    index: usize,
    symbols: &SymbolTable,
    document: &str,
    position: Position,
) -> Option<Range> {
    // Determine context from line
    let lines: Vec<&str> = document.lines().collect();
    let line = lines.get(position.line as usize)?;

    let context = if line.contains("call") {
        IndexContext::Function
    } else if line.contains("global") {
        IndexContext::Global
    } else if line.contains("local") {
        IndexContext::Local
    } else if line.contains("type") || line.contains("struct") || line.contains("array") {
        IndexContext::Type
    } else if line.contains("table") {
        IndexContext::Table
    } else if line.contains("memory") {
        IndexContext::Memory
    } else {
        return None;
    };

    crate::symbol_lookup::find_index_definition_range(index, symbols, context, position)
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
        if let Some(def_range) = find_symbol_definition_range(word, symbols, Position::new(0, 0)) {
            refs.retain(|r| {
                r.start.line != def_range.start.line
                    || r.start.character != def_range.start.character
            });
        }
    }

    refs
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
