//! LSP Backend — the tower-lsp `LanguageServer` implementation.
//!
//! Extracted from `main.rs` so that integration tests can construct
//! an `LspService` in-process without going through stdio.

use super::documents::{DocumentEvent, DocumentHandle};
use crate::parser::ModuleInfo;
use crate::{
    completion, definition, document_symbols, folding, hover, references, signature, symbols, utils,
};
use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::Tree;

/// Debounce duration for wast validation after edits (milliseconds).
pub const DEBOUNCE_DURATION_MS: u64 = 500;

/// Owned snapshot components: no map guards survive a feature handler's await.
type DocumentContext = (Arc<String>, Arc<Vec<ModuleInfo>>, Tree);

/// The LSP backend that implements `LanguageServer`.
#[derive(Debug)]
pub struct Backend {
    client: Client,
    // Keep a lightweight lane after close so a concurrent close/reopen cannot
    // create two owners for the same URI. Closed lanes retain no document data;
    // dropping the backend drops their senders and terminates the tasks.
    documents: DashMap<String, DocumentHandle>,
}

/// Find the SymbolTable for a given position from a list of modules.
/// Falls back to the first module if no module contains the position.
fn symbols_for_position(modules: &[ModuleInfo], pos: Position) -> Option<&symbols::SymbolTable> {
    let line = pos.line;
    let character = pos.character;
    for module in modules {
        let start = &module.range.start;
        let end = &module.range.end;
        if (line > start.line || (line == start.line && character >= start.character))
            && (line < end.line || (line == end.line && character <= end.character))
        {
            return Some(&module.symbols);
        }
    }
    // Fall back to first module
    modules.first().map(|m| &m.symbols)
}

/// Find the ModuleInfo for a given position (returns symbols + range).
fn module_for_position(modules: &[ModuleInfo], pos: Position) -> Option<&ModuleInfo> {
    let line = pos.line;
    let character = pos.character;
    for module in modules {
        let start = &module.range.start;
        let end = &module.range.end;
        if (line > start.line || (line == start.line && character >= start.character))
            && (line < end.line || (line == end.line && character <= end.character))
        {
            return Some(module);
        }
    }
    modules.first()
}

/// Get the first (or only) SymbolTable from a module list.
fn first_symbols(modules: &[ModuleInfo]) -> Option<&symbols::SymbolTable> {
    modules.first().map(|m| &m.symbols)
}

