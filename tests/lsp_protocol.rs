//! End-to-end tests for the native LSP protocol layer.
//!
//! These tests exercise the full tower-lsp service in-process by sending
//! JSON-RPC requests and draining the ClientSocket for notifications.

#![cfg(feature = "native")]

use std::sync::Arc;

use std::time::Duration;

use futures::StreamExt;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tower::Service;
use tower_lsp::jsonrpc::Request;
use tower_lsp::lsp_types::*;
use tower_lsp::{ClientSocket, LspService};

use wat_lsp_rust::native::Backend;

// ───────────────────────── Helpers ─────────────────────────

/// Collected server-to-client notifications.
#[derive(Debug, Default)]
struct Notifications {
    diagnostics: Vec<PublishDiagnosticsParams>,
}

/// Spin up an LspService, drain the socket in the background, and return the
/// service handle plus a shared notification store.
fn create_service() -> (LspService<Backend>, Arc<Mutex<Notifications>>) {
    let (service, socket) = LspService::new(Backend::new);
    let notifs = Arc::new(Mutex::new(Notifications::default()));
    let notifs2 = notifs.clone();

    // Drain socket on a background task to prevent backpressure deadlocks.
    tokio::spawn(drain_socket(socket, notifs2));

    (service, notifs)
}

async fn drain_socket(mut socket: ClientSocket, notifs: Arc<Mutex<Notifications>>) {
    while let Some(msg) = socket.next().await {
        if msg.method() == "textDocument/publishDiagnostics" {
            if let Some(params) = msg.params() {
                if let Ok(diag) = serde_json::from_value::<PublishDiagnosticsParams>(params.clone())
                {
                    notifs.lock().await.diagnostics.push(diag);
                }
            }
        }
        // Other notifications (log_message, etc.) are silently ignored.
    }
}

/// Send an initialize request then an initialized notification.
async fn initialize(svc: &mut LspService<Backend>) {
    let init_params = InitializeParams::default();
    let req = Request::build("initialize")
        .id(1)
        .params(serde_json::to_value(init_params).unwrap())
        .finish();
    let resp = svc.call(req).await.unwrap().unwrap();
    assert!(resp.is_ok(), "initialize should succeed");

    let notif = Request::build("initialized")
        .params(serde_json::to_value(InitializedParams {}).unwrap())
        .finish();
    let _ = svc.call(notif).await;
}

/// Send a textDocument/didOpen notification.
async fn open_document(svc: &mut LspService<Backend>, uri: &str, text: &str) {
    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.parse().unwrap(),
            language_id: "wat".into(),
            version: 1,
            text: text.into(),
        },
    };
    let notif = Request::build("textDocument/didOpen")
        .params(serde_json::to_value(params).unwrap())
        .finish();
    let _ = svc.call(notif).await;
}

/// Send a request and return its response result value.
async fn send_request(
    svc: &mut LspService<Backend>,
    method: &'static str,
    id: i64,
    params: Value,
) -> Value {
    let req = Request::build(method).id(id).params(params).finish();
    let resp = svc.call(req).await.unwrap().unwrap();
    assert!(resp.is_ok(), "request {method} (id={id}) failed: {resp:?}");
    resp.result().cloned().unwrap_or(Value::Null)
}

/// Send a textDocument/didChange notification with a full document replace.
async fn change_document(svc: &mut LspService<Backend>, uri: &str, text: &str, version: i32) {
    let params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.parse().unwrap(),
            version,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.into(),
        }],
    };
    let notif = Request::build("textDocument/didChange")
        .params(serde_json::to_value(params).unwrap())
        .finish();
    let _ = svc.call(notif).await;
}

