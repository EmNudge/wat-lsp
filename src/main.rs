mod completion;
mod definition;
mod diagnostics;
mod hover;
mod parser;
mod signature;
mod symbols;
mod utils;

use dashmap::DashMap;
use tokio::sync::watch;
use tokio::time::{sleep, Duration};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::Tree;
use wat_lsp_rust::tree_sitter_bindings;

const DEBOUNCE_DURATION_MS: u64 = 500;

#[derive(Debug)]
struct Backend {
    client: Client,
    document_map: DashMap<String, String>,
    symbol_map: DashMap<String, symbols::SymbolTable>,
    tree_map: DashMap<String, Tree>,
    validation_cancellation: DashMap<String, watch::Sender<bool>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            document_map: DashMap::new(),
            symbol_map: DashMap::new(),
            tree_map: DashMap::new(),
            validation_cancellation: DashMap::new(),
        }
    }

    async fn update_document(&self, uri: String, text: String) {
        // Parse with tree-sitter and cache the tree
        let mut parser = tree_sitter_bindings::create_parser();
        if let Some(tree) = parser.parse(&text, None) {
            // Generate IMMEDIATE syntax diagnostics only
            let syntax_diagnostics = diagnostics::provide_tree_sitter_diagnostics(&tree, &text);

            // Extract symbols from the document (needed for semantic diagnostics)
            let semantic_diagnostics = if let Ok(symbol_table) = parser::parse_document(&text) {
                self.symbol_map.insert(uri.clone(), symbol_table.clone());
                diagnostics::provide_semantic_diagnostics(&tree, &text, &symbol_table)
            } else {
                vec![]
            };

            // Merge syntax and semantic diagnostics
            let mut combined = syntax_diagnostics;
            combined.extend(semantic_diagnostics);

            // Publish immediate diagnostics
            if let Ok(lsp_uri) = uri.parse() {
                self.client
                    .publish_diagnostics(lsp_uri, combined, None)
                    .await;
            }

            // Cache the parsed tree
            self.tree_map.insert(uri.clone(), tree);
        }
        self.document_map.insert(uri.clone(), text.clone());

        // Schedule debounced wast validation
        self.schedule_wast_validation(uri, text).await;
    }

    async fn schedule_wast_validation(&self, uri: String, text: String) {
        // Cancel any existing validation task for this document
        if let Some(entry) = self.validation_cancellation.get(&uri) {
            let _ = entry.send(true); // Signal cancellation
        }

        // Create new cancellation channel
        let (tx, mut rx) = watch::channel(false);
        self.validation_cancellation.insert(uri.clone(), tx);

        // Clone what we need for the async task
        let client = self.client.clone();
        let tree_map = self.tree_map.clone();

        // Spawn background task
        tokio::spawn(async move {
            // Wait for debounce period or cancellation
            tokio::select! {
                _ = sleep(Duration::from_millis(DEBOUNCE_DURATION_MS)) => {
                    // Debounce period elapsed, run validation
                }
                _ = rx.changed() => {
                    // Cancelled by new edit
                    return;
                }
            }

            // Get tree-sitter diagnostics (from cached tree)
            let tree_diags = if let Some(tree) = tree_map.get(&uri) {
                diagnostics::provide_tree_sitter_diagnostics(&tree, &text)
            } else {
                vec![]
            };

            // Get semantic diagnostics (undefined label references)
            let semantic_diags = if let Some(tree) = tree_map.get(&uri) {
                if let Ok(symbols) = parser::parse_document(&text) {
                    diagnostics::provide_semantic_diagnostics(&tree, &text, &symbols)
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            // Run wast validation
            let wast_diags = diagnostics::validate_wat(&text);

            // Merge all diagnostics
            let combined =
                diagnostics::merge_all_diagnostics(tree_diags, semantic_diags, wast_diags);

            // Publish combined diagnostics
            if let Ok(lsp_uri) = uri.parse() {
                client.publish_diagnostics(lsp_uri, combined, None).await;
            }
        });
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "wat-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "$".to_string(),
                        "@".to_string(),
                        "2".to_string(),
                        "4".to_string(),
                    ]),
                    ..Default::default()
                }),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "WAT LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text;
        self.update_document(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        // Get the current document text
        let mut text = match self.document_map.get(&uri) {
            Some(doc) => doc.clone(),
            None => {
                // Document not found, fall back to full sync
                if let Some(change) = params.content_changes.into_iter().next() {
                    self.update_document(uri, change.text).await;
                }
                return;
            }
        };

        // Get the current tree for incremental reparsing
        let mut old_tree = self.tree_map.get(&uri).map(|t| t.clone());

        // Apply all incremental changes
        for change in params.content_changes {
            if let Some(range) = change.range {
                // Incremental change
                let start_byte = utils::position_to_byte(&text, range.start);
                let old_end_byte = utils::position_to_byte(&text, range.end);

                // Apply the text edit and get new end position
                let new_end =
                    utils::apply_text_edit(&mut text, range.start, range.end, &change.text);
                let new_end_byte = start_byte + change.text.len();

                // Apply the edit to the tree if we have one
                if let Some(ref mut tree) = old_tree {
                    // Create InputEdit for tree-sitter
                    let edit = tree_sitter::InputEdit {
                        start_byte,
                        old_end_byte,
                        new_end_byte,
                        start_position: tree_sitter::Point {
                            row: range.start.line as usize,
                            column: range.start.character as usize,
                        },
                        old_end_position: tree_sitter::Point {
                            row: range.end.line as usize,
                            column: range.end.character as usize,
                        },
                        new_end_position: tree_sitter::Point {
                            row: new_end.line as usize,
                            column: new_end.character as usize,
                        },
                    };

                    tree.edit(&edit);
                }
            } else {
                // Full document sync fallback
                text = change.text;
                old_tree = None; // Invalidate tree for full sync
            }
        }

        // Reparse with the edited tree for better performance
        let mut parser = tree_sitter_bindings::create_parser();
        if let Some(tree) = parser.parse(&text, old_tree.as_ref()) {
            // Generate IMMEDIATE syntax diagnostics
            let syntax_diagnostics = diagnostics::provide_tree_sitter_diagnostics(&tree, &text);

            // Extract symbols and generate semantic diagnostics
            let semantic_diagnostics = if let Ok(symbol_table) = parser::parse_document(&text) {
                self.symbol_map.insert(uri.clone(), symbol_table.clone());
                diagnostics::provide_semantic_diagnostics(&tree, &text, &symbol_table)
            } else {
                vec![]
            };

            // Merge syntax and semantic diagnostics
            let mut combined = syntax_diagnostics;
            combined.extend(semantic_diagnostics);

            // Publish immediate diagnostics
            if let Ok(lsp_uri) = uri.parse() {
                self.client
                    .publish_diagnostics(lsp_uri, combined, None)
                    .await;
            }

            // Cache the new tree
            self.tree_map.insert(uri.clone(), tree);
        }

        // Update document text
        self.document_map.insert(uri.clone(), text.clone());

        // Schedule debounced wast validation
        self.schedule_wast_validation(uri, text).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        // Clear diagnostics for the closed document
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;

        // Remove cached data
        self.document_map.remove(&uri);
        self.symbol_map.remove(&uri);
        self.tree_map.remove(&uri);

        // Cancel any pending validation
        self.validation_cancellation.remove(&uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let document = self.document_map.get(&uri);
        let symbols = self.symbol_map.get(&uri);
        let tree = self.tree_map.get(&uri);

        if let (Some(doc), Some(syms), Some(t)) = (document, symbols, tree) {
            return Ok(hover::provide_hover(&doc, &syms, &t, position));
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        let document = self.document_map.get(&uri);
        let symbols = self.symbol_map.get(&uri);
        let tree = self.tree_map.get(&uri);

        if let (Some(doc), Some(syms), Some(t)) = (document, symbols, tree) {
            return Ok(Some(CompletionResponse::Array(
                completion::provide_completion(&doc, &syms, &t, position),
            )));
        }

        Ok(None)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let document = self.document_map.get(&uri);
        let symbols = self.symbol_map.get(&uri);
        let tree = self.tree_map.get(&uri);

        if let (Some(doc), Some(syms), Some(t)) = (document, symbols, tree) {
            return Ok(signature::provide_signature_help(&doc, &syms, &t, position));
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let document = self.document_map.get(&uri);
        let symbols = self.symbol_map.get(&uri);
        let tree = self.tree_map.get(&uri);

        if let (Some(doc), Some(syms), Some(t)) = (document, symbols, tree) {
            if let Some(location) = definition::provide_definition(&doc, &syms, &t, position, &uri)
            {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
            }
        }

        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
