//! Per-URI document ownership. The queue serializes edits, reads, lifecycle events,
//! and diagnostic publication without holding a cache lock across an await.
//!
//! Analysis remains synchronous here; moving it to cancellable background workers
//! is a separate change. Such workers must return results through this queue and
//! check both document version and open-session identity before publishing.

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep_until, Duration, Instant};
use tower_lsp::lsp_types::*;
use tower_lsp::Client;
use tree_sitter::Tree;

use crate::{diagnostics, parser, tree_sitter_bindings, utils};

use super::server::DEBOUNCE_DURATION_MS;

/// Text and all derived data belong to the same version, including parse failure:
/// a failed parse has no tree or symbols, never data left over from an older edit.
#[derive(Debug)]
pub(super) struct DocumentSnapshot {
    pub text: Arc<String>,
    pub modules: Arc<Vec<parser::ModuleInfo>>,
    pub tree: Option<Tree>,
    pub version: i32,
    syntax_diagnostics: Vec<Diagnostic>,
    semantic_diagnostics: Vec<Diagnostic>,
}

impl DocumentSnapshot {
    fn parse(text: String, version: i32, old_tree: Option<&Tree>) -> Self {
        let tree = tree_sitter_bindings::create_parser().parse(&text, old_tree);
        let mut modules = Vec::new();
        let mut syntax_diagnostics = Vec::new();
        let mut semantic_diagnostics = Vec::new();
        if let Some(tree) = &tree {
            syntax_diagnostics = diagnostics::provide_tree_sitter_diagnostics(tree, &text);
            if let Ok(parsed) = parser::parse_modules_from_tree(tree, &text) {
                semantic_diagnostics = if parsed.len() <= 1 {
                    parsed.first().map_or_else(Vec::new, |module| {
                        diagnostics::provide_semantic_diagnostics(tree, &text, &module.symbols)
                    })
                } else {
                    diagnostics::provide_semantic_diagnostics_multi(tree, &text, &parsed)
                };
                modules = parsed;
            }
        }
        Self {
            text: Arc::new(text),
            modules: Arc::new(modules),
            tree,
            version,
            syntax_diagnostics,
            semantic_diagnostics,
        }
    }

    fn changed(&self, version: i32, changes: Vec<TextDocumentContentChangeEvent>) -> Self {
        let mut text = (*self.text).clone();
        let mut old_tree = self.tree.clone();
        for change in changes {
            if let Some(range) = change.range {
                let start_byte = utils::position_to_byte(&text, range.start.into());
                let old_end_byte = utils::position_to_byte(&text, range.end.into());
                let new_end = utils::apply_text_edit(
                    &mut text,
                    range.start.into(),
                    range.end.into(),
                    &change.text,
                );
                if let Some(tree) = &mut old_tree {
                    // Coordinate conversion is unchanged here; the byte/UTF-16
                    // InputEdit discrepancy is tracked separately in #289.
                    tree.edit(&tree_sitter::InputEdit {
                        start_byte,
                        old_end_byte,
                        new_end_byte: start_byte + change.text.len(),
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
                    });
                }
            } else {
                text = change.text;
                old_tree = None;
            }
        }
        Self::parse(text, version, old_tree.as_ref())
    }
}

pub(super) enum DocumentEvent {
    Open {
        text: String,
        version: i32,
    },
    Change {
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
    },
    Close,
    Snapshot,
}

struct Command {
    event: DocumentEvent,
    reply: oneshot::Sender<Option<Arc<DocumentSnapshot>>>,
}

#[derive(Debug, Clone)]
pub(super) struct DocumentHandle {
    commands: mpsc::Sender<Command>,
}

