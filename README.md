# WAT LSP Server

[![CI](https://github.com/EmNudge/wat-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/EmNudge/wat-lsp/actions/workflows/ci.yml)

A Language Server for WebAssembly Text Format (`.wat` files) written in Rust.

**[Try it in your browser →](https://wat-lsp.emnudge.dev)**

## Features

Hover, completions, signature help, go to definition, find references, and rename.

Supports WasmGC, Relaxed SIMD, Exception Handling, and Reference Types.

## Install

**VS Code**: Install from the [Marketplace](https://marketplace.visualstudio.com/items?itemName=EmNudge.wat-lsp) or [Open VSX](https://open-vsx.org/extension/EmNudge/wat-lsp).

**Other editors**: Configure to launch `wat-lsp-rust` for `.wat` files.

## Packages

| Package | Description |
|---------|-------------|
| [`packages/wat-lsp`](packages/wat-lsp) | WASM build of the LSP for browser and Node.js (`@emnudge/wat-lsp`) |
| [`packages/vscode-extension`](packages/vscode-extension) | VS Code extension |
| [`packages/playground`](packages/playground) | Browser-based [playground](https://wat-lsp.emnudge.dev) |
| [`packages/docs`](packages/docs) | Documentation site |

## Building

Requires `tree-sitter-cli` (`npm install -g tree-sitter-cli`).

```bash
# Generate parser (required first)
cd grammars/tree-sitter-wat && tree-sitter generate && cd ../..

# Build native LSP server
cargo build --release  # outputs to target/release/wat-lsp-rust

# Build WASM module (for browser)
cd grammars/tree-sitter-wat && tree-sitter build --wasm && cd ../..
wasm-pack build --target web --features wasm --no-default-features
```

## License

MIT
