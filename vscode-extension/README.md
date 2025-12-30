# WAT Language Server

Language Server Protocol support for WebAssembly Text Format (`.wat` and `.wast` files).

## Features

- **Hover information** - Types, signatures, and documentation
- **Code completion** - Instructions, variables, functions, and emmet-like expansions (`5i32` → `(i32.const 5)`)
- **Signature help** - Parameter hints for function calls
- **Go to definition** - Jump to functions, types, and variables
- **Diagnostics** - Real-time syntax and type error detection

## Installation

Install from [Open VSX Registry](https://open-vsx.org/extension/EmNudge/wat-lsp):

1. Open Extensions in your editor
2. Search for "WAT Language Server"
3. Click Install

## Configuration

- `watLsp.trace.server` - Debug communication (`off` / `messages` / `verbose`)
- `watLsp.serverPath` - Custom server binary path (optional)

## License

MIT
