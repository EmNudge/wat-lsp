# Documentation Files

This directory contains documentation that is parsed at build time and embedded into the LSP server.

## instructions.md

This file contains comprehensive documentation for WebAssembly instructions. It is parsed by `build.rs` during compilation and converted into a Rust HashMap for fast lookup during hover operations.

### Format

Each instruction is documented using the following format:

```markdown
## instruction.name
Brief description of what the instruction does.

Signature: `(param types) (result types)`

Example:
\`\`\`wat
example code here
\`\`\`
---
```

**Important**:
- Each entry must start with `## ` followed by the instruction name
- Sections are separated by `---` on its own line
- The signature line is optional but recommended
- Examples should use WAT syntax highlighting
- Include comments in examples to explain behavior

### Example Entry

```markdown
## i32.add
Add two i32 values.

Signature: `(param i32 i32) (result i32)`

Example:
\`\`\`wat
(i32.add (i32.const 5) (i32.const 3))  ;; Returns 8
\`\`\`
---
```

### Adding New Instructions

1. Find the appropriate section in `instructions.md` (arithmetic, memory, control flow, etc.)
2. Add a new entry following the format above
3. Rebuild the project: `cargo build`
4. The new documentation will automatically be available in hover tooltips

### Special Instructions

Some instructions have variant forms:

- **Type-prefixed**: `i32.add`, `i64.add`, `f32.add`, `f64.add`
- **Size variants**: `i32.load8_s`, `i32.load8_u`, `i32.load16_s`, etc.
- **Signedness**: `i32.div_s` (signed) vs `i32.div_u` (unsigned)

Document each variant separately for clarity.

### Documentation Best Practices

1. **Start with a clear, concise description** (1-2 sentences)
2. **Include signature** showing parameter and result types
3. **Provide practical examples** that demonstrate common usage
4. **Add comments** in code examples to explain results
5. **Show edge cases** when relevant (division by zero, overflow, etc.)
6. **Reference related instructions** when helpful

### Build Process

When you run `cargo build`, the build script (`../build.rs`) will:

1. Read `instructions.md`
2. Parse each instruction section
3. Generate `instruction_docs.rs` in the build output directory
4. This file is included in `src/hover.rs` at compile time

### Modifying Documentation

After editing `instructions.md`:

```bash
# Clean build to ensure regeneration
cargo clean

# Build to regenerate docs
cargo build

# Or just rebuild (build.rs will detect changes)
cargo build
```

The build script is set up with `println!("cargo:rerun-if-changed=docs/instructions.md");` so Cargo will automatically rerun it when the file changes.

## Future Documentation Files

This directory can be extended with additional documentation files:

- `types.md` - Detailed type system documentation
- `concepts.md` - WebAssembly concepts (linear memory, tables, etc.)
- `examples.md` - Full working examples
- `troubleshooting.md` - Common errors and solutions

Each new file would need a corresponding parser in `build.rs`.
