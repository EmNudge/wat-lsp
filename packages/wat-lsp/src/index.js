/**
 * @emnudge/wat-lsp - WebAssembly Text Format Language Server
 *
 * A WASM-based language server for WAT files that runs in the browser or Node.js.
 */

import { Parser } from 'web-tree-sitter';

// Re-export the raw WASM bindings for advanced usage
export { WatLSP } from './wasm/wat_lsp_rust.js';
export { default as initWasm } from './wasm/wat_lsp_rust.js';

/**
 * Path helpers for locating bundled WASM assets.
 * Use these when you need to serve the WASM files from a specific location.
 */
export const assets = {
  /**
   * Get the path to tree-sitter.wasm relative to this package.
   * You'll need to serve this file and configure web-tree-sitter to find it.
   */
  treeSitterWasm: new URL('./wasm/tree-sitter.wasm', import.meta.url).href,

  /**
   * Get the path to the main wat-lsp WASM module.
   */
  watLspWasm: new URL('./wasm/wat_lsp_rust_bg.wasm', import.meta.url).href,
};

/**
 * Create and initialize a WAT Language Server instance.
 *
 * This is a convenience function that handles all the initialization steps:
 * 1. Initialize web-tree-sitter runtime
 * 2. Initialize the wat-lsp WASM module
 * 3. Create and initialize the WatLSP instance
 *
 * @param {Object} options - Configuration options
 * @param {string} [options.treeSitterWasmPath] - Custom path to tree-sitter.wasm
 * @param {string} [options.watLspWasmPath] - Custom path to wat_lsp_rust_bg.wasm
 * @param {boolean} [options.skipTreeSitterInit=false] - Skip tree-sitter init if already done
 * @returns {Promise<import('./wasm/wat_lsp_rust.js').WatLSP>} Initialized WatLSP instance
 *
 * @example
 * // Basic usage
 * const lsp = await createWatLSP();
 * lsp.parse('(module (func $add (param i32 i32) (result i32)))');
 * const hover = lsp.provideHover(0, 15);
 *
 * @example
 * // With custom paths (e.g., when bundling for a web app)
 * const lsp = await createWatLSP({
 *   treeSitterWasmPath: '/assets/tree-sitter.wasm',
 *   watLspWasmPath: '/assets/wat_lsp_rust_bg.wasm'
 * });
 */
export async function createWatLSP(options = {}) {
  const {
    treeSitterWasmPath,
    watLspWasmPath,
    skipTreeSitterInit = false,
  } = options;

  // Step 1: Initialize web-tree-sitter (if not already done)
  if (!skipTreeSitterInit) {
    await Parser.init({
      locateFile: (file) => {
        if (file === 'tree-sitter.wasm') {
          return treeSitterWasmPath || assets.treeSitterWasm;
        }
        return file;
      },
    });
  }

  // Step 2: Initialize the WASM module
  const { default: initWasm, WatLSP } = await import('./wasm/wat_lsp_rust.js');
  await initWasm(watLspWasmPath || assets.watLspWasm);

  // Step 3: Create and initialize the LSP instance
  const lsp = new WatLSP();
  const success = await lsp.initialize();

  if (!success) {
    throw new Error('Failed to initialize WAT LSP');
  }

  return lsp;
}

/**
 * LSP-like message handler wrapper.
 * Provides a JSON-RPC style interface for the WAT LSP.
 *
 * Note: This is a simplified wrapper, not a full LSP implementation.
 * For a complete LSP server, you'd need to handle all LSP methods and lifecycle.
 */
export class WatLanguageServer {
  /** @type {import('./wasm/wat_lsp_rust.js').WatLSP | null} */
  #lsp = null;

  /** @type {string} */
  #currentDocument = '';

  /**
   * Check if the server is ready
   */
  get ready() {
    return this.#lsp?.ready ?? false;
  }

  /**
   * Initialize the language server
   * @param {Object} options - Same options as createWatLSP
   */
  async initialize(options = {}) {
    this.#lsp = await createWatLSP(options);
    return { capabilities: this.getCapabilities() };
  }

  /**
   * Get server capabilities
   */
  getCapabilities() {
    return {
      hoverProvider: true,
      definitionProvider: true,
      referencesProvider: true,
      documentSymbolProvider: false, // Not implemented yet
      completionProvider: false, // Uses static completions
      semanticTokensProvider: {
        legend: this.#lsp?.getSemanticTokensLegend() ?? { tokenTypes: [], tokenModifiers: [] },
        full: true,
      },
    };
  }

  /**
   * Update the document content
   * @param {string} content - The new document content
   */
  updateDocument(content) {
    this.#currentDocument = content;
    this.#lsp?.parse(content);
  }

  /**
   * Provide hover information
   * @param {number} line - 0-indexed line number
   * @param {number} character - 0-indexed character position
   * @returns {Object|null} Hover result or null
   */
  provideHover(line, character) {
    return this.#lsp?.provideHover(line, character) ?? null;
  }

  /**
   * Provide go-to-definition
   * @param {number} line - 0-indexed line number
   * @param {number} character - 0-indexed character position
   * @returns {Object|null} Definition location or null
   */
  provideDefinition(line, character) {
    return this.#lsp?.provideDefinition(line, character) ?? null;
  }

  /**
   * Provide find-references
   * @param {number} line - 0-indexed line number
   * @param {number} character - 0-indexed character position
   * @param {boolean} includeDeclaration - Whether to include the declaration
   * @returns {Array} Array of reference locations
   */
  provideReferences(line, character, includeDeclaration = true) {
    return this.#lsp?.provideReferences(line, character, includeDeclaration) ?? [];
  }

  /**
   * Provide diagnostics for the current document
   * @returns {Array} Array of diagnostic objects
   */
  provideDiagnostics() {
    return this.#lsp?.provideDiagnostics() ?? [];
  }

  /**
   * Provide semantic tokens for syntax highlighting
   * @returns {Uint32Array} Delta-encoded semantic tokens
   */
  provideSemanticTokens() {
    return this.#lsp?.provideSemanticTokens() ?? new Uint32Array(0);
  }

  /**
   * Get the semantic tokens legend
   * @returns {Object} Token types and modifiers
   */
  getSemanticTokensLegend() {
    return this.#lsp?.getSemanticTokensLegend() ?? { tokenTypes: [], tokenModifiers: [] };
  }
}