/// Wait briefly for diagnostics to arrive via the background socket drainer.
async fn wait_for_diagnostics(notifs: &Arc<Mutex<Notifications>>, min_count: usize) {
    for _ in 0..50 {
        if notifs.lock().await.diagnostics.len() >= min_count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Standard test document with a function.
const TEST_DOC: &str = r#"(module
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
  (func $main (result i32)
    (call $add (i32.const 1) (i32.const 2))))"#;

const TEST_URI: &str = "file:///test.wat";

// ───────────────────────── Tests ─────────────────────────

#[tokio::test]
async fn lifecycle_initialize_shutdown() {
    let (mut svc, _notifs) = create_service();

    // Initialize
    let init_params = InitializeParams::default();
    let req = Request::build("initialize")
        .id(1)
        .params(serde_json::to_value(init_params).unwrap())
        .finish();
    let resp = svc.call(req).await.unwrap().unwrap();
    assert!(resp.is_ok());

    // Verify server info
    let result = resp.result().unwrap();
    let info = result.get("serverInfo").unwrap();
    assert_eq!(info.get("name").unwrap().as_str().unwrap(), "wat-lsp");

    // Verify capabilities
    let caps = result.get("capabilities").unwrap();
    assert!(caps.get("hoverProvider").is_some());
    assert!(caps.get("completionProvider").is_some());
    assert!(caps.get("definitionProvider").is_some());
    assert!(caps.get("referencesProvider").is_some());
    assert!(caps.get("renameProvider").is_some());
    assert!(caps.get("foldingRangeProvider").is_some());

    // Initialized notification
    let notif = Request::build("initialized")
        .params(serde_json::to_value(InitializedParams {}).unwrap())
        .finish();
    let _ = svc.call(notif).await;

    // Shutdown
    let req = Request::build("shutdown").id(2).finish();
    let resp = svc.call(req).await.unwrap().unwrap();
    assert!(resp.is_ok());
}

#[tokio::test]
async fn diagnostics_on_open_valid() {
    let (mut svc, notifs) = create_service();
    initialize(&mut svc).await;

    open_document(&mut svc, TEST_URI, TEST_DOC).await;
    wait_for_diagnostics(&notifs, 1).await;

    let store = notifs.lock().await;
    // Should have received at least one publishDiagnostics
    assert!(!store.diagnostics.is_empty(), "should receive diagnostics");
    // The valid doc should have 0 errors in the immediate diagnostics
    let last = store.diagnostics.last().unwrap();
    assert_eq!(
        last.diagnostics.len(),
        0,
        "valid document should have 0 errors, got: {:?}",
        last.diagnostics
    );
}

#[tokio::test]
async fn diagnostics_on_open_invalid() {
    let (mut svc, notifs) = create_service();
    initialize(&mut svc).await;

    let invalid = "(module (func (i32.add)))";
    open_document(&mut svc, TEST_URI, invalid).await;
    wait_for_diagnostics(&notifs, 1).await;

    let store = notifs.lock().await;
    assert!(!store.diagnostics.is_empty());
    let last = store.diagnostics.last().unwrap();
    assert!(
        !last.diagnostics.is_empty(),
        "invalid document should produce errors"
    );
}

#[tokio::test]
async fn hover_on_instruction() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;
    open_document(&mut svc, TEST_URI, TEST_DOC).await;

    // Hover on "i32.add" — line 2, col ~5
    let params = json!({
        "textDocument": { "uri": TEST_URI },
        "position": { "line": 2, "character": 5 }
    });
    let result = send_request(&mut svc, "textDocument/hover", 10, params).await;
    assert_ne!(result, Value::Null, "hover should return a result");

    let contents = result.get("contents").unwrap();
    let value = contents.get("value").unwrap().as_str().unwrap();
    assert!(
        value.contains("i32.add"),
        "hover on i32.add should mention the instruction, got: {value}"
    );
}

#[tokio::test]
async fn completion_after_dot() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;

    // Document with a partial instruction
    let doc = "(module\n  (func\n    (i32.";
    open_document(&mut svc, TEST_URI, doc).await;

    let params = json!({
        "textDocument": { "uri": TEST_URI },
        "position": { "line": 2, "character": 9 }
    });
    let result = send_request(&mut svc, "textDocument/completion", 11, params).await;
    assert_ne!(result, Value::Null, "completion should return results");

    let items = result.as_array().unwrap();
    assert!(!items.is_empty(), "should have completions");

    // Should include i32.add somewhere
    let labels: Vec<&str> = items
        .iter()
        .filter_map(|i| i.get("label").and_then(|l| l.as_str()))
        .collect();
    assert!(
        labels.iter().any(|l| l.contains("add")),
        "completions should include 'add', got: {labels:?}"
    );
}

#[tokio::test]
async fn goto_definition() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;
    open_document(&mut svc, TEST_URI, TEST_DOC).await;

    // Go to definition of "$add" reference in the call on line 4
    // "(call $add ..." — $add starts at col 10
    let params = json!({
        "textDocument": { "uri": TEST_URI },
        "position": { "line": 4, "character": 11 }
    });
    let result = send_request(&mut svc, "textDocument/definition", 12, params).await;
    assert_ne!(result, Value::Null, "definition should return a result");

    // Should point to line 1 where $add is defined
    let range = result.get("range").unwrap();
    let start_line = range
        .get("start")
        .unwrap()
        .get("line")
        .unwrap()
        .as_u64()
        .unwrap();
    assert_eq!(start_line, 1, "definition of $add should be on line 1");
}

#[tokio::test]
async fn find_references() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;
    open_document(&mut svc, TEST_URI, TEST_DOC).await;

    // Find references to "$add" from its definition on line 1, col ~8
    let params = json!({
        "textDocument": { "uri": TEST_URI },
        "position": { "line": 1, "character": 9 },
        "context": { "includeDeclaration": true }
    });
    let result = send_request(&mut svc, "textDocument/references", 13, params).await;
    let refs = result.as_array().unwrap();
    // Should find at least 2: the definition + the call site
    assert!(
        refs.len() >= 2,
        "should find definition + call site, got {} refs",
        refs.len()
    );
}

