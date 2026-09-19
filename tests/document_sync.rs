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
    initialization: Value,
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
            initialization: Value::Null,
        };
        let initialized = server.request(
            "initialize",
            json!({"capabilities": {
                "general": {"positionEncodings": ["utf-8", "utf-16"]}
            }}),
        );
        server.initialization = initialized;
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
fn reads_on_a_second_document_stay_responsive_during_a_large_edit() {
    // The per-edit reparse now runs on a blocking thread the actor `await`s, so a
    // large edit on one document releases the executor worker instead of pinning
    // it. A read on a second, small document must therefore still return promptly
    // and correctly while the large document is being (re)parsed.
    let mut server = Server::new();
    let big = "file:///big-document-sync.wat";
    let small = "file:///small-document-sync.wat";
    // A large body: many functions so tree-sitter + the semantic pass do real work.
    let mut body = String::from("(module\n");
    for i in 0..4000 {
        body.push_str(&format!("  (func $f{i} (call $f{i}))\n"));
    }
    body.push(')');
    server.open(big, &body, 1);
    server.open(small, "(module (func $small))", 1);
    // Fire an expensive edit on the big document, then immediately query the small
    // one without waiting for the big reparse to publish.
    server.change(big, 2, json!([insert(0, 0, ";; touch\n")]));
    let symbols = server.symbols(small);
    assert_eq!(symbols[0]["name"], "$small");
    // The big document still converges to its edited state.
    let big_symbols = server.symbols(big);
    assert!(
        big_symbols.as_array().unwrap().len() >= 4000,
        "big document keeps all functions after the edit"
    );
}

