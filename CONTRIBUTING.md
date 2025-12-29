# Contributing to WAT LSP Server

Thank you for your interest in contributing! This document provides guidelines for contributing to the project.

## Development Setup

1. **Prerequisites**:
   - Rust 1.70 or later
   - Cargo (comes with Rust)

2. **Clone and build**:
   ```bash
   git clone <repository-url>
   cd wat-lsp-rust
   cargo build
   ```

3. **Run tests**:
   ```bash
   cargo test
   ```

4. **Build release version**:
   ```bash
   cargo build --release
   ```

## Project Structure

```
wat-lsp-rust/
├── src/
│   ├── main.rs                  # LSP server entry point
│   ├── parser.rs                # WAT parser (tree-sitter-based)
│   ├── tree_sitter_bindings.rs  # Tree-sitter FFI bindings
│   ├── symbols.rs               # Symbol table structures
│   ├── hover.rs                 # Hover provider
│   ├── completion.rs            # Completion provider
│   ├── signature.rs             # Signature help provider
│   ├── diagnostics.rs           # Error detection and reporting
│   └── utils.rs                 # Shared utilities
├── docs/
│   ├── instructions.md   # Instruction documentation (parsed at build)
│   └── README.md         # Documentation format guide
├── build.rs              # Build script for doc generation
├── Cargo.toml            # Dependencies
└── example.wat           # Example WAT file for testing
```

## Adding Instruction Documentation

The easiest way to contribute is by improving instruction documentation:

1. **Edit** `docs/instructions.md`
2. **Follow the format** described in `docs/README.md`:
   ```markdown
   ## instruction.name
   Description here.

   Signature: `(param types) (result types)`

   Example:
   \`\`\`wat
   (instruction.name ...)
   \`\`\`
   ---
   ```
3. **Rebuild** to test:
   ```bash
   cargo build
   ```
4. **Verify** your changes work by testing hover on the instruction in a `.wat` file

### Documentation Guidelines

- **Be concise**: 1-2 sentences for the description
- **Include signatures**: Show parameter and result types
- **Provide examples**: Practical, working code snippets
- **Add comments**: Explain what the code does or returns
- **Show edge cases**: Division by zero, overflow, special values (NaN, inf)

## Adding New Features

### Adding a New LSP Feature

1. **Update server capabilities** in `src/main.rs::initialize()`
2. **Implement the handler** in the `LanguageServer` trait
3. **Create a provider module** if needed (like `src/hover.rs`)
4. **Test** with a real editor client

### Improving the Parser

The parser uses tree-sitter for robust AST-based parsing. Improvements could include:

- Enhanced error recovery and diagnostics
- More advanced semantic analysis
- Support for inline module syntax
- Multi-file/import support
- Advanced scope tracking for nested blocks

When working with tree-sitter:
1. Tree-sitter WAT grammar is in `tree-sitter-wasm/` submodule
2. Parser logic is in `src/parser.rs` with AST traversal
3. Bindings are in `src/tree_sitter_bindings.rs`
4. Build script in `build.rs` compiles the grammar from C source

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Run Clippy for linting (`cargo clippy`)
- Add comments for complex logic
- Keep functions focused and single-purpose

## Testing

### Manual Testing

1. **Build the server**:
   ```bash
   cargo build
   ```

2. **Configure your editor** to use the binary

3. **Test features**:
   - Hover over instructions, functions, variables
   - Type completions (try `i32.`, `l$`, `5i32`)
   - Function signature help (type `call $function(`)

### Automated Testing

We welcome contributions for automated tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function() {
        let wat = "(func $add (param i32 i32) (result i32) ...)";
        let symbols = parse_document(wat).unwrap();
        assert_eq!(symbols.functions.len(), 1);
        assert_eq!(symbols.functions[0].name, Some("$add".to_string()));
    }
}
```

## Common Tasks

### Adding a New Completion Trigger

1. Edit `src/main.rs` to add trigger character in `initialize()`
2. Add handler logic in `src/completion.rs::provide_completion()`

### Adding Hover Information

1. Edit `src/hover.rs::provide_hover()`
2. Add new context detection
3. Return appropriate `Hover` with markdown content

### Fixing Parser Issues

1. Identify the AST traversal logic in `src/parser.rs`
2. Update the tree-sitter node queries and extraction logic
3. Test with various WAT syntax variations
4. Consider edge cases (comments, nested structures, etc.)
5. Use `tree.root_node()` to debug AST structure
6. Check `diagnostics.rs` for error node detection

## Pull Request Process

1. **Fork** the repository
2. **Create a branch** for your feature/fix
3. **Make your changes**
4. **Test** thoroughly
5. **Update documentation** if needed
6. **Submit a PR** with:
   - Clear description of changes
   - Motivation/reasoning
   - Test results
   - Breaking changes (if any)

## Areas for Contribution

### High Priority
- [ ] Add "Go to Definition" support
- [ ] Implement advanced diagnostics (type checking, semantic validation)
- [ ] Add "Find References" support
- [ ] Implement symbol renaming

### Medium Priority
- [ ] Add document symbols (outline view)
- [ ] Support workspace symbols
- [ ] Add code formatting
- [ ] Improve incremental parsing performance

### Low Priority
- [ ] Add inlay hints for types
- [ ] Implement code actions (quick fixes)
- [ ] Add semantic highlighting
- [ ] Support multi-file projects
- [ ] Add snippet library

### Documentation
- [ ] Add more instruction examples
- [ ] Create tutorial/walkthrough
- [ ] Add troubleshooting guide
- [ ] Document common patterns
- [ ] Add video demonstrations

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help newcomers learn
- Credit others' work

## Questions?

- **Issues**: Open an issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions
- **Documentation**: Check `docs/` and `README.md` first

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (MIT).
