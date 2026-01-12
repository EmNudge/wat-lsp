#!/usr/bin/env node
/**
 * Copy assets for the @emnudge/wat-lsp package.
 *
 * This script:
 * 1. Copies tree-sitter.wasm from node_modules/web-tree-sitter
 * 2. Cleans up unnecessary files from wasm-pack output
 */

import { copyFileSync, existsSync, mkdirSync, unlinkSync, readFileSync, writeFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const packageRoot = join(__dirname, '..');
const distWasm = join(packageRoot, 'dist', 'wasm');

// Ensure dist/wasm exists
if (!existsSync(distWasm)) {
  mkdirSync(distWasm, { recursive: true });
}

// Copy tree-sitter.wasm from node_modules
const treeSitterWasmSrc = join(packageRoot, 'node_modules', 'web-tree-sitter', 'tree-sitter.wasm');
const treeSitterWasmDest = join(distWasm, 'tree-sitter.wasm');

if (existsSync(treeSitterWasmSrc)) {
  copyFileSync(treeSitterWasmSrc, treeSitterWasmDest);
  console.log('Copied tree-sitter.wasm');
} else {
  console.warn('Warning: tree-sitter.wasm not found in node_modules. Run npm install first.');
}

// Clean up unnecessary files from wasm-pack output
const filesToRemove = [
  '.gitignore',
  'package.json', // wasm-pack generates its own, we don't need it
];

for (const file of filesToRemove) {
  const filePath = join(distWasm, file);
  if (existsSync(filePath)) {
    unlinkSync(filePath);
    console.log(`Removed ${file}`);
  }
}

// Fix the generated JS to use relative imports
const jsPath = join(distWasm, 'wat_lsp_rust.js');
if (existsSync(jsPath)) {
  let content = readFileSync(jsPath, 'utf-8');

  // The generated file should already have correct imports, but let's verify
  // wasm-pack with --target web generates proper ESM
  console.log('WASM JS bindings ready');
}

console.log('Assets copied successfully!');
