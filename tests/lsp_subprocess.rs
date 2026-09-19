//! Subprocess LSP protocol tests (native only).
//!
//! Unlike `lsp_protocol.rs`, which drives the `LspService` in-process, these
//! tests spawn the real `wat-lsp-rust` binary and talk to it over stdio using
//! the actual `Content-Length` framing. They exercise process lifecycle and
//! transport failure modes — exit status, EOF, malformed JSON, and malformed or
//! truncated framing — with bounded timeouts so a hung server fails the test
//! instead of blocking forever.

#![cfg(feature = "native")]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Absolute path to the built server binary, provided by Cargo.
const SERVER_BIN: &str = env!("CARGO_BIN_EXE_wat-lsp-rust");

/// Generous per-read/exit timeout. Real work is sub-millisecond; this only
/// guards against a hang.
const TIMEOUT: Duration = Duration::from_secs(10);

/// A spawned server plus channels that decouple the test thread from the
/// child's pipes so every read/wait can be bounded.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    messages: Receiver<String>,
    reader: Option<JoinHandle<()>>,
    stderr: Option<JoinHandle<String>>,
}

/// Wrap a JSON body in LSP `Content-Length` framing.
fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

/// Read one framed LSP message (headers + body) from `reader`.
/// Returns `None` on EOF or malformed headers.
fn read_message<R: BufRead>(reader: &mut R) -> Option<String> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None; // EOF before headers completed
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // blank line terminates headers
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().ok();
        }
    }

    let len = content_length?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

impl Server {
    fn spawn() -> Server {
        let mut child = Command::new(SERVER_BIN)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn server binary");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Forward each framed stdout message onto a channel so reads can be
        // bounded with `recv_timeout`.
        let (tx, rx) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut buf = BufReader::new(stdout);
            while let Some(msg) = read_message(&mut buf) {
                if tx.send(msg).is_err() {
                    break;
                }
            }
        });

        // Drain stderr so the child never blocks on a full pipe, and capture it
        // for assertions.
        let stderr = thread::spawn(move || {
            let mut s = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut s);
            s
        });

        Server {
            child,
            stdin: Some(stdin),
            messages: rx,
            reader: Some(reader),
            stderr: Some(stderr),
        }
    }

    fn send(&mut self, body: &str) {
        self.send_raw(frame(body).as_bytes());
    }

    /// Send raw bytes (possibly unframed, for malformed-input tests).
    fn send_raw(&mut self, bytes: &[u8]) {
        let stdin = self.stdin.as_mut().expect("stdin already closed");
        stdin.write_all(bytes).expect("write to server stdin");
        stdin.flush().expect("flush server stdin");
    }

    /// Wait for the next message whose body contains `needle`, skipping others.
    /// The server interleaves notifications (e.g. `window/logMessage` from the
    /// `initialized` handler) with request responses, so tests must match on the
    /// message they care about rather than assume ordering. Bounded by `TIMEOUT`.
    fn recv_matching(&self, needle: &str) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let msg = self.messages.recv_timeout(remaining).unwrap_or_else(|_| {
                panic!("timed out waiting for a message containing {needle:?}")
            });
            if msg.contains(needle) {
                return msg;
            }
        }
    }

    /// Close the server's stdin (client hung up / sent EOF).
    fn close_stdin(&mut self) {
        self.stdin.take(); // dropping the only handle closes the pipe
    }

    /// Wait for the process to exit, bounded by `TIMEOUT`. Kills and panics on
    /// timeout so a hung server fails loudly.
    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                return status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                panic!("server did not exit within {TIMEOUT:?}");
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Join the stderr collector and return everything the server wrote there.
    fn take_stderr(&mut self) -> String {
        self.stderr
            .take()
            .map(|h| h.join().unwrap())
            .unwrap_or_default()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn initialize_body(id: i64) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"initialize","params":{{"capabilities":{{}},"processId":null,"rootUri":null}}}}"#
    )
}

/// A full, correctly-sequenced handshake terminates with exit code 0.
#[test]
fn lifecycle_initialize_shutdown_exit_is_clean() {
    let mut server = Server::spawn();

    server.send(&initialize_body(1));
    let init_response = server.recv_matching("\"id\":1");
    assert!(
        init_response.contains("\"result\""),
        "initialize should succeed, got: {init_response}"
    );

    server.send(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);

    server.send(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#);
    let shutdown_response = server.recv_matching("\"id\":2");
    assert!(
        shutdown_response.contains("\"result\""),
        "shutdown should succeed, got: {shutdown_response}"
    );

    server.send(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    server.close_stdin(); // a real client closes the connection after `exit`

    let status = server.wait();
    assert!(
        status.success(),
        "clean shutdown+exit should exit 0, got {status:?}"
    );
}

/// `exit` without a preceding `shutdown` is a protocol error: exit code 1.
#[test]
fn exit_without_shutdown_is_error_status() {
    let mut server = Server::spawn();

    server.send(&initialize_body(1));
    let _ = server.recv_matching("\"id\":1");

    server.send(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    server.close_stdin();

    let status = server.wait();
    assert!(
        !status.success(),
        "exit without shutdown should be a non-zero exit, got {status:?}"
    );
    assert_eq!(status.code(), Some(1));
}

/// Losing the input stream (EOF) before `shutdown` is a transport failure: the
/// server exits 1 and says why on stderr, never on stdout.
#[test]
fn eof_without_shutdown_is_error_status() {
    let mut server = Server::spawn();

    server.close_stdin();

    let status = server.wait();
    assert_eq!(
        status.code(),
        Some(1),
        "EOF without shutdown should exit 1, got {status:?}"
    );

    let stderr = server.take_stderr();
    assert!(
        stderr.contains("without a shutdown request"),
        "expected a stderr diagnostic, got: {stderr:?}"
    );
}

/// A well-framed message with an invalid JSON body must surface a protocol-level
/// Parse error (never silent, never raw text on stdout) and, because the
/// underlying transport ends the stream on a decode error, terminate with an
/// error status rather than pretend success.
///
/// (tower-lsp 0.20's `FramedRead` treats a decode error as end-of-stream, so a
/// malformed message is not recoverable at the transport layer today. This test
/// pins that behavior; fixing in-session recovery would require replacing the
/// transport, which is explicitly out of scope for this change.)
#[test]
fn malformed_json_surfaces_parse_error_and_exits_nonzero() {
    let mut server = Server::spawn();

    server.send(&initialize_body(1));
    let _ = server.recv_matching("\"id\":1");

    // Valid framing, invalid JSON payload.
    server.send("{ this is not valid json ]");

    // The failure is reported over the protocol as a JSON-RPC parse error — a
    // well-framed message on stdout, not raw diagnostic text.
    let error_response = server.recv_matching("-32700");
    assert!(
        error_response.contains("Parse error"),
        "malformed JSON should produce a JSON-RPC parse error, got: {error_response}"
    );

    server.close_stdin();
    let status = server.wait();
    assert_eq!(
        status.code(),
        Some(1),
        "a malformed message without a prior shutdown must exit 1, got {status:?}"
    );
}

/// A truncated frame (header promises more bytes than are delivered, then EOF)
/// must terminate in bounded time with an error status rather than hang.
#[test]
fn truncated_framing_terminates_with_error() {
    let mut server = Server::spawn();

    // Claim 4096 bytes but send only a few, then hang up.
    server.send_raw(b"Content-Length: 4096\r\n\r\n{\"partial\":");
    server.close_stdin();

    let status = server.wait();
    assert_eq!(
        status.code(),
        Some(1),
        "truncated framing then EOF should exit 1, got {status:?}"
    );
}
