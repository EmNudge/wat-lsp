# WAT LSP Server (Rust Implementation)

A Language Server Protocol (LSP) implementation for WebAssembly Text Format (`.wat` files) written in Rust. This project provides the same feature set as the [wati](https://github.com/NateLevin1/wati) VSCode extension, but as a standalone LSP server that can be used with any editor that supports LSP.

## Features

### 🎯 Hover Information
- **Functions**: Complete signature with parameters and return types
- **Global Variables**: Type, mutability status, and initial values
- **Local Variables & Parameters**: Type and scope information
- **Tables**: Size limits and reference types
- **Type Definitions**: Complete function type signatures
- **Block/Loop Labels**: Label type and line number
- **Instructions**: Comprehensive WebAssembly instruction documentation
- **Numeric Indices**: Resolution of indexed variable access

### ✨ Code Completion

#### Type-Prefixed Instructions
- `i32.` → shows add, sub, mul, eq, load, store, etc.
- `i64.` → 64-bit integer operations
- `f32.` → 32-bit float operations
- `f64.` → 64-bit float operations
- `local.` → get, set, tee
- `global.` → get, set
- `memory.` → size, grow, fill, copy
- `table.` → get, set, size, grow, fill, copy

#### Emmet-like Expansions
Transform shorthand into full WebAssembly instructions:

- **Constants**:
  - `5i32` → `(i32.const 5)`
  - `30.12f64` → `(f64.const 30.12)`
  - `100_000i64` → `(i64.const 100000)`

- **Local Variables**:
  - `l$varName` → `(local.get $varName)`
  - `l=$varName` → `(local.set $varName )`

- **Global Variables**:
  - `g$varName` → `(global.get $varName)`
  - `g=$varName` → `(global.set $varName )`

#### Context-Aware Suggestions
- Function names after `call`
- Variable names after `$`
- Block labels for branch instructions
- JSDoc tags: `@param`, `@result`, `@function`, `@todo`

### 📝 Signature Help
Shows parameter information during function calls:
- Displays function signature
- Highlights current parameter position
- Works with both named and indexed parameters

## Building

### Prerequisites

This project requires `tree-sitter-cli` to be installed for generating the WAT parser at build time.

Install it with:
```bash
npm install -g tree-sitter-cli
# or
cargo install tree-sitter-cli
```

### Build

```bash
cargo build --release
```

The compiled binary will be at `target/release/wat-lsp-rust`.

### Build-Time Documentation Generation

The LSP server uses a build script to parse instruction documentation from `docs/instructions.md` at compile time. This means:

- Documentation is maintained in a readable Markdown format
- Examples can be easily added or updated without touching Rust code
- The documentation is embedded in the binary for fast lookup
- No runtime file I/O is needed for instruction docs

To add or modify instruction documentation, edit `docs/instructions.md` and rebuild. See `docs/README.md` for the documentation format.

## Usage

### VS Code

Create a `.vscode/settings.json` with:

```json
{
  "wat.server.path": "/path/to/wat-lsp-rust"
}
```

Or install a VS Code extension that uses this LSP server.

### Neovim

Using nvim-lspconfig:

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

-- Define the WAT LSP server
if not configs.wat_lsp then
  configs.wat_lsp = {
    default_config = {
      cmd = { '/path/to/wat-lsp-rust' },
      filetypes = { 'wat' },
      root_dir = lspconfig.util.root_pattern('.git'),
      settings = {},
    },
  }
end

-- Enable it
lspconfig.wat_lsp.setup{}
```

### Helix

Add to your `languages.toml`:

```toml
[[language]]
name = "wat"
language-servers = ["wat-lsp"]

[language-server.wat-lsp]
command = "/path/to/wat-lsp-rust"
```

### Emacs (eglot)

```elisp
(add-to-list 'eglot-server-programs
             '(wat-mode . ("/path/to/wat-lsp-rust")))
```

## Architecture

### Components

- **Parser** (`src/parser.rs`): Regex-based parser that extracts symbols from WAT files
- **Symbols** (`src/symbols.rs`): Data structures for functions, globals, locals, types, and tables
- **Hover** (`src/hover.rs`): Provides hover information with instruction documentation
- **Completion** (`src/completion.rs`): Context-aware completion with emmet-like expansions
- **Signature** (`src/signature.rs`): Function signature help during calls
- **Build Script** (`build.rs`): Parses `docs/instructions.md` at compile time
- **Documentation** (`docs/instructions.md`): Comprehensive WebAssembly instruction reference with examples

### Parsing Strategy

The server uses a regex-based parser that efficiently extracts:
- Function declarations with parameters, results, and locals
- Global variable declarations
- Table definitions
- Type definitions
- Block labels within functions

Results are cached per file for fast response times.

## Comparison with wati

This Rust implementation provides the same core features as wati:

| Feature | wati (TypeScript) | wat-lsp-rust |
|---------|------------------|--------------|
| Hover Information | ✅ | ✅ |
| Code Completion | ✅ | ✅ |
| Signature Help | ✅ | ✅ |
| Emmet Expansions | ✅ | ✅ |
| Parser | Tree-sitter + Regex | Regex-based |
| LSP Support | VS Code only | Any LSP client |
| Performance | Good | Excellent |
| Memory Usage | Higher | Lower |

## Future Enhancements

- [ ] Add tree-sitter parser for more accurate AST analysis
- [ ] Implement diagnostics (syntax errors, type checking)
- [ ] Add "Go to Definition" support
- [ ] Add "Find References" support
- [ ] Implement symbol renaming
- [ ] Add document symbols for outline view
- [ ] Support code formatting
- [ ] Add inlay hints for types

## Contributing

Contributions are welcome! Areas for improvement:
- Tree-sitter integration for better parsing
- Additional instruction documentation
- Performance optimizations
- More comprehensive testing

## License

MIT

## Acknowledgments

Based on the feature set of [wati](https://github.com/NateLevin1/wati) by Nate Levin.
