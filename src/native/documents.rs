//! Per-URI document ownership. The queue serializes edits, reads, lifecycle events,
//! and diagnostic publication without holding a cache lock across an await.
//!
//! Incremental parse and syntax/semantic diagnostics run synchronously so reads
//! see a current snapshot and fast feedback publishes immediately. The heavier
//! full `wast` validation runs off the async executor in a background task and
//! returns its result through this queue, tagged with the document version and
//! an open-session generation; the actor publishes it only if neither has moved
//! on. That keeps the actor free to serve reads while validation runs, and means
//! obsolete or post-close results are discarded instead of clobbering current
//! diagnostics — aborting a blocking task is never assumed to stop its work.

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

    fn changed(&self, version: i32, changes: Vec<TextDocumentContentChangeEvent>) -> Option<Self> {
        let mut text = (*self.text).clone();
        let mut old_tree = self.tree.clone();
        for change in changes {
            if let Some(range) = change.range {
                let edit = utils::apply_text_edit_checked(
                    &mut text,
                    range.start.into(),
                    range.end.into(),
                    &change.text,
                )?;
                if let Some(tree) = &mut old_tree {
                    tree.edit(&tree_sitter::InputEdit {
                        start_byte: edit.start_byte,
                        old_end_byte: edit.old_end_byte,
                        new_end_byte: edit.new_end_byte,
                        start_position: tree_sitter::Point {
                            row: edit.start_position.line as usize,
                            column: edit.start_position.character as usize,
                        },
                        old_end_position: tree_sitter::Point {
                            row: edit.old_end_position.line as usize,
                            column: edit.old_end_position.character as usize,
                        },
                        new_end_position: tree_sitter::Point {
                            row: edit.new_end_position.line as usize,
                            column: edit.new_end_position.character as usize,
                        },
                    });
                }
            } else {
                text = change.text;
                old_tree = None;
            }
        }
        Some(Self::parse(text, version, old_tree.as_ref()))
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
            // Bumped on every open/close so a background validation started for an
            // earlier session cannot publish into a later one.
            let mut generation: u64 = 0;
            // Background validations return (generation, version, wast diagnostics)
            // here. The actor holds a sender, so recv never spuriously closes.
            let (validation_tx, mut validation_rx) =
                mpsc::channel::<(u64, i32, Vec<Diagnostic>)>(8);
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
                                generation += 1;
                                snapshot = Some(Arc::new(DocumentSnapshot::parse(text, version, None)));
                                updated = true;
                            }
                            DocumentEvent::Change { changes, version } => {
                                if let Some(current) = &snapshot {
                                    if version > current.version {
                                        if let Some(changed) = current.changed(version, changes) {
                                            snapshot = Some(Arc::new(changed));
                                            updated = true;
                                        } else {
                                            client.log_message(MessageType::WARNING, format!(
                                                "Ignoring didChange with a reversed range for {uri} at version {version}",
                                            )).await;
                                        }
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
                                generation += 1;
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
                                super::positions::diagnostics(&current.text, &uri, &mut combined);
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
                            // Run the expensive full validation off the async executor.
                            // Tag it with the version and generation it was started for;
                            // the result is version/generation-gated on the way back so a
                            // later edit or a close discards it instead of clobbering.
                            let text = current.text.clone();
                            let version = current.version;
                            let started_generation = generation;
                            let tx = validation_tx.clone();
                            tokio::spawn(async move {
                                let wast = tokio::task::spawn_blocking(move || {
                                    diagnostics::validate_wat(&text)
                                })
                                .await
                                .unwrap_or_default();
                                let _ = tx.send((started_generation, version, wast)).await;
                            });
                        }
                    }
                    result = validation_rx.recv() => {
                        // Only the current, still-open version may publish. A result for
                        // a superseded edit or a closed session is dropped.
                        if let Some((result_generation, result_version, wast)) = result {
                            if result_generation == generation {
                                if let Some(current) = &snapshot {
                                    if current.version == result_version {
                                        let mut combined = diagnostics::merge_all_diagnostics(
                                            current.syntax_diagnostics.clone(),
                                            current.semantic_diagnostics.clone(),
                                            wast,
                                        );
                                        super::positions::diagnostics(&current.text, &uri, &mut combined);
                                        client.publish_diagnostics(uri.clone(), combined, Some(current.version)).await;
                                    }
                                }
                            }
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
    #[test]
    fn unicode_incremental_snapshots_match_fresh_parses() {
        use crate::core::text::TextIndex;
        fn geometry(
            tree: &Tree,
        ) -> Vec<(String, usize, usize, tree_sitter::Point, tree_sitter::Point)> {
            fn walk(
                node: tree_sitter::Node<'_>,
                out: &mut Vec<(String, usize, usize, tree_sitter::Point, tree_sitter::Point)>,
            ) {
                out.push((
                    node.kind().to_owned(),
                    node.start_byte(),
                    node.end_byte(),
                    node.start_position(),
                    node.end_position(),
                ));
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    walk(child, out);
                }
            }
            let mut nodes = Vec::new();
            walk(tree.root_node(), &mut nodes);
            nodes
        }
        let mut snapshot =
            DocumentSnapshot::parse("(module (; é漢😀 ;) (func $a))\r\n".into(), 1, None);
        for (needle, replacement) in [
            ("😀", "😀x\r\n漢\n"),
            ("$a", "$long"),
            ("é", "😀"),
            ("x\r\n漢\n", "z"),
            ("\r\n", "\n\n"),
        ] {
            let start = snapshot.text.find(needle).unwrap();
            let end = start + needle.len();
            let mut expected = (*snapshot.text).clone();
            expected.replace_range(start..end, replacement);
            let index = TextIndex::new(&snapshot.text);
            let change = TextDocumentContentChangeEvent {
                range: Some(Range::new(
                    index.byte_to_utf16(start).into(),
                    index.byte_to_utf16(end).into(),
                )),
                range_length: None,
                text: replacement.into(),
            };
            snapshot = snapshot
                .changed(snapshot.version + 1, vec![change])
                .unwrap();
            assert_eq!(*snapshot.text, expected);
            let fresh = DocumentSnapshot::parse(expected, snapshot.version, None);
            assert_eq!(
                geometry(snapshot.tree.as_ref().unwrap()),
                geometry(fresh.tree.as_ref().unwrap())
            );
            assert_eq!(snapshot.syntax_diagnostics, fresh.syntax_diagnostics);
            assert_eq!(snapshot.semantic_diagnostics, fresh.semantic_diagnostics);
        }
    }

    #[test]
    fn invalid_edit_batch_is_rejected_atomically() {
        let snapshot = DocumentSnapshot::parse("(module (func $a))".into(), 1, None);
        let mut reversed = insertion("BAD");
        reversed.range = Some(Range::new(Position::new(0, 17), Position::new(0, 0)));
        assert!(snapshot
            .changed(2, vec![insertion("b"), reversed])
            .is_none());
        assert_eq!(*snapshot.text, "(module (func $a))");
        assert_eq!(snapshot.version, 1);
    }
}