#[tokio::test]
async fn document_symbols() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;
    open_document(&mut svc, TEST_URI, TEST_DOC).await;

    let params = json!({
        "textDocument": { "uri": TEST_URI }
    });
    let result = send_request(&mut svc, "textDocument/documentSymbol", 14, params).await;
    let symbols = result.as_array().unwrap();
    // Should find at least 2 functions: $add and $main
    assert!(
        symbols.len() >= 2,
        "should find at least 2 symbols, got {}",
        symbols.len()
    );

    let names: Vec<&str> = symbols
        .iter()
        .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        names.iter().any(|n| n.contains("add")),
        "should find $add symbol, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("main")),
        "should find $main symbol, got: {names:?}"
    );
}

#[tokio::test]
async fn incremental_edit_updates_diagnostics() {
    let (mut svc, notifs) = create_service();
    initialize(&mut svc).await;

    // Start with valid doc
    open_document(&mut svc, TEST_URI, TEST_DOC).await;
    wait_for_diagnostics(&notifs, 1).await;

    let initial_count = notifs.lock().await.diagnostics.len();

    // Change to an invalid doc
    let invalid = "(module (func (unreachable) (i32.add)))";
    change_document(&mut svc, TEST_URI, invalid, 2).await;
    wait_for_diagnostics(&notifs, initial_count + 1).await;

    let store = notifs.lock().await;
    // The changed document has a type error (i32.add after unreachable)
    // Just verify we got fresh diagnostics for it
    assert!(
        store.diagnostics.len() > initial_count,
        "should receive new diagnostics after edit"
    );
}

#[tokio::test]
async fn folding_ranges() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;
    open_document(&mut svc, TEST_URI, TEST_DOC).await;

    let params = json!({
        "textDocument": { "uri": TEST_URI }
    });
    let result = send_request(&mut svc, "textDocument/foldingRange", 15, params).await;
    let ranges = result.as_array().unwrap();
    assert!(
        !ranges.is_empty(),
        "should return folding ranges for a multi-line document"
    );
}

#[tokio::test]
async fn signature_help() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;

    // Document with a call that should trigger signature help
    let doc = "(module\n  (func $add (param i32) (param i32) (result i32)\n    (i32.add (local.get 0) (local.get 1)))\n  (func (call $add";
    open_document(&mut svc, TEST_URI, doc).await;

    let params = json!({
        "textDocument": { "uri": TEST_URI },
        "position": { "line": 3, "character": 17 }
    });
    let _result = send_request(&mut svc, "textDocument/signatureHelp", 16, params).await;
    // Signature help may or may not return a result depending on context
    // We just verify it doesn't crash
}

#[tokio::test]
async fn prepare_rename() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;
    open_document(&mut svc, TEST_URI, TEST_DOC).await;

    // Prepare rename on $add (line 1, col 9)
    let params = json!({
        "textDocument": { "uri": TEST_URI },
        "position": { "line": 1, "character": 9 }
    });
    let result = send_request(&mut svc, "textDocument/prepareRename", 17, params).await;
    assert_ne!(result, Value::Null, "prepareRename should return a range");
}

#[tokio::test]
async fn rename_symbol() {
    let (mut svc, _notifs) = create_service();
    initialize(&mut svc).await;
    open_document(&mut svc, TEST_URI, TEST_DOC).await;

    // Rename $add to $sum
    let params = json!({
        "textDocument": { "uri": TEST_URI },
        "position": { "line": 1, "character": 9 },
        "newName": "$sum"
    });
    let result = send_request(&mut svc, "textDocument/rename", 18, params).await;
    assert_ne!(result, Value::Null, "rename should return a WorkspaceEdit");

    let changes = result.get("changes").unwrap();
    // Should have edits for our URI
    let uri_edits = changes.get(TEST_URI).unwrap().as_array().unwrap();
    assert!(
        uri_edits.len() >= 2,
        "should have at least 2 edits (definition + call), got {}",
        uri_edits.len()
    );

    // All edits should set the new name
    for edit in uri_edits {
        assert_eq!(edit.get("newText").unwrap().as_str().unwrap(), "$sum");
    }
}

#[tokio::test]
async fn did_close_clears_diagnostics() {
    let (mut svc, notifs) = create_service();
    initialize(&mut svc).await;

    // Open doc with errors
    let invalid = "(module (func (i32.add)))";
    open_document(&mut svc, TEST_URI, invalid).await;
    wait_for_diagnostics(&notifs, 1).await;

    let before = notifs.lock().await.diagnostics.len();

    // Close document
    let params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier {
            uri: TEST_URI.parse().unwrap(),
        },
    };
    let notif = Request::build("textDocument/didClose")
        .params(serde_json::to_value(params).unwrap())
        .finish();
    let _ = svc.call(notif).await;

    // Wait for the clear-diagnostics notification
    wait_for_diagnostics(&notifs, before + 1).await;

    let store = notifs.lock().await;
    let last = store.diagnostics.last().unwrap();
    assert!(
        last.diagnostics.is_empty(),
        "closing document should clear diagnostics"
    );
}