impl DocumentHandle {
    pub fn new(client: Client, uri: Url) -> Self {
        let (commands, mut incoming) = mpsc::channel::<Command>(16);
        tokio::spawn(async move {
            let mut snapshot: Option<Arc<DocumentSnapshot>> = None;
            let mut validation_due = None;
            loop {
                // The disabled timer branch still needs an argument to construct
                // its future; this deadline is never used when no work is due.
                let deadline = validation_due.unwrap_or_else(Instant::now);
                tokio::select! {
                    biased;
                    command = incoming.recv() => {
                        let Some(Command { event, reply }) = command else { break };
                        let mut updated = false;
                        match event {
                            DocumentEvent::Open { text, version } => {
                                snapshot = Some(Arc::new(DocumentSnapshot::parse(text, version, None)));
                                updated = true;
                            }
                            DocumentEvent::Change { changes, version } => {
                                if let Some(current) = &snapshot {
                                    if version > current.version {
                                        snapshot = Some(Arc::new(current.changed(version, changes)));
                                        updated = true;
                                    } else {
                                        client.log_message(MessageType::WARNING, format!(
                                            "Ignoring stale didChange for {uri}: version {version} <= {}",
                                            current.version,
                                        )).await;
                                    }
                                } else {
                                    // An incremental fragment cannot be interpreted
                                    // as the complete contents of an unopened file.
                                    client.log_message(MessageType::WARNING, format!(
                                        "Ignoring didChange for unopened document {uri}",
                                    )).await;
                                }
                            }
                            DocumentEvent::Close => {
                                snapshot = None;
                                validation_due = None;
                                client.publish_diagnostics(uri.clone(), vec![], None).await;
                            }
                            DocumentEvent::Snapshot => {}
                        }
                        if updated {
                            // State is committed before publication. Only this task
                            // publishes for this URI, so close/reopen and deferred
                            // validation cannot overtake each other.
                            if let Some(current) = &snapshot {
                                let mut combined = current.syntax_diagnostics.clone();
                                combined.extend(current.semantic_diagnostics.clone());
                                client.publish_diagnostics(uri.clone(), combined, Some(current.version)).await;
                            }
                            validation_due = Some(Instant::now() + Duration::from_millis(DEBOUNCE_DURATION_MS));
                        }
                        // Even if the caller was cancelled, its queued edit must
                        // take effect. Dropping a reply never discards a mutation.
                        let _ = reply.send(snapshot.clone());
                    }
                    _ = sleep_until(deadline), if validation_due.is_some() => {
                        validation_due = None;
                        if let Some(current) = &snapshot {
                            let wast = diagnostics::validate_wat(&current.text);
                            let combined = diagnostics::merge_all_diagnostics(
                                current.syntax_diagnostics.clone(),
                                current.semantic_diagnostics.clone(),
                                wast,
                            );
                            client.publish_diagnostics(uri.clone(), combined, Some(current.version)).await;
                        }
                    }
                }
            }
        });
        Self { commands }
    }

    pub async fn dispatch(&self, event: DocumentEvent) -> Option<Arc<DocumentSnapshot>> {
        let (reply, result) = oneshot::channel();
        self.commands.send(Command { event, reply }).await.ok()?;
        result.await.ok().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{future::join_all, StreamExt};
    use tower_lsp::LspService;

    fn document() -> (DocumentHandle, LspService<super::super::Backend>) {
        let mut document = None;
        let (service, mut socket) = LspService::new(|client| {
            document = Some(DocumentHandle::new(
                client.clone(),
                "file:///snapshot.wat".parse().unwrap(),
            ));
            super::super::Backend::new(client)
        });
        tokio::spawn(async move { while socket.next().await.is_some() {} });
        (document.unwrap(), service)
    }

    fn insertion(text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 16), Position::new(0, 16))),
            range_length: None,
            text: text.into(),
        }
    }

    #[tokio::test]
    async fn queued_edits_preserve_exact_text_and_immutable_snapshots() {
        let (document, _service) = document();
        let initial = "(module (func $a))";
        let original = document
            .dispatch(DocumentEvent::Open {
                text: initial.into(),
                version: 1,
            })
            .await
            .unwrap();
        let mut expected = initial.to_owned();
        let events: Vec<_> = (2..=81)
            .map(|version| {
                let text = char::from(b'b' + (version % 24) as u8).to_string();
                expected.insert_str(16, &text);
                DocumentEvent::Change {
                    version,
                    changes: vec![insertion(&text)],
                }
            })
            .collect();
        // More commands than channel capacity: exercise FIFO backpressure too.
        let results = join_all(events.into_iter().map(|event| document.dispatch(event))).await;
        assert!(results.iter().all(Option::is_some));
        let snapshot = document.dispatch(DocumentEvent::Snapshot).await.unwrap();
        assert_eq!(*snapshot.text, expected);
        assert_eq!(snapshot.version, 81);
        assert_eq!(*original.text, initial);
        assert_eq!(original.version, 1);
        let fresh = DocumentSnapshot::parse(expected, 81, None);
        assert_eq!(
            snapshot.tree.as_ref().unwrap().root_node().to_sexp(),
            fresh.tree.as_ref().unwrap().root_node().to_sexp()
        );
        assert_eq!(
            snapshot.tree.as_ref().unwrap().root_node().end_byte(),
            snapshot.text.len()
        );
        assert_eq!(snapshot.modules.len(), fresh.modules.len());
        assert!(snapshot.semantic_diagnostics.is_empty());
    }

    #[tokio::test]
    async fn abandoning_a_reply_does_not_discard_a_queued_edit() {
        let (document, _service) = document();
        document
            .dispatch(DocumentEvent::Open {
                text: "(module (func $a))".into(),
                version: 1,
            })
            .await
            .unwrap();
        let (reply, receiver) = oneshot::channel();
        document
            .commands
            .send(Command {
                event: DocumentEvent::Change {
                    version: 2,
                    changes: vec![insertion("b")],
                },
                reply,
            })
            .await
            .unwrap();
        drop(receiver);
        let snapshot = document.dispatch(DocumentEvent::Snapshot).await.unwrap();
        assert_eq!(*snapshot.text, "(module (func $ab))");
        assert_eq!(snapshot.version, 2);
    }
}