#[test]
fn burst_edits_on_a_large_document_lose_no_edits_and_converge() {
    // Burst typing into a large document: every ordered edit must be applied and
    // the document must converge to the final text even though each reparse runs
    // off the executor. No edit may be dropped or reordered.
    let mut server = Server::new();
    let mut body = String::from("(module (func $a)\n");
    for i in 0..1500 {
        body.push_str(&format!("  (func $pad{i})\n"));
    }
    body.push(')');
    server.open(URI, &body, 1);
    // The declaration `$a` sits on line 0 at a fixed column; grow it with a burst
    // of single-character insertions and confirm the final name is exact.
    let column = body.lines().next().unwrap().find("$a").unwrap() as u32 + 2;
    let mut expected = "$a".to_owned();
    for version in 2..=61 {
        let ch = char::from(b'b' + (version % 24) as u8).to_string();
        expected.insert_str(2, &ch);
        server.change(URI, version, json!([insert(0, column, &ch)]));
    }
    let symbols = server.symbols(URI);
    assert_eq!(symbols[0]["name"], expected);
    server.collect_for(Duration::from_millis(750));
    let versions: Vec<_> = server
        .diagnostics
        .iter()
        .filter(|d| d["uri"] == URI)
        .map(|d| d["version"].as_i64().expect("versioned diagnostics"))
        .collect();
    assert!(versions.windows(2).all(|v| v[0] <= v[1]), "{versions:?}");
    assert_eq!(versions.last(), Some(&61));
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

// Independent UTF-16 oracle for wire assertions; deliberately does not use the
// server's conversion helpers, so matching conversion bugs cannot hide a failure.
fn client_position(source: &str, byte: usize) -> Value {
    let before = &source[..byte];
    let line = before.bytes().filter(|&b| b == b'\n').count();
    let column = before.rsplit('\n').next().unwrap().encode_utf16().count();
    json!({"line": line, "character": column})
}

fn client_offset(source: &str, position: &Value) -> usize {
    let row = position["line"].as_u64().unwrap() as usize;
    let mut units = position["character"].as_u64().unwrap() as usize;
    let mut start = 0;
    for _ in 0..row {
        start += source[start..]
            .find('\n')
            .expect("range line lies in document")
            + 1;
    }
    let line = source[start..]
        .split('\n')
        .next()
        .unwrap()
        .trim_end_matches('\r');
    for (byte, ch) in line.char_indices() {
        if units == 0 {
            return start + byte;
        }
        assert!(
            units >= ch.len_utf16(),
            "position splits a surrogate pair: {position}"
        );
        units -= ch.len_utf16();
    }
    assert_eq!(
        units, 0,
        "range column escapes document: {position}, {source:?}"
    );
    start + line.len()
}

fn client_range(source: &str, range: &Value) -> std::ops::Range<usize> {
    let start = client_offset(source, &range["start"]);
    let end = client_offset(source, &range["end"]);
    assert!(start <= end, "reversed range: {range}");
    start..end
}

#[test]
fn unicode_rename_definition_references_and_outline_use_utf16() {
    let mut server = Server::new();
    for prefix in ["ascii", "é", "漢", "😀", "é漢😀"] {
        let source = format!("(module (; {prefix} ;) (func $a (param $p i32) (local $l i32)) (func $caller (call $a (i32.const 0))))");
        server.open(URI, &source, 1);
        let position = client_position(&source, source.rfind("$a").unwrap() + 1);
        let params = json!({"textDocument": {"uri": URI}, "position": position});
        let prepared = server.request("textDocument/prepareRename", params.clone());
        assert_eq!(&source[client_range(&source, &prepared)], "$a");
        let definition = server.request("textDocument/definition", params.clone());
        assert_eq!(&source[client_range(&source, &definition["range"])], "$a");
        assert_eq!(
            definition["range"]["start"],
            client_position(&source, source.find("$a").unwrap())
        );
        let references = server.request(
            "textDocument/references",
            json!({
                "textDocument": {"uri": URI}, "position": position,
                "context": {"includeDeclaration": true},
            }),
        );
        assert_eq!(references.as_array().unwrap().len(), 2);
        for location in references.as_array().unwrap() {
            assert_eq!(&source[client_range(&source, &location["range"])], "$a");
        }
        let outline = server.symbols(URI);
        assert_eq!(
            &source[client_range(&source, &outline[0]["selectionRange"])],
            "$a"
        );
        for child in outline[0]["children"].as_array().unwrap() {
            let selected = &source[client_range(&source, &child["selectionRange"])];
            assert!(selected == "$p" || selected == "$l", "{child}");
            client_range(&source, &child["range"]);
        }
        let rename = server.request(
            "textDocument/rename",
            json!({
                "textDocument": {"uri": URI}, "position": position, "newName": "$renamed",
            }),
        );
        let mut edits: Vec<_> = rename["changes"][URI]
            .as_array()
            .unwrap()
            .iter()
            .map(|edit| {
                (
                    client_range(&source, &edit["range"]),
                    edit["newText"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(edits.len(), 2);
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let mut renamed = source.clone();
        for (range, text) in edits {
            renamed.replace_range(range, text);
        }
        assert_eq!(renamed, source.replace("$a", "$renamed"));
    }
}

#[test]
fn unicode_requests_select_the_correct_same_line_module() {
    let mut server = Server::new();
    // The accumulated byte/UTF-16 difference is larger than the second module's
    // declaration prefix. Comparing a UTF-16 cursor against byte ranges would
    // select the first module (or its fallback) instead of the second.
    let source = format!(
        "(module (; {} ;) (func $first)) (module (func $second (call $second)))",
        "😀".repeat(40)
    );
    server.open(URI, &source, 1);
    let position = client_position(&source, source.find("$second").unwrap() + 1);
    let result = server.request(
        "textDocument/rename",
        json!({
            "textDocument": {"uri": URI}, "position": position, "newName": "$fixed",
        }),
    );
    let edits = result["changes"][URI]
        .as_array()
        .expect("rename in second module");
    assert_eq!(edits.len(), 2);
    for edit in edits {
        assert_eq!(&source[client_range(&source, &edit["range"])], "$second");
    }
}

#[test]
fn unicode_diagnostics_are_utf16_and_stay_within_incomplete_documents() {
    let mut server = Server::new();
    let source = "(module (; é漢😀 ;) (func (call $missing)))";
    server.open(URI, source, 1);
    server.symbols(URI);
    server.collect_for(Duration::from_millis(750));
    assert!(
        server.diagnostics.len() >= 2,
        "immediate and deferred diagnostics"
    );
    for publication in &server.diagnostics {
        let diagnostics = publication["diagnostics"].as_array().unwrap();
        assert!(!diagnostics.is_empty());
        for diagnostic in diagnostics {
            let selected = &source[client_range(source, &diagnostic["range"])];
            if diagnostic["message"]
                .as_str()
                .unwrap()
                .contains("Undefined function")
            {
                assert_eq!(selected, "$missing");
            }
        }
    }
    for (i, source) in [
        "(module",
        "(module\n",
        "(module\r\n",
        "(; 😀 ;) (module (func",
        "(; 😀 ;) (module (func i32.const 漢))",
    ]
    .iter()
    .enumerate()
    {
        server.diagnostics.clear();
        server.change(URI, i as i32 + 2, json!([{"text": source}]));
        server.symbols(URI);
        server.collect_for(Duration::from_millis(750));
        assert!(server.diagnostics.len() >= 2);
        for publication in &server.diagnostics {
            for diagnostic in publication["diagnostics"].as_array().unwrap() {
                client_range(source, &diagnostic["range"]);
            }
        }
    }
}

#[test]
fn oversized_and_reversed_edit_ranges_do_not_kill_or_wipe_the_document() {
    let mut server = Server::new();
    server.open(URI, ";; é漢😀\r\n(module (func $kept))", 1);
    server.change(URI, 2, json!([{
        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 2147483647}}, "text": "",
    }]));
    assert_eq!(server.symbols(URI)[0]["name"], "$kept");
    // First-line overshoot followed by an ordered next-line endpoint used to panic.
    server.change(URI, 3, json!([{
        "range": {"start": {"line": 0, "character": 300}, "end": {"line": 1, "character": 0}}, "text": "",
    }]));
    assert_eq!(server.symbols(URI)[0]["name"], "$kept");
    server.change(URI, 4, json!([
        insert(0, 16, "x"),
        {"range": {"start": {"line": 1, "character": 0}, "end": {"line": 0, "character": 0}}, "text": "BAD"},
    ]));
    assert_eq!(
        server.symbols(URI)[0]["name"],
        "$kept",
        "reject the entire malformed batch"
    );
    server.change(URI, 5, json!([{"text": "(module (func $recovered))"}]));
    assert_eq!(server.symbols(URI)[0]["name"], "$recovered");
}

#[test]
fn wast_script_document_gets_no_wast_validator_errors() {
    let mut server = Server::new();
    let wast = "file:///script.wast";
    // A multi-module script fragment that single-module `wast` validation
    // misreports as an error. Under a `.wast` URI that validation is suppressed.
    let script = "(module (func) (func) (module)\n";
    server.open(wast, script, 1);
    server.symbols(wast);
    // Wait past the debounce so any (suppressed) background validation would land.
    server.collect_for(Duration::from_millis(900));
    for publication in server.diagnostics.iter().filter(|d| d["uri"] == wast) {
        for diagnostic in publication["diagnostics"].as_array().unwrap() {
            assert_ne!(
                diagnostic["source"], "wast-validator",
                "wast script must not receive single-module wast validation: {diagnostic}"
            );
        }
    }

    // The same input under a `.wat` URI DOES get a wast-validator error,
    // confirming the gate is scheme-specific rather than globally disabling it.
    let wat = "file:///script-as-wat.wat";
    server.diagnostics.clear();
    server.open(wat, script, 1);
    server.symbols(wat);
    server.collect_for(Duration::from_millis(900));
    assert!(
        server
            .diagnostics
            .iter()
            .filter(|d| d["uri"] == wat)
            .flat_map(|d| d["diagnostics"].as_array().unwrap())
            .any(|d| d["source"] == "wast-validator"),
        "expected a wast-validator error for the .wat document: {:?}",
        server.diagnostics
    );
}

#[test]
fn deeply_nested_input_stays_responsive_with_a_fallback_notice() {
    let mut server = Server::new();
    // Deep nesting drives recursive traversals; the analysis budget must skip the
    // semantic/`wast` passes and surface an informational fallback instead of
    // hanging the request path.
    let deep = format!("{}{}", "(".repeat(2000), ")".repeat(2000));
    server.open(URI, &deep, 1);
    // Requests must still return promptly while analysis is bounded.
    server.symbols(URI);
    server.collect_for(Duration::from_millis(900));
    let published: Vec<_> = server
        .diagnostics
        .iter()
        .filter(|d| d["uri"] == URI)
        .collect();
    assert!(!published.is_empty(), "expected a diagnostics publication");
    assert!(
        published.iter().all(|d| d["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|diag| diag["source"] != "wast-validator")),
        "budget-exhausted input must not run wast validation: {published:?}"
    );
    assert!(
        published.iter().any(
            |d| d["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diag| diag["severity"] == 3
                    && diag["message"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("nesting-depth"))
        ),
        "expected an informational fallback notice: {published:?}"
    );
}

#[test]
fn position_encoding_is_explicit_utf16() {
    let server = Server::new();
    assert_eq!(
        server.initialization["capabilities"]["positionEncoding"],
        "utf-16"
    );
}
