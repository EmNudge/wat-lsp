# WAT LSP Server

A Language Server Protocol (LSP) implementation for WebAssembly Text Format (`.wat` files) written in Rust. Works with any editor that supports LSP.

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

### 🔍 Go to Definition
Jump to symbol definitions:
- Functions, globals, types, tables
- Local variables and parameters
- Block labels
- Works with both named (`$symbol`) and numeric indices

### 🔎 Find References
Find all references to a symbol:
- Functions, globals, locals, parameters
- Block labels (including numeric depth like `br 0`)
- Types and tables
- Scope-aware for locals/parameters
- Option to include/exclude declaration

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

The server can be integrated with any LSP-compatible editor (VS Code, Neovim, Helix, Emacs, etc.). Configure your editor to launch the `wat-lsp-rust` binary for `.wat` files. See your editor's LSP documentation for specific setup instructions.

## Future Enhancements

- [ ] Symbol renaming
- [ ] Document symbols for outline view
- [ ] Code formatting
- [ ] Inlay hints for types
- [ ] Code actions and quick fixes

## License

MIT
