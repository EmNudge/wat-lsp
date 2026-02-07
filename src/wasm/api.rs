//! wasm-bindgen API for browser usage.
//!
//! Provides the WatLSP class that can be used from JavaScript.

use wasm_bindgen::prelude::*;

use crate::completion::provide_completion;
use crate::core::types::{
    CompletionItem, CompletionItemKind, Diagnostic as CoreDiagnostic, HoverResult,
    InsertTextFormat, Position, Range,
};
use crate::diagnostics_core::track_stack_in_instr_list as core_track_stack_in_instr_list;
use crate::folding::{provide_folding_ranges, FoldingRangeKind};
use crate::hover::provide_hover_core;
use crate::parser::parse_document_from_tree;
use crate::symbol_lookup::{find_symbol_definition_range, IndexContext};
use crate::symbols::{SymbolTable, ValueType};
use crate::ts_facade::{self, Language, Parser, Query, Tree};
use crate::utils::{
    format_function_signature, get_line_at_position, get_word_at_position, is_word_char,
    node_at_position,
};

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

    /// Get diagnostics (syntax and semantic errors) for the current document
    #[wasm_bindgen(js_name = provideDiagnostics)]
    pub fn provide_diagnostics(&self) -> JsValue {
        let js_array = js_sys::Array::new();

        // Add tree-sitter based syntax diagnostics and semantic diagnostics
        if let (Some(tree), Some(symbols)) = (&self.tree, &self.symbols) {
            // Add syntax errors from tree-sitter ERROR nodes
            let syntax_diagnostics = provide_tree_sitter_diagnostics(tree, &self.document);
            for diag in syntax_diagnostics {
                js_array.push(&diagnostic_to_js(&diag));
            }

            // Add semantic diagnostics
            let semantic_diagnostics =
                provide_wasm_semantic_diagnostics(tree, &self.document, symbols);
            for diag in semantic_diagnostics {
                js_array.push(&diagnostic_to_js(&diag));
            }
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

    /// Prepare rename: check if position has a renamable symbol and return its range
    /// Returns null if the symbol cannot be renamed, otherwise returns { range, placeholder }
    #[wasm_bindgen(js_name = prepareRename)]
    pub fn prepare_rename(&self, line: u32, col: u32) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return JsValue::NULL,
        };

        let tree = match &self.tree {
            Some(t) => t,
            None => return JsValue::NULL,
        };

        let position = Position::new(line, col);

        // Get word at position
        let word = match get_word_at_position(&self.document, position) {
            Some(w) => w,
            None => return JsValue::NULL,
        };

        // Only named symbols (starting with $) can be renamed
        if !word.starts_with('$') {
            return JsValue::NULL;
        }

        // Check if this is a valid symbol that exists in the symbol table
        let is_valid_symbol = symbols.get_function_by_name(&word).is_some()
            || symbols.get_global_by_name(&word).is_some()
            || symbols.get_table_by_name(&word).is_some()
            || symbols.get_memory_by_name(&word).is_some()
            || symbols.get_type_by_name(&word).is_some()
            || symbols.get_tag_by_name(&word).is_some()
            || symbols.get_data_by_name(&word).is_some()
            || symbols.get_elem_by_name(&word).is_some()
            || symbols
                .find_function_containing_line(line)
                .map(|f| {
                    f.parameters
                        .iter()
                        .any(|p| p.name.as_deref() == Some(&word))
                        || f.locals.iter().any(|l| l.name.as_deref() == Some(&word))
                })
                .unwrap_or(false)
            || symbols
                .find_function_containing_line(line)
                .map(|f| f.blocks.iter().any(|b| b.label == word))
                .unwrap_or(false);

        if !is_valid_symbol {
            return JsValue::NULL;
        }

        // Find the identifier node at position to get its exact range
        if let Some(node) = crate::utils::node_at_position(tree, &self.document, position) {
            if node.kind() == "identifier" {
                let range = Range::from_coords(
                    node.start_position().row as u32,
                    node.start_position().column as u32,
                    node.end_position().row as u32,
                    node.end_position().column as u32,
                );

                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &"range".into(), &range_to_js(&range)).ok();
                js_sys::Reflect::set(&obj, &"placeholder".into(), &word.into()).ok();
                return obj.into();
            }
        }

        JsValue::NULL
    }

    /// Rename a symbol at the given position to a new name
    /// Returns null if rename is not possible, otherwise returns { changes: [{ range, newText }] }
    #[wasm_bindgen(js_name = rename)]
    pub fn rename(&self, line: u32, col: u32, new_name: &str) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return JsValue::NULL,
        };

        // Validation: New name MUST start with $
        if !new_name.starts_with('$') {
            let error = js_sys::Object::new();
            js_sys::Reflect::set(
                &error,
                &"error".into(),
                &format!("Invalid name '{}': symbols must start with '$'", new_name).into(),
            )
            .ok();
            return error.into();
        }

        let position = Position::new(line, col);

        // Get word at position
        let word = match get_word_at_position(&self.document, position) {
            Some(w) => w,
            None => return JsValue::NULL,
        };

        // Only named symbols can be renamed
        if !word.starts_with('$') {
            return JsValue::NULL;
        }

        // Find all references (including declaration)
        let refs = find_symbol_references(&word, symbols, &self.document, true);

        if refs.is_empty() {
            return JsValue::NULL;
        }

        // Create workspace edit with text edits
        let edits = js_sys::Array::new();
        for range in refs {
            let edit = js_sys::Object::new();
            js_sys::Reflect::set(&edit, &"range".into(), &range_to_js(&range)).ok();
            js_sys::Reflect::set(&edit, &"newText".into(), &new_name.into()).ok();
            edits.push(&edit);
        }

        let result = js_sys::Object::new();
        js_sys::Reflect::set(&result, &"changes".into(), &edits).ok();
        result.into()
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

    /// Provide document symbols (outline) for the current document
    /// Returns a hierarchical array of symbols matching the LSP DocumentSymbol structure
    #[wasm_bindgen(js_name = provideDocumentSymbols)]
    pub fn provide_document_symbols(&self) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return js_sys::Array::new().into(),
        };

        let result = js_sys::Array::new();

        // Add types
        for type_def in &symbols.types {
            result.push(&type_to_js_symbol(type_def));
        }

        // Add functions with children
        for func in &symbols.functions {
            result.push(&function_to_js_symbol(func));
        }

        // Add globals
        for global in &symbols.globals {
            result.push(&global_to_js_symbol(global));
        }

        // Add tables
        for table in &symbols.tables {
            result.push(&table_to_js_symbol(table));
        }

        // Add memories
        for memory in &symbols.memories {
            result.push(&memory_to_js_symbol(memory));
        }

        // Add tags
        for tag in &symbols.tags {
            result.push(&tag_to_js_symbol(tag));
        }

        // Add data segments
        for data in &symbols.data_segments {
            result.push(&data_to_js_symbol(data));
        }

        // Add element segments
        for elem in &symbols.elem_segments {
            result.push(&elem_to_js_symbol(elem));
        }

        result.into()
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

    /// Provide folding ranges for the current document
    /// Returns an array of folding range objects with startLine, endLine, and kind
    #[wasm_bindgen(js_name = provideFoldingRanges)]
    pub fn provide_folding_ranges(&self) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return js_sys::Array::new().into(),
        };

        let tree = match &self.tree {
            Some(t) => t,
            None => return js_sys::Array::new().into(),
        };

        let ranges = provide_folding_ranges(&self.document, symbols, tree);

        let js_array = js_sys::Array::new();
        for range in ranges {
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &"startLine".into(), &range.start_line.into()).ok();
            js_sys::Reflect::set(&obj, &"endLine".into(), &range.end_line.into()).ok();
            js_sys::Reflect::set(
                &obj,
                &"kind".into(),
                &match range.kind {
                    FoldingRangeKind::Region => "region",
                    FoldingRangeKind::Comment => "comment",
                }
                .into(),
            )
            .ok();
            js_array.push(&obj);
        }

        js_array.into()
    }

    /// Provide code completion items at the given position
    /// Returns an array of completion item objects
    #[wasm_bindgen(js_name = provideCompletion)]
    pub fn provide_completion(&self, line: u32, col: u32) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return js_sys::Array::new().into(),
        };

        let position = Position::new(line, col);
        let completions = provide_completion(&self.document, symbols, position);

        let js_array = js_sys::Array::new();
        for item in completions {
            js_array.push(&completion_item_to_js(&item));
        }

        js_array.into()
    }

    /// Provide signature help at the given position
    /// Returns null if no signature help available, otherwise returns:
    /// { signatures: [{ label, documentation?, parameters: [{ label, documentation? }] }],
    ///   activeSignature, activeParameter }
    #[wasm_bindgen(js_name = provideSignatureHelp)]
    pub fn provide_signature_help(&self, line: u32, col: u32) -> JsValue {
        let symbols = match &self.symbols {
            Some(s) => s,
            None => return JsValue::NULL,
        };

        let tree = match &self.tree {
            Some(t) => t,
            None => return JsValue::NULL,
        };

        let position = Position::new(line, col);

        // Try AST-based approach first
        let call_info = if let Some(node) = node_at_position(tree, &self.document, position) {
            find_function_call_ast(&node, &self.document)
        } else {
            None
        };

        // Fall back to string-based approach for incomplete code
        let call_info = call_info.or_else(|| {
            let line_text = get_line_at_position(&self.document, line as usize)?;
            let line_prefix = &line_text[..col.min(line_text.len() as u32) as usize];
            find_function_call(line_prefix)
        });

        let call_info = match call_info {
            Some(info) => info,
            None => return JsValue::NULL,
        };

        match call_info.call_type {
            CallType::Direct => provide_direct_call_signature_js(symbols, &call_info),
            CallType::CallRef | CallType::ReturnCallRef => {
                provide_call_ref_signature_js(symbols, &call_info)
            }
        }
    }
}

