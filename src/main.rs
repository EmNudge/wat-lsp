use std::sync::atomic::Ordering;

use tower_lsp::{LspService, Server};
use wat_lsp_rust::native::Backend;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    // Grab the shutdown flag before `serve` consumes the service.
    let shutdown = service.inner().shutdown_flag();

    Server::new(stdin, stdout, socket).serve(service).await;

    // The serve loop ends on a clean `exit`, on EOF, or when malformed framing
    // terminates the codec — `serve` cannot tell us which. Per the LSP spec the
    // process exits 0 only if a `shutdown` request was received first; any other
    // way of ending the stream is a transport failure that must be observable
    // via a non-zero exit code. Diagnostics go to stderr so stdout stays a clean
    // LSP wire channel.
    if shutdown.load(Ordering::SeqCst) {
        std::process::exit(0);
    } else {
        eprintln!(
            "wat-lsp: input stream ended without a shutdown request; exiting with error status"
        );
        std::process::exit(1);
    }
}
