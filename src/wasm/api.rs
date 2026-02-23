//! wasm-bindgen API for browser usage.
//!
//! Provides the WatLSP class that can be used from JavaScript.

use wasm_bindgen::prelude::*;

use crate::completion::provide_completion;
use crate::core::types::{
    CompletionItem, CompletionItemKind, Diagnostic as CoreDiagnostic, HoverResult,
    InsertTextFormat, Position, Range,
};
use crate::diagnostics_core::tree_walk::{walk_tree_for_diagnostics, DiagnosticConfig};
use crate::folding::{provide_folding_ranges, FoldingRangeKind};
use crate::hover::provide_hover_core;
use crate::parser::parse_document_from_tree;
use crate::signature::call_info::{find_function_call, find_function_call_ast, CallType};
use crate::signature::signature_core::{
    provide_call_ref_signature_core, provide_direct_call_signature_core,
};
use crate::symbols::SymbolTable;
use crate::ts_facade::{self, Language, Parser, Query, Tree};
use crate::utils::{get_line_at_position, get_word_at_position, node_at_position};

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

impl WatLSP {
    fn symbols_and_tree(&self) -> Option<(&SymbolTable, &Tree)> {
        Some((self.symbols.as_ref()?, self.tree.as_ref()?))
    }
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

        if let Some((symbols, tree)) = self.symbols_and_tree() {
            // Syntax errors from tree-sitter ERROR nodes
            let syntax_diagnostics =
                crate::diagnostics_core::provide_tree_sitter_diagnostics(tree, &self.document);
            for diag in syntax_diagnostics {
                js_array.push(&core_diagnostic_to_js(&diag));
            }

            // Semantic diagnostics via shared tree walk
            let config = DiagnosticConfig::from_symbols(symbols);
            let mut diagnostics = Vec::new();
            walk_tree_for_diagnostics(
                tree.root_node(),
                &self.document,
                symbols,
                &config,
                &mut diagnostics,
            );

            // Subtype hierarchy + module-level structural validations
            diagnostics
                .extend(crate::diagnostics_core::subtype::validate_subtype_hierarchy(symbols));
            diagnostics.extend(
                crate::diagnostics_core::module_checks::validate_module_structure(
                    &tree.root_node(),
                    &self.document,
                    symbols,
                ),
            );

            for diag in diagnostics {
                js_array.push(&core_diagnostic_to_js(&diag));
            }
        }

