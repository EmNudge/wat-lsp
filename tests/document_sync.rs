//! Regression tests through the real stdio transport (including its concurrent
//! notification dispatch), not just sequential LspService calls.
#![cfg(feature = "native")]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const URI: &str = "file:///document-sync.wat";
const DOCUMENT: &str = "(module\n  (func $a)\n  (func (call $a)))";

struct Server {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    diagnostics: Vec<Value>,
    next_id: u64,
}

impl Server {
    fn new() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_wat-lsp-rust"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start language server");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, messages) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut length = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.strip_prefix("Content-Length:") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
                let mut body = vec![0; length.expect("Content-Length header")];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                if tx.send(serde_json::from_slice(&body).unwrap()).is_err() {
                    return;
                }
            }
        });
        let mut server = Self {
            child,
            stdin,
            messages,
            diagnostics: Vec::new(),
            next_id: 1,
        };
        server.request("initialize", json!({"capabilities": {}}));
        server.notify("initialized", json!({}));
        server
    }

    fn send(&mut self, message: Value) {
        let body = serde_json::to_vec(&message).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn receive_until(&mut self, deadline: Instant) -> Value {
        let message = self
            .messages
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("server response before timeout");
        if message["method"] == "textDocument/publishDiagnostics" {
            self.diagnostics.push(message["params"].clone());
        }
        message
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let message = self.receive_until(deadline);
            if message["id"] == id {
                assert!(message.get("error").is_none(), "{message}");
                return message["result"].clone();
            }
        }
    }

    fn open(&mut self, uri: &str, text: &str, version: i32) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri, "languageId": "wat", "version": version, "text": text,
            }}),
        );
    }

    fn change(&mut self, uri: &str, version: i32, changes: Value) {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": uri, "version": version}, "contentChanges": changes,
            }),
        );
    }

    fn symbols(&mut self, uri: &str) -> Value {
        self.request(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": uri}}),
        )
    }

    fn collect_for(&mut self, duration: Duration) {
        let deadline = Instant::now() + duration;
        while let Ok(message) = self
            .messages
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            if message["method"] == "textDocument/publishDiagnostics" {
                self.diagnostics.push(message["params"].clone());
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // The exit-notification lifecycle bug is separate from document sync.
        // Always reap the child, including when an assertion or timeout fails.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn insert(line: u32, column: u32, text: &str) -> Value {
    json!({"range": {
        "start": {"line": line, "character": column},
        "end": {"line": line, "character": column},
    }, "text": text})
}

#[test]
fn burst_edits_preserve_every_change_and_publish_matching_versions() {
    let mut server = Server::new();
    server.open(URI, DOCUMENT, 1);
    let mut expected = "$a".to_owned();
    let declaration = DOCUMENT.lines().nth(1).unwrap().find("$a").unwrap() as u32 + 2;
    let reference = DOCUMENT.lines().nth(2).unwrap().find("$a").unwrap() as u32 + 2;
    // No request/barrier between notifications. Each notification contains two
    // edits, and the next notification depends on the previous document state.
    for version in 2..=81 {
        let text = char::from(b'b' + (version % 24) as u8).to_string();
        expected.insert_str(2, &text);
        server.change(
            URI,
            version,
            json!([insert(1, declaration, &text), insert(2, reference, &text),]),
        );
    }
    let symbols = server.symbols(URI);
    assert_eq!(symbols[0]["name"], expected);
    // Check the reference cache too, and ensure it agrees with the text/tree.
    let refs = server.request(
        "textDocument/references",
        json!({
            "textDocument": {"uri": URI}, "position": {"line": 1, "character": 9},
            "context": {"includeDeclaration": true},
        }),
    );
    assert_eq!(refs.as_array().unwrap().len(), 2, "{refs}");
    for location in refs.as_array().unwrap() {
        let range = &location["range"];
        assert_eq!(
            range["end"]["character"].as_u64().unwrap()
                - range["start"]["character"].as_u64().unwrap(),
            expected.len() as u64
        );
    }
    server.collect_for(Duration::from_millis(750));
    let versions: Vec<_> = server
        .diagnostics
        .iter()
        .filter(|d| d["uri"] == URI)
        .map(|d| d["version"].as_i64().expect("versioned diagnostics"))
        .collect();
    assert!(versions.windows(2).all(|v| v[0] <= v[1]), "{versions:?}");
    assert_eq!(versions.last(), Some(&81));
    let mut distinct = versions.clone();
    distinct.dedup();
    assert_eq!(distinct, (1..=81).collect::<Vec<_>>());
    // Every intermediate version updates both declaration and use together.
    assert!(
        server
            .diagnostics
            .iter()
            .all(|d| d["diagnostics"].as_array().unwrap().is_empty()),
        "{:?}",
        server.diagnostics
    );
}

#[test]
fn stale_versions_are_ignored_and_changes_are_applied_sequentially() {
    let mut server = Server::new();
    server.open(URI, DOCUMENT, 10);
    // Gaps in version numbers are permitted, but duplicates and older edits are not.
    server.change(
        URI,
        20,
        json!([
            {"text": "(module (func $a))"},
            insert(0, 16, "b"),
            insert(0, 17, "c"),
        ]),
    );
    server.change(URI, 20, json!([{"text": "(module (func $duplicate))"}]));
    server.change(URI, 19, json!([{"text": "(module (func $stale))"}]));
    assert_eq!(server.symbols(URI)[0]["name"], "$abc");
    server.collect_for(Duration::from_millis(750));
    let last = server.diagnostics.last().unwrap();
    assert_eq!(last["version"], 20);
    assert!(last["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn close_cancels_validation_and_reopen_can_restart_versions() {
    let mut server = Server::new();
    server.open(URI, "(module (func (call $missing)))", 90);
    server.symbols(URI);
    server.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": URI}}),
    );
    // An incremental fragment for a closed document must never become a new file.
    server.change(URI, 91, json!([insert(0, 0, "(module (func $fragment))")]));
    assert!(server.symbols(URI).is_null());
    server.collect_for(Duration::from_millis(750));
    let clear_index = server
        .diagnostics
        .iter()
        .rposition(|d| d["version"].is_null())
        .expect("close clears diagnostics");
    assert_eq!(
        clear_index,
        server.diagnostics.len() - 1,
        "late diagnostics after close"
    );
    assert!(server.diagnostics[clear_index]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());

    server.open(URI, "(module (func $fresh))", 1);
    assert_eq!(server.symbols(URI)[0]["name"], "$fresh");
    server.collect_for(Duration::from_millis(750));
    for diagnostics in &server.diagnostics[clear_index + 1..] {
        assert_eq!(diagnostics["version"], 1);
        assert!(diagnostics["diagnostics"].as_array().unwrap().is_empty());
    }
}

#[test]
fn interleaved_documents_and_immediate_reopen_have_independent_state() {
    let mut server = Server::new();
    let other = "file:///other-document-sync.wat";
    server.open(URI, DOCUMENT, 50);
    server.open(other, DOCUMENT, 1);
    for version in 2..=21 {
        server.change(
            other,
            version,
            json!([{"text": format!("(module (func $other{version}))")} ]),
        );
        server.notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": URI}}),
        );
        let text = if version < 21 {
            "(module (func (call $missing)))".to_owned()
        } else {
            format!("(module (func $fresh{version}))")
        };
        server.open(URI, &text, 1);
    }
    assert_eq!(server.symbols(URI)[0]["name"], "$fresh21");
    assert_eq!(server.symbols(other)[0]["name"], "$other21");
    let settled = server.diagnostics.len();
    server.collect_for(Duration::from_millis(750));
    // All opens deliberately reuse version 1. Version comparison alone cannot
    // distinguish old-session diagnostics from the final valid session.
    for diagnostics in &server.diagnostics[settled..] {
        assert!(
            diagnostics["diagnostics"].as_array().unwrap().is_empty(),
            "{diagnostics}"
        );
    }
    for (uri, version) in [(URI, 1), (other, 21)] {
        let last = server
            .diagnostics
            .iter()
            .rev()
            .find(|d| d["uri"] == uri)
            .unwrap();
        assert_eq!(last["version"], version);
        assert!(last["diagnostics"].as_array().unwrap().is_empty());
    }
}
