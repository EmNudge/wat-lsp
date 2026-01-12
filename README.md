# WAT LSP Server

[![CI](https://github.com/EmNudge/wat-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/EmNudge/wat-lsp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub stars](https://img.shields.io/github/stars/EmNudge/wat-lsp)](https://github.com/EmNudge/wat-lsp/stargazers)

A Language Server Protocol implementation for WebAssembly Text Format (`.wat` files) written in Rust.

## Features

- **Hover**: Documentation for instructions, functions, globals, locals, types, tables, and block labels
- **Completion**: Type-prefixed instructions (`i32.`, `local.`, etc.), emmet-like expansions (`5i32` → `(i32.const 5)`, `l$var` → `(local.get $var)`), context-aware suggestions
- **Signature Help**: Parameter info during function calls
- **Go to Definition**: Jump to functions, globals, types, tables, locals, and block labels
- **Find References**: Scope-aware reference finding for all symbol types
- **Rename**: Rename symbols across the file (updates both named and numeric references)

Supports WasmGC, Relaxed SIMD, Exception Handling, and Reference Types.

## Architecture

The LSP server supports two build targets:

- **Native**: Uses tree-sitter for fast, incremental parsing. Runs as a standalone LSP server.
- **WASM**: Uses web-tree-sitter for browser-compatible parsing. Runs in the browser for the playground.

Both targets share the same core code for symbol table construction and LSP features.

## Building

### Prerequisites

Install `tree-sitter-cli`:
```bash
npm install -g tree-sitter-cli  # or: cargo install tree-sitter-cli
```

Generate the tree-sitter parser (required before any build):
```bash
cd grammars/tree-sitter-wat
tree-sitter generate
cd ../..
```

### Native LSP Server

```bash
cargo build --release
```

Binary outputs to `target/release/wat-lsp-rust`.

### WASM Module

For browser usage, first build the tree-sitter WASM grammar:
```bash
cd grammars/tree-sitter-wat
tree-sitter build --wasm
cd ../..
```

Then build the Rust WASM module:
```bash
wasm-pack build --target web --features wasm --no-default-features
```

Outputs to `pkg/` directory.

Instruction documentation is parsed from `docs/instructions.md` at compile time—edit that file to update hover docs.

## Usage

### VS Code

[![VS Code Marketplace](https://img.shields.io/visual-studio-marketplace/v/EmNudge.wat-lsp)](https://marketplace.visualstudio.com/items?itemName=EmNudge.wat-lsp)
[![Open VSX](https://img.shields.io/open-vsx/v/EmNudge/wat-lsp)](https://open-vsx.org/extension/EmNudge/wat-lsp)

Or build manually:

```bash
./scripts/build-extension.sh
cd packages/vscode-extension && npm run package
code --install-extension wat-lsp-*.vsix
```

See [packages/vscode-extension/README.md](packages/vscode-extension/README.md) for details.

### Other Editors

Configure your editor to launch `wat-lsp-rust` for `.wat` files.

## Playground

Try the LSP in your browser: **[wat-lsp.emnudge.dev](https://wat-lsp.emnudge.dev)**

To build locally, first complete the [Prerequisites](#prerequisites) and [WASM Module](#wasm-module) build steps, then:

```bash
cd packages/playground
npm install
npm run setup          # Copy WASM files and dependencies
npm run build          # Build the site
npm run preview        # Preview the built site
```

For development with hot reload:

```bash
cd packages/playground
npm run dev
```

Open the URL shown by Vite. Use F12 for Go to Definition, Shift+F12 for Find References.

The playground uses the same Rust LSP code as the native server, compiled to WebAssembly.

## License

MIT