impl Backend {
    /// Create a new Backend with the given tower-lsp `Client`.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
        }
    }

    fn document(&self, uri: &Url) -> DocumentHandle {
        self.documents
            .entry(uri.to_string())
            .or_insert_with(|| DocumentHandle::new(self.client.clone(), uri.clone()))
            .clone()
    }

    /// Reads use the same queue as mutations, so a request observes preceding
    /// edits, not a mixture of independently fetched text, symbols, and tree.
    async fn get_document_context(&self, uri: &str) -> Option<DocumentContext> {
        let document = self.documents.get(uri)?.clone();
        let snapshot = document.dispatch(DocumentEvent::Snapshot).await?;
        Some((
            snapshot.text.clone(),
            snapshot.modules.clone(),
            snapshot.tree.clone()?,
        ))
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
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
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
        let doc = params.text_document;
        self.document(&doc.uri)
            .dispatch(DocumentEvent::Open {
                text: doc.text,
                version: doc.version,
            })
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Clone the lane and release the map guard before awaiting it.
        let document = self
            .documents
            .get(params.text_document.uri.as_str())
            .map(|d| d.clone());
        if let Some(document) = document {
            document
                .dispatch(DocumentEvent::Change {
                    changes: params.content_changes,
                    version: params.text_document.version,
                })
                .await;
        } else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!(
                        "Ignoring didChange for unopened document {}",
                        params.text_document.uri,
                    ),
                )
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.document(&params.text_document.uri)
            .dispatch(DocumentEvent::Close)
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        if let Some((doc, modules, tree)) = self.get_document_context(&uri).await {
            if let Some(syms) = symbols_for_position(&modules, position) {
                return Ok(hover::provide_hover(&doc, syms, &tree, position));
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        if let Some((doc, modules, _tree)) = self.get_document_context(&uri).await {
            if let Some(syms) = symbols_for_position(&modules, position) {
                let completions = completion::provide_completion(&doc, syms, position.into());
                return Ok(Some(CompletionResponse::Array(
                    completions.into_iter().map(|c| c.into()).collect(),
                )));
            }
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

        if let Some((doc, modules, tree)) = self.get_document_context(&uri).await {
            if let Some(syms) = symbols_for_position(&modules, position) {
                return Ok(signature::provide_signature_help(
                    &doc, syms, &tree, position,
                ));
            }
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

        if let Some((doc, modules, tree)) = self.get_document_context(&uri).await {
            if let Some(syms) = symbols_for_position(&modules, position) {
                if let Some(location) =
                    definition::provide_definition(&doc, syms, &tree, position, &uri)
                {
                    return Ok(Some(GotoDefinitionResponse::Scalar(location)));
                }
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "References requested at {}:{}",
                    position.line, position.character
                ),
            )
            .await;

        if let Some((doc, modules, tree)) = self.get_document_context(&uri).await {
            if let Some(module) = module_for_position(&modules, position) {
                // For multi-module docs, scope references to the containing module
                let module_scope = if modules.len() > 1 {
                    Some(module.range)
                } else {
                    None
                };
                let refs = references::provide_references_scoped(
                    &doc,
                    &module.symbols,
                    &tree,
                    position,
                    &uri,
                    include_declaration,
                    module_scope,
                );

                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("Found {} references", refs.len()),
                    )
                    .await;

                return Ok(Some(refs));
            }
        }

        self.client
            .log_message(MessageType::WARNING, "No document/symbols/tree found")
            .await;

        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri.to_string();

        if let Some((_doc, modules, _tree)) = self.get_document_context(&uri).await {
            // Aggregate document symbols from all modules
            let mut all_symbols = Vec::new();
            for module in modules.iter() {
                all_symbols.extend(document_symbols::provide_document_symbols(&module.symbols));
            }
            return Ok(Some(DocumentSymbolResponse::Nested(all_symbols)));
        }

        Ok(None)
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri.to_string();
        let position = params.position;

        if let Some((doc, modules, tree)) = self.get_document_context(&uri).await {
            let syms = match symbols_for_position(&modules, position) {
                Some(s) => s,
                None => return Ok(None),
            };
            if references::identify_symbol_at_position(&doc, syms, &tree, position).is_some() {
                // The symbol logic deems this a valid symbol.
                // Now find the range to select.
                if let Some(node) = utils::node_at_position(&tree, &doc, position.into()) {
                    // If it's an identifier (e.g. $foo), return its full range.
                    if node.kind() == "identifier" {
                        let range = Range {
                            start: Position {
                                line: node.start_position().row as u32,
                                character: node.start_position().column as u32,
                            },
                            end: Position {
                                line: node.end_position().row as u32,
                                character: node.end_position().column as u32,
                            },
                        };
                        return Ok(Some(PrepareRenameResponse::Range(range)));
                    } else if node.kind() == "nat" || node.kind() == "index" {
                        // Even for indices, return the range so client knows what to replace
                        let range = Range {
                            start: Position {
                                line: node.start_position().row as u32,
                                character: node.start_position().column as u32,
                            },
                            end: Position {
                                line: node.end_position().row as u32,
                                character: node.end_position().column as u32,
                            },
                        };
                        return Ok(Some(PrepareRenameResponse::Range(range)));
                    }
                }
            }
        }
        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        // Validation: New name MUST start with $
        if !new_name.starts_with('$') {
            self.client
                .show_message(
                    MessageType::ERROR,
                    format!("Invalid name '{}': symbols must start with '$'", new_name),
                )
                .await;
            return Ok(None);
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "Rename requested at {}:{} to {}",
                    position.line, position.character, new_name
                ),
            )
            .await;

        if let Some((doc, modules, tree)) = self.get_document_context(&uri).await {
            let module = match module_for_position(&modules, position) {
                Some(m) => m,
                None => return Ok(None),
            };
            let module_scope = if modules.len() > 1 {
                Some(module.range)
            } else {
                None
            };
            // Identify the symbol we are renaming
            if let Some(target) =
                references::identify_symbol_at_position(&doc, &module.symbols, &tree, position)
            {
                if !target.has_name() {
                    self.client
                        .show_message(MessageType::ERROR, "Cannot rename unnamed symbol")
                        .await;
                    return Ok(None);
                }

                // Find all references (scoped to module for multi-module docs)
                let refs = references::provide_references_scoped(
                    &doc,
                    &module.symbols,
                    &tree,
                    position,
                    &uri,
                    true, // include declaration
                    module_scope,
                );

                if refs.is_empty() {
                    return Ok(None);
                }

                // Create WorkspaceEdit
                let mut changes = std::collections::HashMap::new();
                let mut text_edits = Vec::new();

                for location in refs {
                    text_edits.push(TextEdit {
                        range: location.range,
                        new_text: new_name.clone(),
                    });
                }

                if let Ok(url) = Url::parse(&uri) {
                    changes.insert(url, text_edits);
                    return Ok(Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }));
                }
            } else {
                self.client
                    .show_message(MessageType::WARNING, "No symbol found at position")
                    .await;
            }
        }

        Ok(None)
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri.to_string();

        if let Some((doc, modules, tree)) = self.get_document_context(&uri).await {
            if let Some(syms) = first_symbols(&modules) {
                return Ok(Some(folding::provide_folding_ranges_lsp(&doc, syms, &tree)));
            }
        }

        Ok(None)
    }
}
