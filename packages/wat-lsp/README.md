# @emnudge/wat-lsp

WebAssembly Text Format (WAT) Language Server - WASM build for browser and Node.js.

## Installation

```bash
npm install @emnudge/wat-lsp
```

## Usage

### Basic Usage

```javascript
import { createWatLSP } from '@emnudge/wat-lsp';

// Create and initialize the LSP
const lsp = await createWatLSP();

// Parse a WAT document
lsp.parse(`
  (module
    (func $add (param $a i32) (param $b i32) (result i32)
      local.get $a
      local.get $b
      i32.add
    )
    (export "add" (func $add))
  )
`);

// Get hover information
const hover = lsp.provideHover(2, 12); // line 2, column 12
console.log(hover?.contents.value);

// Get diagnostics (syntax errors)
const diagnostics = lsp.provideDiagnostics();

// Go to definition
const definition = lsp.provideDefinition(5, 16);

// Find references
const references = lsp.provideReferences(2, 12, true);
```

### With Custom WASM Paths

When bundling for a web application, you may need to serve the WASM files from a specific location:

```javascript
import { createWatLSP, assets } from '@emnudge/wat-lsp';

// Use custom paths
const lsp = await createWatLSP({
  treeSitterWasmPath: '/assets/tree-sitter.wasm',
  watLspWasmPath: '/assets/wat_lsp_rust_bg.wasm',
});

// Or use the bundled asset URLs directly
console.log(assets.treeSitterWasm); // URL to bundled tree-sitter.wasm
console.log(assets.watLspWasm);     // URL to bundled wat_lsp_rust_bg.wasm
```

### Using the WatLanguageServer Class

For a more LSP-like interface:

```javascript
import { WatLanguageServer } from '@emnudge/wat-lsp';

const server = new WatLanguageServer();
await server.initialize();

// Update document
server.updateDocument('(module (func $foo))');

// Use LSP-style methods
const hover = server.provideHover(0, 15);
const diagnostics = server.provideDiagnostics();
```

### Low-level WASM Access

For advanced usage, you can access the raw WASM bindings:

```javascript
import { WatLSP, initWasm } from '@emnudge/wat-lsp';
import { Parser } from 'web-tree-sitter';

// Initialize tree-sitter yourself
await Parser.init({
  locateFile: (file) => `/my-path/${file}`,
});

// Initialize the WASM module
await initWasm('/my-path/wat_lsp_rust_bg.wasm');

// Create and use WatLSP directly
const lsp = new WatLSP();
await lsp.initialize();
```

## Features

- **Hover**: Get documentation for WAT instructions and symbols
- **Go to Definition**: Jump to function, global, and type definitions
- **Find References**: Find all usages of a symbol
- **Diagnostics**: Syntax error detection using `wast` parser
- **Semantic Tokens**: Tree-sitter based syntax highlighting

## Requirements

- Modern browser with WebAssembly support, or Node.js 16+
- `web-tree-sitter` (peer dependency)

## License

MIT