// ============================================================================
// Signature Help helpers
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
enum CallType {
    Direct,        // call $func
    CallRef,       // call_ref $type
    ReturnCallRef, // return_call_ref $type
}

struct CallInfo {
    name: String,
    arg_text: String,
    call_type: CallType,
}

/// Find function call using AST analysis
fn find_function_call_ast(node: &crate::ts_facade::Node, document: &str) -> Option<CallInfo> {
    // Walk up the tree to find a call instruction
    let mut current = node.clone();

    loop {
        let kind = current.kind();

        // Check if this is a call instruction
        if kind == "instr_plain" || kind == "expr1_plain" {
            let instr_text = &document[current.byte_range()];

            // Determine the call type based on the instruction text
            let call_type = if instr_text.contains("return_call_ref ") {
                Some(CallType::ReturnCallRef)
            } else if instr_text.contains("call_ref ") {
                Some(CallType::CallRef)
            } else if instr_text.contains("call ") && !instr_text.contains("call_") {
                Some(CallType::Direct)
            } else {
                None
            };

            if let Some(call_type) = call_type {
                // Extract the function/type name from the call instruction
                let mut name = None;
                let mut arg_count = 0;

                // Iterate through children to find the function name and count arguments
                let mut cursor = current.walk();
                for child in current.children(&mut cursor) {
                    let child_kind = child.kind();

                    // The function/type name is in an identifier or index node
                    if child_kind == "index" || child_kind == "identifier" {
                        if name.is_none() {
                            // First identifier/index after the operator is the function/type name
                            name = Some(&document[child.byte_range()]);
                        } else {
                            // Subsequent identifiers/indices are arguments
                            arg_count += 1;
                        }
                    }
                }

                if let Some(func_name) = name {
                    // Create arg_text with appropriate number of commas
                    let arg_text = if arg_count > 0 {
                        vec![","; arg_count - 1].join("")
                    } else {
                        String::new()
                    };

                    return Some(CallInfo {
                        name: func_name.to_string(),
                        arg_text,
                        call_type,
                    });
                }
            }
        }

        // Move to parent
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    None
}

/// Find function call using string-based analysis (fallback for incomplete code)
fn find_function_call(line_prefix: &str) -> Option<CallInfo> {
    let mut depth = 0;
    let mut paren_pos: Option<usize> = None;

    let chars: Vec<char> = line_prefix.chars().collect();

    // Scan backwards to find the call instruction
    for i in (0..chars.len()).rev() {
        match chars[i] {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(i);
                    break;
                } else {
                    depth -= 1;
                }
            }
            _ => {}
        }
    }

    let paren_pos = paren_pos?;

    // Now look backwards from paren to find call keyword and function/type name
    let before_paren = &line_prefix[..paren_pos];
    let call_pattern = before_paren.trim_end();

    // Try to find return_call_ref first (most specific)
    if let Some(call_idx) = call_pattern.rfind("return_call_ref ") {
        let after_call = call_pattern[call_idx + 16..].trim_start();
        if let Some(name) = extract_name_from_call(after_call) {
            let arg_text = line_prefix[paren_pos + 1..].to_string();
            return Some(CallInfo {
                name,
                arg_text,
                call_type: CallType::ReturnCallRef,
            });
        }
    }

    // Try call_ref next
    if let Some(call_idx) = call_pattern.rfind("call_ref ") {
        let after_call = call_pattern[call_idx + 9..].trim_start();
        if let Some(name) = extract_name_from_call(after_call) {
            let arg_text = line_prefix[paren_pos + 1..].to_string();
            return Some(CallInfo {
                name,
                arg_text,
                call_type: CallType::CallRef,
            });
        }
    }

    // Finally try regular call (but not call_indirect or call_ref)
    if let Some(call_idx) = call_pattern.rfind("call ") {
        let before_call = &call_pattern[..call_idx];
        if !before_call.ends_with("return_") && !before_call.ends_with('_') {
            let after_call = call_pattern[call_idx + 5..].trim_start();
            if let Some(name) = extract_name_from_call(after_call) {
                let arg_text = line_prefix[paren_pos + 1..].to_string();
                return Some(CallInfo {
                    name,
                    arg_text,
                    call_type: CallType::Direct,
                });
            }
        }
    }

    None
}

