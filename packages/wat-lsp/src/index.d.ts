/**
 * @emnudge/wat-lsp - WebAssembly Text Format Language Server
 */

export { WatLSP } from './wasm/wat_lsp_rust.js';
export { default as initWasm } from './wasm/wat_lsp_rust.js';

/**
 * LSP Position
 */
export interface Position {
  line: number;
  character: number;
}

/**
 * LSP Range
 */
export interface Range {
  start: Position;
  end: Position;
}

/**
 * Hover result from the LSP
 */
export interface HoverResult {
  contents: {
    kind: 'markdown' | 'plaintext';
    value: string;
  };
  range?: Range;
}

/**
 * Definition result from the LSP
 */
export interface DefinitionResult {
  range: Range;
}

/**
 * Reference result from the LSP
 */
export interface ReferenceResult {
  range: Range;
}

/**
 * Diagnostic from the LSP
 */
export interface Diagnostic {
  range: Range;
  message: string;
  severity: 1 | 2 | 3 | 4; // Error, Warning, Info, Hint
}

/**
 * Semantic tokens legend
 */
export interface SemanticTokensLegend {
  tokenTypes: string[];
  tokenModifiers: string[];
}

/**
 * Server capabilities
 */
export interface ServerCapabilities {
  hoverProvider: boolean;
  definitionProvider: boolean;
  referencesProvider: boolean;
  documentSymbolProvider: boolean;
  completionProvider: boolean;
  semanticTokensProvider: {
    legend: SemanticTokensLegend;
    full: boolean;
  };
}

/**
 * Path helpers for locating bundled WASM assets
 */
export const assets: {
  /** URL to tree-sitter.wasm */
  treeSitterWasm: string;
  /** URL to wat_lsp_rust_bg.wasm */
  watLspWasm: string;
};

/**
 * Options for createWatLSP
 */
export interface CreateWatLSPOptions {
  /** Custom path to tree-sitter.wasm */
  treeSitterWasmPath?: string;
  /** Custom path to wat_lsp_rust_bg.wasm */
  watLspWasmPath?: string;
  /** Skip tree-sitter initialization if already done */
  skipTreeSitterInit?: boolean;
}

/**
 * Create and initialize a WAT Language Server instance.
 *
 * @example
 * const lsp = await createWatLSP();
 * lsp.parse('(module (func $add (param i32 i32) (result i32)))');
 * const hover = lsp.provideHover(0, 15);
 */
export function createWatLSP(options?: CreateWatLSPOptions): Promise<WatLSP>;

/**
 * WAT Language Server instance (from WASM)
 */
export interface WatLSP {
  /** Check if the LSP is ready */
  readonly ready: boolean;

  /** Initialize the LSP (loads tree-sitter) */
  initialize(): Promise<boolean>;

  /** Parse a WAT document */
  parse(document: string): void;

  /** Provide hover information at position */
  provideHover(line: number, col: number): HoverResult | null;

  /** Provide go-to-definition at position */
  provideDefinition(line: number, col: number): DefinitionResult | null;

  /** Provide find-references at position */
  provideReferences(
    line: number,
    col: number,
    includeDeclaration: boolean
  ): ReferenceResult[];

  /** Provide diagnostics for the current document */
  provideDiagnostics(): Diagnostic[];

  /** Provide semantic tokens for syntax highlighting */
  provideSemanticTokens(): Uint32Array;

  /** Get the semantic tokens legend */
  getSemanticTokensLegend(): SemanticTokensLegend;

  /** Get symbol table as HTML (for debugging) */
  getSymbolTableHTML(): string;

  /** Debug: get word info at position */
  debugWordAt(line: number, col: number): {
    word: string | null;
    hasFunction?: boolean;
    functionRange?: string;
    functionLine?: number | null;
  };
}

/**
 * LSP-like message handler wrapper.
 * Provides a cleaner interface for the WAT LSP.
 */
export class WatLanguageServer {
  /** Check if the server is ready */
  readonly ready: boolean;

  /** Initialize the language server */
  initialize(options?: CreateWatLSPOptions): Promise<{ capabilities: ServerCapabilities }>;

  /** Get server capabilities */
  getCapabilities(): ServerCapabilities;

  /** Update the document content */
  updateDocument(content: string): void;

  /** Provide hover information */
  provideHover(line: number, character: number): HoverResult | null;

  /** Provide go-to-definition */
  provideDefinition(line: number, character: number): DefinitionResult | null;

  /** Provide find-references */
  provideReferences(
    line: number,
    character: number,
    includeDeclaration?: boolean
  ): ReferenceResult[];

  /** Provide diagnostics */
  provideDiagnostics(): Diagnostic[];

  /** Provide semantic tokens */
  provideSemanticTokens(): Uint32Array;

  /** Get semantic tokens legend */
  getSemanticTokensLegend(): SemanticTokensLegend;
}
