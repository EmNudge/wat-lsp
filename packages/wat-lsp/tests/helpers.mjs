/**
 * Shared test helper: initialize the WatLSP WASM module for Node.js.
 *
 * Re-used by both the E2E and parity test suites.
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Parser } from 'web-tree-sitter';

const __dirname = dirname(fileURLToPath(import.meta.url));

let _lspModule = null;

/**
 * Initialize the WASM module (idempotent).
 * Returns { WatLSP } constructor.
 */
export async function initModule() {
  if (_lspModule) return _lspModule;

  const distWasmDir = join(__dirname, '..', 'dist', 'wasm');
  const treeSitterWasmPath = join(distWasmDir, 'tree-sitter.wasm');
  const watLspWasmPath = join(distWasmDir, 'wat_lsp_rust_bg.wasm');

  // Init web-tree-sitter runtime
  await Parser.init({
    locateFile: (file) =>
      file === 'tree-sitter.wasm'
        ? `file://${treeSitterWasmPath}`
        : file,
  });

  // Load the wat-lsp WASM module
  const mod = await import('../dist/wasm/wat_lsp_rust.js');
  const wasmBuffer = readFileSync(watLspWasmPath);
  await mod.default(wasmBuffer);

  _lspModule = mod;
  return mod;
}

/**
 * Create and initialize a fresh WatLSP instance.
 */
export async function createLSP() {
  const { WatLSP } = await initModule();
  const lsp = new WatLSP();
  const ok = await lsp.initialize();
  if (!ok) throw new Error('WatLSP.initialize() failed');
  return lsp;
}