/// Extract the function/type name from text after a call keyword
fn extract_name_from_call(after_call: &str) -> Option<String> {
    let name_end = after_call
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(after_call.len());

    let name = after_call[..name_end].to_string();
    if !name.is_empty() {
        Some(name)
    } else {
        None
    }
}

/// Provide signature help for direct function calls (call $func)
fn provide_direct_call_signature_js(symbols: &SymbolTable, call_info: &CallInfo) -> JsValue {
    // Look up the function in the symbol table
    let func = if call_info.name.starts_with('$') {
        symbols.get_function_by_name(&call_info.name)
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_function_by_index(index)
    } else {
        None
    };

    let func = match func {
        Some(f) => f,
        None => return JsValue::NULL,
    };

    // Build signature information
    let label = format_function_signature(func);

    let parameters = js_sys::Array::new();
    for param in &func.parameters {
        let param_label = if let Some(ref name) = param.name {
            format!("({} {})", name, param.param_type)
        } else {
            format!("(param {})", param.param_type)
        };

        let param_obj = js_sys::Object::new();
        js_sys::Reflect::set(&param_obj, &"label".into(), &param_label.into()).ok();
        parameters.push(&param_obj);
    }

    // Determine which parameter we're currently on based on comma count
    let active_parameter = call_info.arg_text.matches(',').count() as u32;
    let clamped_active = active_parameter.min(func.parameters.len() as u32);

    // Build signature object
    let sig_obj = js_sys::Object::new();
    js_sys::Reflect::set(&sig_obj, &"label".into(), &label.into()).ok();
    js_sys::Reflect::set(&sig_obj, &"parameters".into(), &parameters).ok();
    js_sys::Reflect::set(&sig_obj, &"activeParameter".into(), &clamped_active.into()).ok();

    // Add documentation if function has doc comment
    if let Some(ref doc) = func.doc_comment {
        js_sys::Reflect::set(&sig_obj, &"documentation".into(), &doc.clone().into()).ok();
    }

    let signatures = js_sys::Array::new();
    signatures.push(&sig_obj);

    // Build result object
    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"signatures".into(), &signatures).ok();
    js_sys::Reflect::set(&result, &"activeSignature".into(), &0u32.into()).ok();
    js_sys::Reflect::set(&result, &"activeParameter".into(), &clamped_active.into()).ok();

    result.into()
}

/// Provide signature help for indirect calls via typed function references (call_ref $type)
fn provide_call_ref_signature_js(symbols: &SymbolTable, call_info: &CallInfo) -> JsValue {
    use crate::symbols::TypeKind;

    // Look up the type in the symbol table
    let type_def = if call_info.name.starts_with('$') {
        symbols.get_type_by_name(&call_info.name)
    } else if let Ok(index) = call_info.name.parse::<usize>() {
        symbols.get_type_by_index(index)
    } else {
        None
    };

    let type_def = match type_def {
        Some(t) => t,
        None => return JsValue::NULL,
    };

    // The type must be a function type
    let (params, results) = match &type_def.kind {
        TypeKind::Func { params, results } => (params, results),
        _ => return JsValue::NULL, // Not a function type
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
    let parameters = js_sys::Array::new();
    for (i, param_type) in params.iter().enumerate() {
        let param_obj = js_sys::Object::new();
        let param_label = format!("(param{} {})", i, param_type);
        js_sys::Reflect::set(&param_obj, &"label".into(), &param_label.into()).ok();
        parameters.push(&param_obj);
    }

    // The last argument is always the funcref
    let funcref_param = js_sys::Object::new();
    js_sys::Reflect::set(&funcref_param, &"label".into(), &"(funcref)".into()).ok();
    js_sys::Reflect::set(
        &funcref_param,
        &"documentation".into(),
        &"Function reference to call".into(),
    )
    .ok();
    parameters.push(&funcref_param);

    // Determine which parameter we're currently on
    let active_parameter = call_info.arg_text.matches(',').count() as u32;
    let param_count = (params.len() + 1) as u32; // +1 for funcref
    let clamped_active = active_parameter.min(param_count);

    // Build signature object
    let sig_obj = js_sys::Object::new();
    js_sys::Reflect::set(&sig_obj, &"label".into(), &label.into()).ok();
    js_sys::Reflect::set(&sig_obj, &"parameters".into(), &parameters).ok();
    js_sys::Reflect::set(&sig_obj, &"activeParameter".into(), &clamped_active.into()).ok();

    let doc = format!(
        "Indirect call via typed function reference. The last argument must be a function reference of type {}.",
        type_name
    );
    js_sys::Reflect::set(&sig_obj, &"documentation".into(), &doc.into()).ok();

    let signatures = js_sys::Array::new();
    signatures.push(&sig_obj);

    // Build result object
    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"signatures".into(), &signatures).ok();
    js_sys::Reflect::set(&result, &"activeSignature".into(), &0u32.into()).ok();
    js_sys::Reflect::set(&result, &"activeParameter".into(), &clamped_active.into()).ok();

    result.into()
}

