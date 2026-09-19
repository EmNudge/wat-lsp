//! Per-URI document ownership. The queue serializes edits, reads, lifecycle events,
//! and diagnostic publication without holding a cache lock across an await.
//!
//! An edit is split into two phases. The ordered, cheap text mutation (and the
//! `Tree::edit` bookkeeping on a clone of the old tree) runs synchronously on the
//! actor. The expensive tree-sitter reparse plus the syntax/semantic passes then
//! run on a blocking thread (`spawn_blocking`) that the actor `await`s: the actor's
//! executor worker thread is released for the duration, so a large or deeply
//! nested edit no longer pins a runtime worker while it parses. Because the actor
//! is a single task that owns the snapshot and processes commands serially, edits
//! and reads stay strictly ordered — a read still observes every preceding edit's
//! text and tree — and no mutation is ever dropped.
//!
//! The heavier full `wast` validation runs off the async executor in a background
//! task and returns its result through this queue, tagged with the document version
//! and an open-session generation; the actor publishes it only if neither has moved
//! on. Obsolete or post-close results are discarded instead of clobbering current
//! diagnostics — aborting a blocking task is never assumed to stop its work.

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep_until, Duration, Instant};
use tower_lsp::lsp_types::*;
use tower_lsp::Client;
use tree_sitter::Tree;

use crate::{diagnostics, parser, tree_sitter_bindings, utils};

use super::server::DEBOUNCE_DURATION_MS;

/// Inputs larger than this (bytes) skip the expensive semantic and `wast`
/// validation passes. Tolerant tree-sitter syntax diagnostics still run so the
/// editor stays usable; a single info diagnostic explains the fallback. Keeping
/// the guard in bytes avoids scanning the whole document just to measure it.
pub(super) const MAX_ANALYSIS_BYTES: usize = 4 * 1024 * 1024;

/// Maximum parenthesis nesting depth before the semantic and `wast` passes are
/// skipped. Deeply nested s-expressions drive recursive traversals that can
/// occupy the request path far longer than the input size suggests, so this is
/// bounded independently of `MAX_ANALYSIS_BYTES`.
pub(super) const MAX_ANALYSIS_DEPTH: usize = 500;

/// Cheap, allocation-free upper bound on s-expression nesting. Counts the
/// deepest run of unbalanced `(` while ignoring parens inside comments and
/// strings so that text content cannot inflate the estimate. Runs in a single
/// pass over the bytes, so it stays proportional to input size.
fn max_paren_depth(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut max = 0usize;
    let mut block_comment = 0usize;
    let mut in_line_comment = false;
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }
        if block_comment > 0 {
            if b == b'(' && bytes.get(i + 1) == Some(&b';') {
                block_comment += 1;
                i += 2;
                continue;
            }
            if b == b';' && bytes.get(i + 1) == Some(&b')') {
                block_comment -= 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' => in_string = true,
            b';' if bytes.get(i + 1) == Some(&b';') => {
                in_line_comment = true;
                i += 2;
                continue;
            }
            b'(' if bytes.get(i + 1) == Some(&b';') => {
                block_comment = 1;
                i += 2;
                continue;
            }
            b'(' => {
                depth += 1;
                max = max.max(depth);
            }
            b')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    max
}

/// Whether a document should skip the expensive semantic and `wast` passes.
/// Returns the info diagnostic to surface the fallback, or `None` to analyze.
fn analysis_budget_exceeded(text: &str) -> Option<Diagnostic> {
    let reason = if text.len() > MAX_ANALYSIS_BYTES {
        "document exceeds the size limit for semantic analysis"
    } else if max_paren_depth(text) > MAX_ANALYSIS_DEPTH {
        "document exceeds the nesting-depth limit for semantic analysis"
    } else {
        return None;
    };
    Some(Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(DiagnosticSeverity::INFORMATION),
        source: Some("wat-lsp".to_string()),
        message: format!("Semantic and script validation skipped: {reason}."),
        ..Default::default()
    })
}

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
    /// Set when the analysis budget was exceeded: the semantic pass was skipped
    /// and the background `wast` validation must be skipped too, so incomplete
    /// validation is never published as a clean result.
    analysis_skipped: bool,
}

