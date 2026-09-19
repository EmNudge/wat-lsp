# WAT LSP Server

[![CI](https://github.com/EmNudge/wat-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/EmNudge/wat-lsp/actions/workflows/ci.yml)

A Language Server for WebAssembly Text Format (`.wat` files) written in Rust.

**[Try it in your browser →](https://wat-lsp.emnudge.dev)**

## Features

Hover, completions, signature help, go to definition, find references, and rename.

Supports WasmGC, Relaxed SIMD, Exception Handling, Reference Types, Wide Arithmetic, and Custom Page Sizes.

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

The tree-sitter parser (`grammars/tree-sitter-wat/src/parser.c`) is generated,
not committed, so building from a clean checkout requires the `tree-sitter-cli`.
Pin the same version CI uses to keep generated output reproducible:

```bash
npm install -g tree-sitter-cli@0.27.0
```

```bash
# Build native LSP server. build.rs runs `tree-sitter generate` automatically,
# so no manual generation step is needed for native builds.
cargo build --release  # outputs to target/release/wat-lsp-rust

# Build WASM module (for browser). WASM builds skip the native grammar
# compilation, so generate the parser explicitly first.
cd grammars/tree-sitter-wat && tree-sitter generate && tree-sitter build --wasm && cd ../..
wasm-pack build --target web --features wasm --no-default-features
```

### Verifying a checkout

With `tree-sitter-cli@0.27.0` on `PATH`, a clean checkout reproduces CI's
formatting, linting, and test results:

```bash
cargo fmt --all --check
cargo clippy --features native --all-targets -- -D warnings
cargo clippy --features wasm --no-default-features --lib -- -D warnings
cargo test --features native
```

## License

MIT