/// Convert a completion item to a JavaScript object
fn completion_item_to_js(item: &CompletionItem) -> JsValue {
    let obj = js_sys::Object::new();

    js_sys::Reflect::set(&obj, &"label".into(), &item.label.clone().into()).ok();

    if let Some(kind) = &item.kind {
        // Map to Monaco CompletionItemKind values
        let kind_value = match kind {
            CompletionItemKind::Text => 1,
            CompletionItemKind::Method => 0,
            CompletionItemKind::Function => 1,
            CompletionItemKind::Constructor => 2,
            CompletionItemKind::Field => 3,
            CompletionItemKind::Variable => 4,
            CompletionItemKind::Class => 5,
            CompletionItemKind::Interface => 7,
            CompletionItemKind::Module => 8,
            CompletionItemKind::Property => 9,
            CompletionItemKind::Unit => 10,
            CompletionItemKind::Value => 11,
            CompletionItemKind::Enum => 12,
            CompletionItemKind::Keyword => 13,
            CompletionItemKind::Snippet => 14,
            CompletionItemKind::Color => 15,
            CompletionItemKind::File => 16,
            CompletionItemKind::Reference => 17,
            CompletionItemKind::Folder => 18,
            CompletionItemKind::EnumMember => 19,
            CompletionItemKind::Constant => 20,
            CompletionItemKind::Struct => 21,
            CompletionItemKind::Event => 22,
            CompletionItemKind::Operator => 23,
            CompletionItemKind::TypeParameter => 24,
        };
        js_sys::Reflect::set(&obj, &"kind".into(), &kind_value.into()).ok();
    }

    if let Some(detail) = &item.detail {
        js_sys::Reflect::set(&obj, &"detail".into(), &detail.clone().into()).ok();
    }

    if let Some(insert_text) = &item.insert_text {
        js_sys::Reflect::set(&obj, &"insertText".into(), &insert_text.clone().into()).ok();
    }

    if let Some(format) = &item.insert_text_format {
        // 1 = PlainText, 2 = Snippet (matches Monaco InsertTextRule.InsertAsSnippet)
        let rules = match format {
            InsertTextFormat::PlainText => 0,
            InsertTextFormat::Snippet => 4, // Monaco InsertTextRule.InsertAsSnippet
        };
        js_sys::Reflect::set(&obj, &"insertTextRules".into(), &rules.into()).ok();
    }

    if let Some(doc) = &item.documentation {
        js_sys::Reflect::set(&obj, &"documentation".into(), &doc.clone().into()).ok();
    }

    obj.into()
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
    end_line: u32,
    end_character: u32,
    message: String,
    severity: u32, // 1 = error, 2 = warning, 3 = info, 4 = hint
}

impl From<CoreDiagnostic> for WasmDiagnostic {
    fn from(diag: CoreDiagnostic) -> Self {
        WasmDiagnostic {
            line: diag.range.start.line,
            character: diag.range.start.character,
            end_line: diag.range.end.line,
            end_character: diag.range.end.character,
            message: diag.message,
            severity: match diag.severity {
                crate::core::types::DiagnosticSeverity::Error => 1,
                crate::core::types::DiagnosticSeverity::Warning => 2,
                crate::core::types::DiagnosticSeverity::Information => 3,
                crate::core::types::DiagnosticSeverity::Hint => 4,
            },
        }
    }
}

use crate::instruction_metadata::{
    get_instruction_arity_map, infer_simd_instruction_arity, OperandMode,
};
use crate::ts_facade::Node;
use crate::utils::{
    determine_instruction_context_at_node, find_containing_function, InstructionContext, STRUCT_OPS,
};

/// Provide semantic diagnostics for undefined references (WASM version)
fn provide_wasm_semantic_diagnostics(
    tree: &crate::ts_facade::Tree,
    source: &str,
    symbols: &SymbolTable,
) -> Vec<WasmDiagnostic> {
    let mut diagnostics = Vec::new();
    walk_tree_for_diagnostics(tree.root_node(), source, symbols, &mut diagnostics);
    diagnostics
}

fn walk_tree_for_diagnostics(
    node: Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<WasmDiagnostic>,
) {
    let kind = node.kind();

    // Check for function body instruction lists - perform stack tracking for unfolded code
    if kind == "module_field_func" {
        // Get the function's declared return types
        let func_line = node.start_position().row as u32;
        let expected_results: &[ValueType] = symbols
            .find_function_containing_line(func_line)
            .map(|f| f.results.as_slice())
            .unwrap_or(&[]);

        // Check if function uses (type $ref) syntax - if so, the symbol table might not
        // have resolved the return types from the type definition
        let mut cursor = node.walk();
        let has_type_use = node.children(&mut cursor).any(|c| c.kind() == "type_use");

        // For return type validation: use Some(results) when we know them, None otherwise
        let expected_for_validation = if has_type_use && expected_results.is_empty() {
            // Function uses type reference but symbol table didn't resolve it - skip validation
            None
        } else {
            Some(expected_results)
        };

        // Find the instr_list child and track stack using shared core
        let mut cursor = node.walk();
        let mut found_instr_list = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "instr_list" {
                // Use shared core function and convert diagnostics
                let core_diagnostics = core_track_stack_in_instr_list(
                    &child,
                    source,
                    symbols,
                    expected_for_validation,
                );
                for diag in core_diagnostics {
                    diagnostics.push(diag.into());
                }
                found_instr_list = true;
                break;
            }
        }

        // If no instr_list but function expects return values, report error
        // Skip this check if function uses type_use (we don't know the expected results)
        if !found_instr_list && !expected_results.is_empty() && !has_type_use {
            diagnostics.push(WasmDiagnostic {
                line: node.start_position().row as u32,
                character: node.start_position().column as u32,
                end_line: node.end_position().row as u32,
                end_character: node.end_position().column as u32,
                message: format!(
                    "Type mismatch: function body leaves 0 values on stack but signature requires {}",
                    expected_results.len()
                ),
                severity: 1,
            });
        }
        // Continue recursing for other checks (references, etc.)
    }

    // Special handling for catch clauses - they have tag and/or label references
    if kind == "catch_clause" {
        check_catch_clause_references(&node, source, symbols, diagnostics);
        return;
    }

    // Special handling for legacy try catch clause
    if kind == "try_catch_clause" {
        check_try_catch_clause_references(&node, source, symbols, diagnostics);
        // Continue recursing to check nested instructions
    }

    // Check folded expression operand count (expr1_plain nodes)
    if kind == "expr1_plain" {
        check_folded_expr_operand_count(&node, source, symbols, diagnostics);
    }

    // Check for undefined references based on instruction context
    let context = determine_instruction_context_at_node(&node, source);
    if context != InstructionContext::General {
        check_undefined_references(&node, source, symbols, diagnostics, &context);

        // Only recurse into nested expr nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "expr" {
                walk_tree_for_diagnostics(child, source, symbols, diagnostics);
            }
        }
        return;
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_diagnostics(child, source, symbols, diagnostics);
    }
}