impl DocumentSnapshot {
    fn parse(text: String, version: i32, old_tree: Option<&Tree>) -> Self {
        let tree = tree_sitter_bindings::create_parser().parse(&text, old_tree);
        let mut modules = Vec::new();
        let mut syntax_diagnostics = Vec::new();
        let mut semantic_diagnostics = Vec::new();
        let budget = analysis_budget_exceeded(&text);
        let analysis_skipped = budget.is_some();
        if let Some(tree) = &tree {
            syntax_diagnostics = diagnostics::provide_tree_sitter_diagnostics(tree, &text);
            if let Some(diagnostic) = budget {
                // Fall back to tolerant syntax diagnostics only. Skip the
                // recursive semantic traversal (and, later, `wast` validation)
                // so a pathological input cannot occupy the request path.
                syntax_diagnostics.push(diagnostic);
            } else if let Ok(parsed) = parser::parse_modules_from_tree(tree, &text) {
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
            analysis_skipped,
        }
    }

    /// Apply an ordered batch of changes to this snapshot's text, mirroring each
    /// edit onto a clone of the old tree (the cheap `Tree::edit` bookkeeping, not
    /// a reparse). Returns the mutated text plus the edited old tree so the caller
    /// can run the expensive [`Self::parse`] off the async executor. Returns `None`
    /// if any edit in the batch is invalid; the batch is then rejected atomically
    /// and no mutation is committed.
    fn apply_changes(
        &self,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Option<(String, Option<Tree>)> {
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
        Some((text, old_tree))
    }

    /// Run the tree-sitter parse plus the syntax/semantic passes on a blocking
    /// thread so the CPU-bound work stays off the async executor's worker threads.
    /// The actor `await`s this: its own worker thread is released for the duration
    /// (other documents' actors and reads keep running), and edits/reads for this
    /// document stay strictly ordered because the actor processes them serially.
    async fn parse_off_executor(
        text: String,
        version: i32,
        old_tree: Option<Tree>,
    ) -> Option<Self> {
        // `spawn_blocking` only fails if the blocking closure panics or the runtime
        // is shutting down. In either case we return `None` and leave the previously
        // installed snapshot in place rather than committing empty text.
        tokio::task::spawn_blocking(move || Self::parse(text, version, old_tree.as_ref()))
            .await
            .ok()
    }

    #[cfg(test)]
    fn changed(&self, version: i32, changes: Vec<TextDocumentContentChangeEvent>) -> Option<Self> {
        let (text, old_tree) = self.apply_changes(changes)?;
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
    /// `.wast` documents are conformance scripts: they may contain script
    /// directives (`assert_*`, `register`) and multiple top-level modules, which
    /// the single-module `wast` validation path misreports as errors. For those
    /// URIs the background `wast` validation is suppressed and only tolerant
    /// tree-sitter syntax (plus semantic) diagnostics are published. The separate
    /// `wast-runner` conformance binary calls `validate_wat` directly and is
    /// unaffected by this gate.
    pub fn new(client: Client, uri: Url) -> Self {
        let validate_script = !uri
            .path()
            .rsplit('.')
            .next()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wast"));
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
                                // The parse runs off the executor; `await` releases
                                // this worker thread while it runs. Only this task
                                // mutates `snapshot`, and it is single-threaded across
                                // awaits, so the install is race-free.
                                if let Some(parsed) =
                                    DocumentSnapshot::parse_off_executor(text, version, None).await
                                {
                                    snapshot = Some(Arc::new(parsed));
                                    updated = true;
                                }
                            }
                            DocumentEvent::Change { changes, version } => {
                                if let Some(current) = &snapshot {
                                    if version > current.version {
                                        // Apply the cheap, ordered text/tree mutation
                                        // synchronously, then run the expensive parse
                                        // off the executor. The batch is rejected
                                        // atomically if any edit is invalid.
                                        if let Some((text, old_tree)) =
                                            current.apply_changes(changes)
                                        {
                                            if let Some(parsed) =
                                                DocumentSnapshot::parse_off_executor(
                                                    text, version, old_tree,
                                                )
                                                .await
                                            {
                                                snapshot = Some(Arc::new(parsed));
                                                updated = true;
                                            }
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
                        // `.wast` scripts and inputs that blew the analysis budget
                        // never run the single-module `wast` validation: it would
                        // misreport script directives / multiple modules, or re-run
                        // the pathological work the parse pass already skipped.
                        if let Some(current) = snapshot.as_ref()
                            .filter(|_| validate_script)
                            .filter(|current| !current.analysis_skipped)
                        {
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
        document_for("file:///snapshot.wat")
    }

    fn document_for(uri: &str) -> (DocumentHandle, LspService<super::super::Backend>) {
        let mut document = None;
        let uri = uri.to_owned();
        let (service, mut socket) = LspService::new(|client| {
            document = Some(DocumentHandle::new(client.clone(), uri.parse().unwrap()));
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
    fn max_paren_depth_ignores_comments_and_strings() {
        assert_eq!(max_paren_depth("(module (func))"), 2);
        // Parens inside line/block comments and strings do not count.
        assert_eq!(max_paren_depth("(a ;; (((\n)"), 1);
        assert_eq!(max_paren_depth("(a (; ((( ;) )"), 1);
        assert_eq!(max_paren_depth("(data \"(((\")"), 1);
        // Escaped quote keeps the string open, so its parens stay uncounted.
        assert_eq!(max_paren_depth("(a \"\\\"(((\" )"), 1);
        // Unbalanced closes never underflow the depth.
        assert_eq!(max_paren_depth(")))(x)"), 1);
    }

    #[test]
    fn deep_nesting_skips_semantic_analysis_with_fallback() {
        let deep = "(".repeat(MAX_ANALYSIS_DEPTH + 5);
        let snapshot = DocumentSnapshot::parse(deep, 1, None);
        assert!(snapshot.analysis_skipped);
        assert!(snapshot.semantic_diagnostics.is_empty());
        assert!(snapshot.modules.is_empty());
        assert!(snapshot
            .syntax_diagnostics
            .iter()
            .any(|d| d.severity == Some(DiagnosticSeverity::INFORMATION)
                && d.message.contains("nesting-depth")));
    }

    #[test]
    fn oversized_input_skips_semantic_analysis_with_fallback() {
        let mut text = String::from("(module (func $a))\n");
        text.push_str(&";; padding\n".repeat(MAX_ANALYSIS_BYTES / 11 + 1));
        assert!(text.len() > MAX_ANALYSIS_BYTES);
        let snapshot = DocumentSnapshot::parse(text, 1, None);
        assert!(snapshot.analysis_skipped);
        assert!(snapshot.semantic_diagnostics.is_empty());
        assert!(snapshot
            .syntax_diagnostics
            .iter()
            .any(|d| d.message.contains("size limit")));
    }

    #[test]
    fn ordinary_document_is_analyzed() {
        let snapshot = DocumentSnapshot::parse("(module (func $a))".into(), 1, None);
        assert!(!snapshot.analysis_skipped);
        assert_eq!(snapshot.modules.len(), 1);
    }

    #[tokio::test]
    async fn wast_script_document_opens_without_panicking() {
        // `.wast` scripts (directives, multiple modules) must open cleanly; the
        // background single-module `wast` validation is suppressed for them. The
        // observable no-late-error behavior is asserted end-to-end in
        // tests/document_sync.rs against real published diagnostics.
        let (document, _service) = document_for("file:///script.wast");
        let script = "(module)\n(assert_return (invoke \"missing\") (i32.const 0))\n";
        let snapshot = document
            .dispatch(DocumentEvent::Open {
                text: script.into(),
                version: 1,
            })
            .await
            .unwrap();
        // Give the debounce timer time to fire; it must be a no-op for `.wast`.
        tokio::time::sleep(Duration::from_millis(DEBOUNCE_DURATION_MS + 200)).await;
        assert!(!snapshot.analysis_skipped);
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
