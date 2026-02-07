# Documentation Files

This directory contains documentation that is parsed at build time and embedded into the LSP server.

## instructions.md

Instruction documentation is maintained in [`packages/docs/instructions.md`](../packages/docs/instructions.md) as the single source of truth. It is used by both:

- **`build.rs`** — parsed at compile time to generate Rust hover documentation
- **`packages/docs/scripts/generate-instruction-docs.mjs`** — parsed to build the documentation site

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

### Adding New Instructions

1. Edit `packages/docs/instructions.md`
2. Find the appropriate section (arithmetic, memory, control flow, etc.)
3. Add a new entry following the format above
4. Rebuild the project: `cargo build`
5. The new documentation will automatically be available in hover tooltips and the docs site

### Build Process

When you run `cargo build`, the build script (`../build.rs`) will:

1. Read `packages/docs/instructions.md`
2. Parse each instruction section
3. Generate `instruction_docs.rs` in the build output directory
4. This file is included in `src/hover.rs` at compile time

## annotations.md

This file contains documentation for WAT annotations (e.g., `@name`, `@custom`). It follows the same format as instructions and is also parsed by `build.rs`.

## Future Documentation Files

This directory can be extended with additional documentation files:

- `types.md` - Detailed type system documentation
- `concepts.md` - WebAssembly concepts (linear memory, tables, etc.)

Each new file would need a corresponding parser in `build.rs`.