fn check_undefined_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<WasmDiagnostic>,
    context: &InstructionContext,
) {
    let text = &source[node.byte_range()];
    let first_token = text.split_whitespace().next().unwrap_or("");

    // For struct.get/struct.set, only the first index is a type reference
    // The second index is a field reference which we don't validate
    if *context == InstructionContext::Type && STRUCT_OPS.contains(&first_token) {
        // Only validate the first index child
        find_first_index_identifier(node, source, symbols, diagnostics, context);
        return;
    }

    find_undefined_identifiers(node, source, symbols, diagnostics, context);
}

/// Find and validate only the first index identifier in a node
/// Used for instructions like struct.get where only the first index is a type
fn find_first_index_identifier(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<WasmDiagnostic>,
    context: &InstructionContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "index" {
            // Found the first index, check its identifier
            find_undefined_identifiers(&child, source, symbols, diagnostics, context);
            return; // Only check the first one
        }
        // Recurse into instr_plain to find the index
        if child.kind() == "instr_plain" {
            find_first_index_identifier(&child, source, symbols, diagnostics, context);
            return;
        }
    }
}

/// Check operand count in folded expressions (expr1_plain nodes)
fn check_folded_expr_operand_count(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<WasmDiagnostic>,
) {
    // Get children
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    // First child should be instr_plain
    let first_child = match children.first() {
        Some(c) if c.kind() == "instr_plain" => c,
        _ => return,
    };

    // Get the instruction name from instr_plain
    let mut instr_cursor = first_child.walk();
    let instr_children: Vec<_> = first_child.children(&mut instr_cursor).collect();
    if instr_children.is_empty() {
        return;
    }

    let instr_kind = instr_children[0].kind();
    let instr_name = match instr_kind.as_str() {
        "op_const" => {
            let mut const_cursor = instr_children[0].walk();
            let const_children: Vec<_> = instr_children[0].children(&mut const_cursor).collect();
            if const_children.is_empty() {
                return;
            }
            source[const_children[0].byte_range()].trim().to_string()
        }
        _ => source[instr_children[0].byte_range()].trim().to_string(),
    };

    // Count expr children (these are the operands)
    let operand_count = children
        .iter()
        .skip(1)
        .filter(|c| c.kind() == "expr")
        .count();

    // Validate operand count using the arity map, with SIMD pattern fallback
    let arity_map = get_instruction_arity_map();
    let resolved = if let Some(arity) = arity_map.get(instr_name.as_str()) {
        Some((arity.operand_mode, true))
    } else {
        infer_simd_instruction_arity(&instr_name).map(|(c, _)| (OperandMode::Fixed(c), false))
    };
    if let Some((operand_mode, check_dynamic)) = resolved {
        match operand_mode {
            OperandMode::Fixed(expected) => {
                if operand_count > expected {
                    // Error: too many operands
                    let start_point = node.start_position();
                    let end_point = node.end_position();
                    diagnostics.push(WasmDiagnostic {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                        end_line: end_point.row as u32,
                        end_character: end_point.column as u32,
                        message: format!(
                            "Instruction '{}' expects at most {} operands, but got {}",
                            instr_name, expected, operand_count
                        ),
                        severity: 1,
                    });
                } else if operand_count > 0 && operand_count < expected {
                    // Check if we're in a nested context where stack values aren't available
                    if is_nested_in_expr(node) {
                        let start_point = node.start_position();
                        let end_point = node.end_position();
                        diagnostics.push(WasmDiagnostic {
                            line: start_point.row as u32,
                            character: start_point.column as u32,
                            end_line: end_point.row as u32,
                            end_character: end_point.column as u32,
                            message: format!(
                                "Instruction '{}' expects {} operands, but got {}",
                                instr_name, expected, operand_count
                            ),
                            severity: 1,
                        });
                    }
                }
            }
            OperandMode::Dynamic if check_dynamic => {
                // Dynamic operand count - check based on instruction type
                check_dynamic_operands(
                    node,
                    &instr_name,
                    operand_count,
                    symbols,
                    source,
                    diagnostics,
                );
            }
            _ => {}
        }
    }
}

/// Check if a node is nested inside another expr (meaning it can't use stack values)
fn is_nested_in_expr(node: &Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "expr" {
            // Found parent expr, check if its parent is also expr
            if let Some(grandparent) = parent.parent() {
                if grandparent.kind() == "expr" || grandparent.kind().starts_with("expr1_") {
                    return true;
                }
            }
        }
        current = parent.parent();
    }
    false
}