        js_array.into()
    }

    /// Provide hover information at the given position (uses tree-sitter based hover)
    #[wasm_bindgen(js_name = provideHover)]
    pub fn provide_hover(&self, line: u32, col: u32) -> JsValue {
        let Some((symbols, tree)) = self.symbols_and_tree() else {
            return JsValue::NULL;
        };
        let position = Position::new(line, col);
        match provide_hover_core(&self.document, symbols, tree, position) {
            Some(hover) => hover_to_js(&hover),
            None => JsValue::NULL,
        }
    }

    /// Provide go-to-definition at the given position
    #[wasm_bindgen(js_name = provideDefinition)]
    pub fn provide_definition(&self, line: u32, col: u32) -> JsValue {
        let Some((symbols, tree)) = self.symbols_and_tree() else {
            return JsValue::NULL;
        };
        let position = Position::new(line, col);

        match crate::features::definition_core::provide_definition_core(
            &self.document,
            symbols,
            tree,
            position,
        ) {
            Some(range) => range_obj_to_js(&range),
            None => JsValue::NULL,
        }
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
        let Some((symbols, tree)) = self.symbols_and_tree() else {
            return js_sys::Array::new().into();
        };
        let position = Position::new(line, col);
        let refs = crate::features::references_core::provide_references_core(
            &self.document,
            symbols,
            tree,
            position,
            include_declaration,
        );
        let js_array = js_sys::Array::new();
        for range in refs {
            js_array.push(&range_obj_to_js(&range));
        }
        js_array.into()
    }

    /// Prepare rename: check if position has a renamable symbol and return its range
    /// Returns null if the symbol cannot be renamed, otherwise returns { range, placeholder }
    #[wasm_bindgen(js_name = prepareRename)]
    pub fn prepare_rename(&self, line: u32, col: u32) -> JsValue {
        let Some((symbols, tree)) = self.symbols_and_tree() else {
            return JsValue::NULL;
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

        // Use the shared core to check if this is a valid, identifiable symbol
        use crate::features::references_core::identify_symbol_at_position;
        let target = match identify_symbol_at_position(&self.document, symbols, tree, position) {
            Some(t) => t,
            None => return JsValue::NULL,
        };

        if !target.has_name() {
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
        let Some((symbols, tree)) = self.symbols_and_tree() else {
            return JsValue::NULL;
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

        // Find all references (including declaration) using the shared core
        let refs = crate::features::references_core::provide_references_core(
            &self.document,
            symbols,
            tree,
            position,
            true, // include_declaration
        );

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

        let core_symbols =
            crate::features::document_symbols_core::provide_document_symbols_core(symbols);
        let result = js_sys::Array::new();
        for sym in core_symbols {
            result.push(&document_symbol_info_to_js(&sym));
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
        let Some((symbols, tree)) = self.symbols_and_tree() else {
            return js_sys::Array::new().into();
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
        let Some((symbols, tree)) = self.symbols_and_tree() else {
            return JsValue::NULL;
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

        let info = match call_info.call_type {
            CallType::Direct => provide_direct_call_signature_core(symbols, &call_info),
            CallType::CallRef | CallType::ReturnCallRef => {
                provide_call_ref_signature_core(symbols, &call_info)
            }
        };

        match info {
            Some(help) => signature_help_to_js(help),
            None => JsValue::NULL,
        }
    }
}

// ============================================================================
// Signature Help JS conversion (core logic is in signature::signature_core)
// ============================================================================

fn signature_help_to_js(info: crate::signature::signature_core::SignatureHelpInfo) -> JsValue {
    let sig = info.signature;

    let parameters = js_sys::Array::new();
    for param in &sig.parameters {
        let param_obj = js_sys::Object::new();
        js_sys::Reflect::set(&param_obj, &"label".into(), &param.label.clone().into()).ok();
        if let Some(ref doc) = param.documentation {
            js_sys::Reflect::set(&param_obj, &"documentation".into(), &doc.clone().into()).ok();
        }
        parameters.push(&param_obj);
    }

    let sig_obj = js_sys::Object::new();
    js_sys::Reflect::set(&sig_obj, &"label".into(), &sig.label.into()).ok();
    js_sys::Reflect::set(&sig_obj, &"parameters".into(), &parameters).ok();
    js_sys::Reflect::set(
        &sig_obj,
        &"activeParameter".into(),
        &sig.active_parameter.into(),
    )
    .ok();

    if let Some(doc) = sig.documentation {
        js_sys::Reflect::set(&sig_obj, &"documentation".into(), &doc.into()).ok();
    }

    let signatures = js_sys::Array::new();
    signatures.push(&sig_obj);

    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"signatures".into(), &signatures).ok();
    js_sys::Reflect::set(&result, &"activeSignature".into(), &0u32.into()).ok();
    js_sys::Reflect::set(
        &result,
        &"activeParameter".into(),
        &sig.active_parameter.into(),
    )
    .ok();

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

fn range_obj_to_js(range: &Range) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"range".into(), &range_to_js(range)).ok();
    obj.into()
}

// ============================================================================

/// Convert a core::types::Diagnostic to a JavaScript object
fn core_diagnostic_to_js(diag: &CoreDiagnostic) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"range".into(), &range_to_js(&diag.range)).ok();
    js_sys::Reflect::set(&obj, &"message".into(), &diag.message.clone().into()).ok();
    let severity: u32 = match diag.severity {
        crate::core::types::DiagnosticSeverity::Error => 1,
        crate::core::types::DiagnosticSeverity::Warning => 2,
        crate::core::types::DiagnosticSeverity::Information => 3,
        crate::core::types::DiagnosticSeverity::Hint => 4,
    };
    js_sys::Reflect::set(&obj, &"severity".into(), &severity.into()).ok();
    obj.into()
}

// ============================================================================
// Document Symbol helpers (shared core → JS conversion)
// ============================================================================

use crate::core::types::DocumentSymbolInfo;

fn document_symbol_info_to_js(sym: &DocumentSymbolInfo) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"name".into(), &sym.name.clone().into()).ok();
    if let Some(ref d) = sym.detail {
        js_sys::Reflect::set(&obj, &"detail".into(), &d.clone().into()).ok();
    }
    js_sys::Reflect::set(&obj, &"kind".into(), &(sym.kind as u32).into()).ok();
    let range_js = range_to_js(&sym.range);
    js_sys::Reflect::set(&obj, &"range".into(), &range_js).ok();
    js_sys::Reflect::set(&obj, &"selectionRange".into(), &range_js).ok();
    if let Some(ref children) = sym.children {
        let arr = js_sys::Array::new();
        for child in children {
            arr.push(&document_symbol_info_to_js(child));
        }
        js_sys::Reflect::set(&obj, &"children".into(), &arr).ok();
    }
    obj.into()
}
