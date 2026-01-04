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

## Building

Requires `tree-sitter-cli`:
```bash
npm install -g tree-sitter-cli  # or: cargo install tree-sitter-cli
```

Build:
```bash
cargo build --release
```

Binary outputs to `target/release/wat-lsp-rust`.

Instruction documentation is parsed from `docs/instructions.md` at compile time—edit that file to update hover docs.

## Usage

### VS Code

[![VS Code Marketplace](https://img.shields.io/visual-studio-marketplace/v/EmNudge.wat-lsp)](https://marketplace.visualstudio.com/items?itemName=EmNudge.wat-lsp)
[![Open VSX](https://img.shields.io/open-vsx/v/EmNudge/wat-lsp)](https://open-vsx.org/extension/EmNudge/wat-lsp)

Or build manually:

```bash
./build-extension.sh
cd vscode-extension && npm run package
code --install-extension wat-lsp-*.vsix
```

See [vscode-extension/README.md](vscode-extension/README.md) for details.

### Other Editors

Configure your editor to launch `wat-lsp-rust` for `.wat` files.

## License

MIT