/// Check dynamic operand count for specific instructions
fn check_dynamic_operands(
    node: &Node,
    instr_name: &str,
    operand_count: usize,
    symbols: &SymbolTable,
    source: &str,
    diagnostics: &mut Vec<WasmDiagnostic>,
) {
    match instr_name {
        "call" => {
            // Get function reference and check param count
            if let Some(func_name) = extract_instruction_index(node, source) {
                if let Some(func) = symbols.get_function_by_name(&func_name) {
                    let expected = func.parameters.len();
                    validate_operand_count(node, instr_name, operand_count, expected, diagnostics);
                } else if let Ok(idx) = func_name.parse::<usize>() {
                    if let Some(func) = symbols.get_function_by_index(idx) {
                        let expected = func.parameters.len();
                        validate_operand_count(
                            node,
                            instr_name,
                            operand_count,
                            expected,
                            diagnostics,
                        );
                    }
                }
            }
        }
        "throw" => {
            // Get tag reference and check param count
            if let Some(tag_name) = extract_instruction_index(node, source) {
                if let Some(tag) = symbols.get_tag_by_name(&tag_name) {
                    let expected = tag.params.len();
                    validate_operand_count(node, instr_name, operand_count, expected, diagnostics);
                } else if let Ok(idx) = tag_name.parse::<usize>() {
                    if let Some(tag) = symbols.get_tag_by_index(idx) {
                        let expected = tag.params.len();
                        validate_operand_count(
                            node,
                            instr_name,
                            operand_count,
                            expected,
                            diagnostics,
                        );
                    }
                }
            }
        }
        _ => {
            // Skip other dynamic instructions for now
        }
    }
}

/// Helper to validate operand count and push diagnostic if invalid
fn validate_operand_count(
    node: &Node,
    instr_name: &str,
    actual: usize,
    expected: usize,
    diagnostics: &mut Vec<WasmDiagnostic>,
) {
    if actual > expected {
        let start_point = node.start_position();
        let end_point = node.end_position();
        diagnostics.push(WasmDiagnostic {
            line: start_point.row as u32,
            character: start_point.column as u32,
            end_line: end_point.row as u32,
            end_character: end_point.column as u32,
            message: format!(
                "Instruction '{}' expects at most {} operands, but got {}",
                instr_name, expected, actual
            ),
            severity: 1,
        });
    } else if actual > 0 && actual < expected && is_nested_in_expr(node) {
        let start_point = node.start_position();
        let end_point = node.end_position();
        diagnostics.push(WasmDiagnostic {
            line: start_point.row as u32,
            character: start_point.column as u32,
            end_line: end_point.row as u32,
            end_character: end_point.column as u32,
            message: format!(
                "Instruction '{}' expects {} operands, but got {}",
                instr_name, expected, actual
            ),
            severity: 1,
        });
    }
}

/// Extract the type/function/tag index from an instruction node
fn extract_instruction_index(node: &Node, source: &str) -> Option<String> {
    // Navigate to find instr_plain > index/identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "instr_plain" {
            let mut instr_cursor = child.walk();
            for instr_child in child.children(&mut instr_cursor) {
                if instr_child.kind() == "index" || instr_child.kind() == "identifier" {
                    return Some(source[instr_child.byte_range()].trim().to_string());
                }
                // Also check inside op_ nodes
                if instr_child.kind().starts_with("op_") {
                    let mut op_cursor = instr_child.walk();
                    for op_child in instr_child.children(&mut op_cursor) {
                        if op_child.kind() == "index" || op_child.kind() == "identifier" {
                            return Some(source[op_child.byte_range()].trim().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check references in a catch_clause node (try_table syntax)
/// For (catch $tag $label) and (catch_ref $tag $label): first index is tag, second is label
/// For (catch_all $label) and (catch_all_ref $label): single index is label
fn check_catch_clause_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<WasmDiagnostic>,
) {
    let text = &source[node.byte_range()];

    // Determine if this is catch/catch_ref (has tag) or catch_all/catch_all_ref (no tag)
    let has_tag =
        text.contains("catch_ref") || (text.contains("catch") && !text.contains("catch_all"));

    // Collect all index children
    let mut cursor = node.walk();
    let indices: Vec<_> = node
        .children(&mut cursor)
        .filter(|c| c.kind() == "index")
        .collect();

    for (idx, index_node) in indices.iter().enumerate() {
        // Find identifier within the index
        let mut idx_cursor = index_node.walk();
        for child in index_node.children(&mut idx_cursor) {
            if child.kind() == "identifier" {
                let identifier_name = &source[child.byte_range()];
                if !identifier_name.starts_with('$') {
                    continue;
                }

                let start_point = child.start_position();
                let end_point = child.end_position();
                let position = crate::core::types::Position::new(
                    start_point.row as u32,
                    start_point.column as u32,
                );

                // First index is tag (if has_tag), remaining are labels
                let is_tag_reference = has_tag && idx == 0;

                let is_defined = if is_tag_reference {
                    symbols.get_tag_by_name(identifier_name).is_some()
                } else {
                    // Label reference - check block labels in containing function
                    if let Some(func) = find_containing_function(symbols, position) {
                        func.blocks.iter().any(|block| {
                            format!("${}", block.label) == identifier_name
                                || block.label == identifier_name
                        })
                    } else {
                        false
                    }
                };

                if !is_defined {
                    let message = if is_tag_reference {
                        format!("Undefined tag '{}'", identifier_name)
                    } else {
                        format!("Undefined label '{}'", identifier_name)
                    };

                    diagnostics.push(WasmDiagnostic {
                        line: start_point.row as u32,
                        character: start_point.column as u32,
                        end_line: end_point.row as u32,
                        end_character: end_point.column as u32,
                        message,
                        severity: 1,
                    });
                }
            }
        }
    }
}

/// Check references in a try_catch_clause node (legacy try syntax)
/// For (catch $tag instr*): the index is a tag reference
fn check_try_catch_clause_references(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<WasmDiagnostic>,
) {
    // Find the index child which contains the tag reference
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "index" {
            // Find the identifier within the index
            let mut idx_cursor = child.walk();
            for idx_child in child.children(&mut idx_cursor) {
                if idx_child.kind() == "identifier" {
                    let identifier_name = &source[idx_child.byte_range()];
                    if !identifier_name.starts_with('$') {
                        continue;
                    }

                    let start_point = idx_child.start_position();
                    let end_point = idx_child.end_position();

                    // It's a tag reference
                    if symbols.get_tag_by_name(identifier_name).is_none() {
                        diagnostics.push(WasmDiagnostic {
                            line: start_point.row as u32,
                            character: start_point.column as u32,
                            end_line: end_point.row as u32,
                            end_character: end_point.column as u32,
                            message: format!("Undefined tag '{}'", identifier_name),
                            severity: 1,
                        });
                    }
                }
            }
            // Only one index in try_catch_clause
            break;
        }
    }
}

fn find_undefined_identifiers(
    node: &Node,
    source: &str,
    symbols: &SymbolTable,
    diagnostics: &mut Vec<WasmDiagnostic>,
    context: &InstructionContext,
) {
    if node.kind() == "identifier" {
        let identifier_name = &source[node.byte_range()];

        // Only check identifiers that start with $
        if !identifier_name.starts_with('$') {
            return;
        }

        let start_point = node.start_position();
        let end_point = node.end_position();
        let position =
            crate::core::types::Position::new(start_point.row as u32, start_point.column as u32);

        let is_defined = match context {
            InstructionContext::Branch | InstructionContext::Block => {
                if let Some(func) = find_containing_function(symbols, position) {
                    func.blocks.iter().any(|block| {
                        format!("${}", block.label) == identifier_name
                            || block.label == identifier_name
                    })
                } else {
                    false
                }
            }
            InstructionContext::Call => symbols.get_function_by_name(identifier_name).is_some(),
            InstructionContext::Local => {
                if let Some(func) = find_containing_function(symbols, position) {
                    func.parameters
                        .iter()
                        .any(|p| p.name.as_ref() == Some(&identifier_name.to_string()))
                        || func
                            .locals
                            .iter()
                            .any(|l| l.name.as_ref() == Some(&identifier_name.to_string()))
                } else {
                    false
                }
            }
            InstructionContext::Global => symbols.get_global_by_name(identifier_name).is_some(),
            InstructionContext::Table => symbols.get_table_by_name(identifier_name).is_some(),
            InstructionContext::Memory => symbols.get_memory_by_name(identifier_name).is_some(),
            InstructionContext::Type => symbols.get_type_by_name(identifier_name).is_some(),
            InstructionContext::Tag => symbols.get_tag_by_name(identifier_name).is_some(),
            InstructionContext::Data => symbols.get_data_by_name(identifier_name).is_some(),
            InstructionContext::Elem => symbols.get_elem_by_name(identifier_name).is_some(),
            InstructionContext::Function | InstructionContext::General => true,
        };

        if !is_defined {
            let message = match context {
                InstructionContext::Branch | InstructionContext::Block => {
                    format!("Undefined label '{}'", identifier_name)
                }
                InstructionContext::Call => format!("Undefined function '{}'", identifier_name),
                InstructionContext::Local => {
                    format!("Undefined local or parameter '{}'", identifier_name)
                }
                InstructionContext::Global => format!("Undefined global '{}'", identifier_name),
                InstructionContext::Table => format!("Undefined table '{}'", identifier_name),
                InstructionContext::Memory => format!("Undefined memory '{}'", identifier_name),
                InstructionContext::Type => format!("Undefined type '{}'", identifier_name),
                InstructionContext::Tag => format!("Undefined tag '{}'", identifier_name),
                InstructionContext::Data => format!("Undefined data segment '{}'", identifier_name),
                InstructionContext::Elem => format!("Undefined elem segment '{}'", identifier_name),
                _ => format!("Undefined reference '{}'", identifier_name),
            };

            diagnostics.push(WasmDiagnostic {
                line: start_point.row as u32,
                character: start_point.column as u32,
                end_line: end_point.row as u32,
                end_character: end_point.column as u32,
                message,
                severity: 1, // Error
            });
        }
        return;
    }

    // Don't recurse into nested expr nodes
    if node.kind() == "expr" {
        return;
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_undefined_identifiers(&child, source, symbols, diagnostics, context);
    }
}

// ============================================================================

/// Provide tree-sitter based syntax diagnostics (WASM version)
fn provide_tree_sitter_diagnostics(
    tree: &crate::ts_facade::Tree,
    source: &str,
) -> Vec<WasmDiagnostic> {
    let mut diagnostics = Vec::new();
    walk_tree_for_syntax_errors(tree.root_node(), source, &mut diagnostics);
    diagnostics
}

/// Recursively walk the tree and collect ERROR nodes as diagnostics
fn walk_tree_for_syntax_errors(node: Node, source: &str, diagnostics: &mut Vec<WasmDiagnostic>) {
    if node.kind() == "ERROR" {
        let start_point = node.start_position();
        let end_point = node.end_position();
        let text = &source[node.byte_range()];

        let message = if text.trim().is_empty() {
            "Syntax error: unexpected token".to_string()
        } else {
            format!("Syntax error near: {}", text.lines().next().unwrap_or(text))
        };

        diagnostics.push(WasmDiagnostic {
            line: start_point.row as u32,
            character: start_point.column as u32,
            end_line: end_point.row as u32,
            end_character: end_point.column as u32,
            message,
            severity: 1, // Error
        });
    }

    // Check for MISSING nodes as well (incomplete syntax)
    if node.is_missing() {
        let start_point = node.start_position();
        let end_point = node.end_position();

        diagnostics.push(WasmDiagnostic {
            line: start_point.row as u32,
            character: start_point.column as u32,
            end_line: end_point.row as u32,
            end_character: end_point.column as u32,
            message: format!("Missing {}", node.kind()),
            severity: 1, // Error
        });
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_syntax_errors(child, source, diagnostics);
    }
}

fn diagnostic_to_js(diag: &WasmDiagnostic) -> JsValue {
    let obj = js_sys::Object::new();

    let range = js_sys::Object::new();
    let start = js_sys::Object::new();
    js_sys::Reflect::set(&start, &"line".into(), &diag.line.into()).ok();
    js_sys::Reflect::set(&start, &"character".into(), &diag.character.into()).ok();
    let end = js_sys::Object::new();
    js_sys::Reflect::set(&end, &"line".into(), &diag.end_line.into()).ok();
    js_sys::Reflect::set(&end, &"character".into(), &diag.end_character.into()).ok();
    js_sys::Reflect::set(&range, &"start".into(), &start).ok();
    js_sys::Reflect::set(&range, &"end".into(), &end).ok();

    js_sys::Reflect::set(&obj, &"range".into(), &range).ok();
    js_sys::Reflect::set(&obj, &"message".into(), &diag.message.clone().into()).ok();
    js_sys::Reflect::set(&obj, &"severity".into(), &diag.severity.into()).ok();

    obj.into()
}

// ============================================================================
// Document Symbol helpers
// ============================================================================

use crate::symbols::{
    BlockLabel, DataSegment, ElemSegment, Function, Global, Memory, Parameter, Table, Tag, TypeDef,
    TypeKind, Variable,
};

/// LSP SymbolKind constants (matching the LSP spec)
mod symbol_kind {
    pub const FUNCTION: u32 = 12;
    pub const VARIABLE: u32 = 13;
    pub const CONSTANT: u32 = 14;
    pub const STRING: u32 = 15;
    pub const ARRAY: u32 = 18;
    pub const OBJECT: u32 = 19;
    pub const KEY: u32 = 20;
    pub const EVENT: u32 = 24;
    pub const INTERFACE: u32 = 11;
}

fn make_js_range(line: u32, character: u32, end_line: u32, end_character: u32) -> js_sys::Object {
    let range = js_sys::Object::new();
    let start = js_sys::Object::new();
    js_sys::Reflect::set(&start, &"line".into(), &line.into()).ok();
    js_sys::Reflect::set(&start, &"character".into(), &character.into()).ok();
    let end = js_sys::Object::new();
    js_sys::Reflect::set(&end, &"line".into(), &end_line.into()).ok();
    js_sys::Reflect::set(&end, &"character".into(), &end_character.into()).ok();
    js_sys::Reflect::set(&range, &"start".into(), &start).ok();
    js_sys::Reflect::set(&range, &"end".into(), &end).ok();
    range
}

fn core_range_to_js_range(range: &Range) -> js_sys::Object {
    make_js_range(
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character,
    )
}

fn create_document_symbol(
    name: &str,
    detail: Option<&str>,
    kind: u32,
    range: js_sys::Object,
    children: Option<js_sys::Array>,
) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"name".into(), &name.into()).ok();
    if let Some(d) = detail {
        js_sys::Reflect::set(&obj, &"detail".into(), &d.into()).ok();
    }
    js_sys::Reflect::set(&obj, &"kind".into(), &kind.into()).ok();
    js_sys::Reflect::set(&obj, &"range".into(), &range).ok();
    js_sys::Reflect::set(&obj, &"selectionRange".into(), &range).ok();
    if let Some(c) = children {
        js_sys::Reflect::set(&obj, &"children".into(), &c).ok();
    }
    obj.into()
}

fn type_to_js_symbol(type_def: &TypeDef) -> JsValue {
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
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(type_def.line, 0, type_def.line, 0));

    create_document_symbol(&name, Some(&detail), symbol_kind::INTERFACE, range, None)
}

fn function_to_js_symbol(func: &Function) -> JsValue {
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
        .as_ref()
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(func.line, 0, func.end_line, 0));

    // Build children
    let children = js_sys::Array::new();

    for param in &func.parameters {
        children.push(&param_to_js_symbol(param));
    }

    for local in &func.locals {
        children.push(&local_to_js_symbol(local));
    }

    for block in &func.blocks {
        children.push(&block_to_js_symbol(block));
    }

    let children_opt = if children.length() > 0 {
        Some(children)
    } else {
        None
    };

    create_document_symbol(
        &name,
        Some(&detail),
        symbol_kind::FUNCTION,
        range,
        children_opt,
    )
}

fn param_to_js_symbol(param: &Parameter) -> JsValue {
    let name = param
        .name
        .clone()
        .unwrap_or_else(|| format!("(param {})", param.index));

    let range = param
        .range
        .as_ref()
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(0, 0, 0, 0));

    create_document_symbol(
        &name,
        Some(&param.param_type.to_string()),
        symbol_kind::VARIABLE,
        range,
        None,
    )
}

fn local_to_js_symbol(local: &Variable) -> JsValue {
    let name = local
        .name
        .clone()
        .unwrap_or_else(|| format!("(local {})", local.index));

    let range = local
        .range
        .as_ref()
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(0, 0, 0, 0));

    create_document_symbol(
        &name,
        Some(&local.var_type.to_string()),
        symbol_kind::VARIABLE,
        range,
        None,
    )
}

fn block_to_js_symbol(block: &BlockLabel) -> JsValue {
    let range = block
        .range
        .as_ref()
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(block.line, 0, block.line, 0));

    create_document_symbol(
        &block.label,
        Some(&block.block_type),
        symbol_kind::KEY,
        range,
        None,
    )
}

fn global_to_js_symbol(global: &Global) -> JsValue {
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
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(global.line, 0, global.line, 0));

    create_document_symbol(&name, Some(&detail), symbol_kind::CONSTANT, range, None)
}

fn table_to_js_symbol(table: &Table) -> JsValue {
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
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(table.line, 0, table.line, 0));

    create_document_symbol(&name, Some(&detail), symbol_kind::ARRAY, range, None)
}

fn memory_to_js_symbol(memory: &Memory) -> JsValue {
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
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(memory.line, 0, memory.line, 0));

    create_document_symbol(&name, Some(&detail), symbol_kind::OBJECT, range, None)
}

fn tag_to_js_symbol(tag: &Tag) -> JsValue {
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
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(tag.line, 0, tag.line, 0));

    create_document_symbol(&name, Some(&detail), symbol_kind::EVENT, range, None)
}

fn data_to_js_symbol(data: &DataSegment) -> JsValue {
    let name = data
        .name
        .clone()
        .unwrap_or_else(|| format!("(data {})", data.index));

    let detail = format!("{} bytes", data.byte_length);

    let range = data
        .range
        .as_ref()
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(data.line, 0, data.line, 0));

    create_document_symbol(&name, Some(&detail), symbol_kind::STRING, range, None)
}

fn elem_to_js_symbol(elem: &ElemSegment) -> JsValue {
    let name = elem
        .name
        .clone()
        .unwrap_or_else(|| format!("(elem {})", elem.index));

    let detail = format!("{} functions", elem.func_names.len());

    let range = elem
        .range
        .as_ref()
        .map(core_range_to_js_range)
        .unwrap_or_else(|| make_js_range(elem.line, 0, elem.line, 0));

    create_document_symbol(&name, Some(&detail), symbol_kind::ARRAY, range, None)
}
