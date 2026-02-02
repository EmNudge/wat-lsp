// WAT Playground - Main Application with LSP Integration
// Powered by @emnudge/wat-lsp

import * as monaco from 'monaco-editor';
import wabtInit from 'wabt';
import { watLanguage } from './wat-language';
import { examples, getExampleById, getDefaultExample } from './examples';
import { createWatLSP } from '@emnudge/wat-lsp';
import * as vsctm from 'vscode-textmate';
import * as oniguruma from 'vscode-oniguruma';

// Types for external libraries
interface WabtModule {
  parseWat(
    filename: string,
    source: string,
    options?: Record<string, boolean>
  ): WabtWatModule;
}

interface WabtWatModule {
  validate(): void;
  toBinary(options?: { log?: boolean; write_debug_names?: boolean }): {
    buffer: Uint8Array;
  };
  destroy(): void;
}

interface WatLSP {
  ready: boolean;
  parse(source: string): void;
  provideHover(
    line: number,
    col: number
  ): {
    contents: { value: string };
    range?: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  } | null;
  provideDefinition(
    line: number,
    col: number
  ): {
    range: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  } | null;
  provideReferences(
    line: number,
    col: number,
    includeDeclaration: boolean
  ): Array<{
    range: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  }>;
  provideDiagnostics(): Array<{
    range: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
    message: string;
    severity: number;
  }>;
  provideCompletion(
    line: number,
    col: number
  ): Array<{
    label: string;
    kind?: number;
    detail?: string;
    insertText?: string;
    insertTextRules?: number;
    documentation?: string;
  }>;
  getSymbolTableHTML(): string;
  getSemanticTokensLegend(): monaco.languages.SemanticTokensLegend;
  provideSemanticTokens(): Uint32Array;
  provideDocumentSymbols?(): Array<{
    name: string;
    detail?: string;
    kind: number;
    range: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
    children?: Array<{
      name: string;
      detail?: string;
      kind: number;
      range: {
        start: { line: number; character: number };
        end: { line: number; character: number };
      };
    }>;
  }>;
}

// Global state
let editor: monaco.editor.IStandaloneCodeEditor;
let wabt: WabtModule;
let watLSP: WatLSP | null = null;
let wasmModule: WebAssembly.Module | null = null;
let wasmInstance: WebAssembly.Instance | null = null;
let wasmBytes: Uint8Array | null = null;
let watGrammar: vsctm.IGrammar | null = null;
let currentExampleId: string = ''; // Track currently loaded example

// Track error counts per file for sidebar styling
const fileErrors: Map<string, number> = new Map();

// DOM element references (initialized in init())
let elements: {
  status: HTMLElement;
  lspIndicator: HTMLElement;
  lspStatusText: HTMLElement;
  consoleOutput: HTMLElement;
  fileTree: HTMLElement;
  compileBtn: HTMLButtonElement;
  runBtn: HTMLButtonElement;
  downloadBtn: HTMLButtonElement;
  importsList: HTMLElement;
  exportsList: HTMLElement;
  exportFnSelect: HTMLSelectElement;
  fnArgs: HTMLInputElement;
  callFnBtn: HTMLButtonElement;
  fnResult: HTMLElement;
  hexView: HTMLElement;
  diagnosticsView: HTMLElement;
  symbolTableView: HTMLElement;
  hoverDebug: HTMLElement;
  outlineView: HTMLElement;
  testLine: HTMLInputElement;
  testCol: HTMLInputElement;
  testHoverBtn: HTMLButtonElement;
  testResult: HTMLElement;
};

// Console output helper
const consoleOutput = {
  get element(): HTMLElement {
    return elements.consoleOutput;
  },

  clear(): void {
    this.element.innerHTML = '';
  },

  log(message: string, type: string = 'log'): void {
    const line = document.createElement('div');
    line.className = `console-line ${type}`;
    line.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
    this.element.appendChild(line);
    this.element.scrollTop = this.element.scrollHeight;
  },

  error(message: string): void {
    this.log(message, 'error');
  },

  warn(message: string): void {
    this.log(message, 'warn');
  },

  info(message: string): void {
    this.log(message, 'info');
  },
};

// Status helpers
function setStatus(message: string, type: string = ''): void {
  elements.status.textContent = message;
  elements.status.className = type;
}

function setLSPStatus(ready: boolean, message: string): void {
  elements.lspIndicator.className =
    'lsp-indicator' +
    (ready ? ' ready' : message.includes('Error') ? ' error' : '');
  elements.lspStatusText.textContent = message;
}

// Populate the file tree from the examples data
function populateFileTree(): void {
  const container = elements.fileTree;

  // Separate valid and invalid examples
  const validExamples = examples.filter(ex => !ex.id.startsWith('invalid_'));
  const invalidExamples = examples.filter(ex => ex.id.startsWith('invalid_'));

  // Helper to create a folder element
  function createFolder(name: string, expanded: boolean = false): HTMLElement {
    const folder = document.createElement('div');
    folder.className = 'tree-folder';

    const header = document.createElement('div');
    header.className = 'tree-folder-header';
    header.innerHTML = `<span class="tree-chevron">${expanded ? '▼' : '▶'}</span><span class="folder-icon">📁</span> ${name}`;

    const children = document.createElement('div');
    children.className = 'tree-folder-children';
    if (!expanded) children.style.display = 'none';

    header.addEventListener('click', () => {
      const isExpanded = children.style.display !== 'none';
      children.style.display = isExpanded ? 'none' : 'block';
      header.querySelector('.tree-chevron')!.textContent = isExpanded ? '▶' : '▼';
    });

    folder.appendChild(header);
    folder.appendChild(children);
    return folder;
  }

  // Helper to create a file item
  function createFileItem(id: string, label: string, icon: string = '📄'): HTMLElement {
    const item = document.createElement('div');
    item.className = 'tree-item';
    item.setAttribute('data-id', id);
    item.innerHTML = `<span class="file-icon">${icon}</span> ${escapeHtml(label)}`;

    item.addEventListener('click', () => {
      // Remove selection from all items
      container.querySelectorAll('.tree-item').forEach(el => el.classList.remove('selected'));
      // Select this item
      item.classList.add('selected');
      // Load the example
      loadExample(id);
    });

    return item;
  }

  // Build the tree
  const tree = document.createElement('div');
  tree.className = 'file-tree';

  // Add valid examples folder
  const validFolder = createFolder('examples', true);
  const validChildren = validFolder.querySelector('.tree-folder-children')!;
  for (const example of validExamples) {
    validChildren.appendChild(createFileItem(example.id, example.label));
  }
  tree.appendChild(validFolder);

  // Add invalid examples folder if there are any
  if (invalidExamples.length > 0) {
    const invalidFolder = createFolder('invalid', false);
    const invalidChildren = invalidFolder.querySelector('.tree-folder-children')!;
    for (const example of invalidExamples) {
      // Remove "Failure - " prefix for cleaner display in tree
      const displayLabel = example.label.replace(/^Failure - /, '');
      invalidChildren.appendChild(createFileItem(example.id, displayLabel, '⚠️'));
    }
    tree.appendChild(invalidFolder);
  }

  container.innerHTML = '';
  container.appendChild(tree);
}

// Map TextMate scopes to Monaco token types
function scopeToToken(scope: string): string {
  if (scope.startsWith('comment')) return 'comment';
  if (scope.startsWith('string')) return 'string';
  if (scope.startsWith('constant.character.escape')) return 'string.escape';
  if (scope.startsWith('constant.numeric')) return 'number';
  if (scope.startsWith('support.class.type')) return 'type';
  if (scope.startsWith('support.class')) return 'keyword';
  if (scope.startsWith('keyword.operator')) return 'delimiter';
  if (scope.startsWith('storage.type')) return 'keyword';
  if (scope.startsWith('keyword.control')) return 'keyword.control';
  if (scope.startsWith('storage.modifier')) return 'keyword';
  if (scope.startsWith('entity.name.type')) return 'type';
  if (scope.startsWith('entity.other.attribute-name')) return 'attribute';
  if (scope.startsWith('variable')) return 'variable';
  return '';
}

// TextMate tokenizer state class
class TMState implements monaco.languages.IState {
  constructor(public ruleStack: vsctm.StateStack | null) {}

  clone(): TMState {
    return new TMState(this.ruleStack);
  }

  equals(other: monaco.languages.IState): boolean {
    if (!other || !(other instanceof TMState)) return false;
    if (!this.ruleStack && !other.ruleStack) return true;
    if (!this.ruleStack || !other.ruleStack) return false;
    return this.ruleStack.equals(other.ruleStack);
  }
}

// Initialize Monaco Editor with LSP features
async function initMonaco(): Promise<void> {
  // Load oniguruma WASM for TextMate grammar support
  const onigResponse = await fetch('/onig.wasm');
  const onigBuffer = await onigResponse.arrayBuffer();
  await oniguruma.loadWASM(onigBuffer);

  // Create oniguruma library interface for vscode-textmate
  const onigLib = {
    createOnigScanner: (patterns: string[]) =>
      new oniguruma.OnigScanner(patterns),
    createOnigString: (s: string) => new oniguruma.OnigString(s),
  };

  // Create TextMate registry
  const registry = new vsctm.Registry({
    onigLib: Promise.resolve(onigLib),
    loadGrammar: async (scopeName: string) => {
      if (scopeName === 'source.wat') {
        const response = await fetch('/wat.tmLanguage.json');
        const grammar = await response.json();
        return vsctm.parseRawGrammar(
          JSON.stringify(grammar),
          'wat.tmLanguage.json'
        );
      }
      return null;
    },
  });

  // Load the WAT grammar
  watGrammar = await registry.loadGrammar('source.wat');
  console.log('TextMate grammar loaded:', watGrammar ? 'success' : 'failed');

  // Register WAT language
  monaco.languages.register({
    id: 'wat',
    extensions: ['.wat', '.wast'],
    aliases: ['WebAssembly Text', 'WAT'],
  });

  // Set language configuration
  monaco.languages.setLanguageConfiguration(
    'wat',
    watLanguage.languageConfiguration
  );

  // Register TextMate-based token provider
  monaco.languages.setTokensProvider('wat', {
    getInitialState: () => new TMState(vsctm.INITIAL),

    tokenize: (
      line: string,
      state: monaco.languages.IState
    ): monaco.languages.ILineTokens => {
      if (!watGrammar) {
        return { tokens: [], endState: state };
      }

      const tmState = state as TMState;
      const result = watGrammar.tokenizeLine(line, tmState.ruleStack);
      const tokens: monaco.languages.IToken[] = [];

      for (const token of result.tokens) {
        const scopes = token.scopes;
        let tokenType = '';
        for (let i = scopes.length - 1; i >= 0; i--) {
          const mapped = scopeToToken(scopes[i]);
          if (mapped) {
            tokenType = mapped;
            break;
          }
        }
        tokens.push({
          startIndex: token.startIndex,
          scopes: tokenType,
        });
      }

      return {
        tokens,
        endState: new TMState(result.ruleStack),
      };
    },
  });

  console.log('TextMate token provider registered');

  // Register completion provider - uses WASM LSP when available, falls back to static completions
  monaco.languages.registerCompletionItemProvider('wat', {
    triggerCharacters: ['.', '$', '@', '2', '4'], // Match LSP trigger characters
    provideCompletionItems: (
      model: monaco.editor.ITextModel,
      position: monaco.Position
    ): monaco.languages.ProviderResult<monaco.languages.CompletionList> => {
      // Use WASM-based completion if LSP is ready
      if (watLSP && watLSP.ready) {
        // Parse latest content
        watLSP.parse(model.getValue());

        const completions = watLSP.provideCompletion(
          position.lineNumber - 1, // Monaco is 1-indexed
          position.column - 1
        );

        const suggestions = completions.map((item) => ({
          label: item.label,
          kind: item.kind ?? monaco.languages.CompletionItemKind.Keyword,
          insertText: item.insertText || item.label,
          insertTextRules: item.insertTextRules || 0,
          documentation: item.documentation,
          detail: item.detail,
          range: {
            startLineNumber: position.lineNumber,
            startColumn: position.column,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        }));

        return { suggestions };
      }

      // Fall back to static completions if LSP not ready
      const suggestions = watLanguage.completionItems.map((item) => ({
        label: item.label,
        kind:
          monaco.languages.CompletionItemKind[
            item.kind as keyof typeof monaco.languages.CompletionItemKind
          ] || monaco.languages.CompletionItemKind.Keyword,
        insertText: item.insertText || item.label,
        insertTextRules: item.insertTextRules || 0,
        documentation: item.documentation,
        range: {
          startLineNumber: position.lineNumber,
          startColumn: position.column,
          endLineNumber: position.lineNumber,
          endColumn: position.column,
        },
      }));
      return { suggestions };
    },
  });

  // Register hover provider
  monaco.languages.registerHoverProvider('wat', {
    provideHover: (
      model: monaco.editor.ITextModel,
      position: monaco.Position
    ): monaco.languages.ProviderResult<monaco.languages.Hover> => {
      if (!watLSP || !watLSP.ready) return null;

      // Parse latest content
      watLSP.parse(model.getValue());

      const hover = watLSP.provideHover(
        position.lineNumber - 1, // Monaco is 1-indexed
        position.column - 1
      );

      // Update debug panel
      updateHoverDebug(position.lineNumber, position.column, hover);

      if (!hover) return null;

      return {
        contents: [{ value: hover.contents.value }],
        range: hover.range
          ? new monaco.Range(
              hover.range.start.line + 1,
              hover.range.start.character + 1,
              hover.range.end.line + 1,
              hover.range.end.character + 1
            )
          : undefined,
      };
    },
  });

  // Register definition provider
  monaco.languages.registerDefinitionProvider('wat', {
    provideDefinition: (
      model: monaco.editor.ITextModel,
      position: monaco.Position
    ): monaco.languages.ProviderResult<monaco.languages.Definition> => {
      if (!watLSP || !watLSP.ready) return null;

      // Parse latest content
      watLSP.parse(model.getValue());

      const definition = watLSP.provideDefinition(
        position.lineNumber - 1,
        position.column - 1
      );

      console.log(
        'Definition request at',
        position.lineNumber,
        position.column,
        '-> result:',
        definition
      );

      if (!definition || !definition.range) return null;

      return {
        uri: model.uri,
        range: new monaco.Range(
          definition.range.start.line + 1,
          definition.range.start.character + 1,
          definition.range.end.line + 1,
          definition.range.end.character + 1
        ),
      };
    },
  });

  // Register references provider
  monaco.languages.registerReferenceProvider('wat', {
    provideReferences: (
      model: monaco.editor.ITextModel,
      position: monaco.Position,
      context: monaco.languages.ReferenceContext
    ): monaco.languages.ProviderResult<monaco.languages.Location[]> => {
      if (!watLSP || !watLSP.ready) return [];

      // Parse latest content
      watLSP.parse(model.getValue());

      const references = watLSP.provideReferences(
        position.lineNumber - 1,
        position.column - 1,
        context.includeDeclaration
      );

      return references.map((ref) => ({
        uri: model.uri,
        range: new monaco.Range(
          ref.range.start.line + 1,
          ref.range.start.character + 1,
          ref.range.end.line + 1,
          ref.range.end.character + 1
        ),
      }));
    },
  });

  // Register document symbol provider for outline view
  monaco.languages.registerDocumentSymbolProvider('wat', {
    provideDocumentSymbols: (
      model: monaco.editor.ITextModel
    ): monaco.languages.ProviderResult<monaco.languages.DocumentSymbol[]> => {
      if (!watLSP || !watLSP.ready || !watLSP.provideDocumentSymbols) return [];

      // Parse latest content
      watLSP.parse(model.getValue());

      const symbols = watLSP.provideDocumentSymbols();
      if (!symbols || symbols.length === 0) return [];

      // Convert to Monaco DocumentSymbol format (recursively handle children)
      function convertSymbol(sym: {
        name: string;
        detail?: string;
        kind: number;
        range: { start: { line: number; character: number }; end: { line: number; character: number } };
        children?: Array<{ name: string; detail?: string; kind: number; range: { start: { line: number; character: number }; end: { line: number; character: number } } }>;
      }): monaco.languages.DocumentSymbol {
        const range = sym.range
          ? new monaco.Range(
              sym.range.start.line + 1,
              sym.range.start.character + 1,
              sym.range.end.line + 1,
              sym.range.end.character + 1
            )
          : new monaco.Range(1, 1, 1, 1);

        const result: monaco.languages.DocumentSymbol = {
          name: sym.name,
          detail: sym.detail || '',
          kind: sym.kind as monaco.languages.SymbolKind,
          range: range,
          selectionRange: range,
          children: [],
          tags: [],
        };

        if (sym.children && sym.children.length > 0) {
          result.children = sym.children.map(convertSymbol);
        }

        return result;
      }

      return symbols.map(convertSymbol);
    },
  });

  // Define custom theme matching VS Code Dark+ colors
  monaco.editor.defineTheme('wat-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '6A9955' },
      { token: 'string', foreground: 'CE9178' },
      { token: 'string.escape', foreground: 'D7BA7D' },
      { token: 'number', foreground: 'B5CEA8' },
      { token: 'type', foreground: '4EC9B0' },
      { token: 'keyword', foreground: '569CD6' },
      { token: 'keyword.control', foreground: 'C586C0' },
      { token: 'variable', foreground: '9CDCFE' },
      { token: 'attribute', foreground: '9CDCFE' },
      { token: 'delimiter', foreground: 'D4D4D4' },
    ],
    colors: {
      'editorBracketHighlight.foreground1': '#D4D4D4',
      'editorBracketHighlight.foreground2': '#D4D4D4',
      'editorBracketHighlight.foreground3': '#D4D4D4',
      'editorBracketHighlight.foreground4': '#D4D4D4',
      'editorBracketHighlight.foreground5': '#D4D4D4',
      'editorBracketHighlight.foreground6': '#D4D4D4',
    },
  });

  // Create editor with default example
  const defaultExample = getDefaultExample();
  currentExampleId = defaultExample.id;
  editor = monaco.editor.create(document.getElementById('editor')!, {
    value: defaultExample.code,
    language: 'wat',
    theme: 'wat-dark',
    fontSize: 14,
    fontFamily: "'Consolas', 'Monaco', 'Courier New', monospace",
    minimap: { enabled: false },
    automaticLayout: true,
    tabSize: 2,
    scrollBeyondLastLine: false,
    renderWhitespace: 'selection',
    lineNumbers: 'on',
    folding: true,
    bracketPairColorization: { enabled: false },
    'semanticHighlighting.enabled': true,
  });

  // Expose editor and monaco for testing
  (window as unknown as { monacoEditor: typeof editor; monaco: typeof monaco }).monacoEditor = editor;
  (window as unknown as { monaco: typeof monaco }).monaco = monaco;

  // Set initial editor title
  const editorTitle = document.getElementById('editor-title');
  if (editorTitle) {
    editorTitle.textContent = `${defaultExample.id}.wat`;
  }

  // Override theme's getTokenStyleMetadata for semantic token coloring
  try {
    const theme = (editor as unknown as { _themeService: { _theme: { getTokenStyleMetadata: (type: string, modifiers: string[], lang: string) => { foreground: number } | undefined } } })._themeService._theme;

    // Inject CSS for semantic token colors
    const styleEl = document.createElement('style');
    styleEl.textContent = `
      .monaco-editor .mtk100 { color: #DCDCAA !important; }
      .monaco-editor .mtk101 { color: #569CD6 !important; }
      .monaco-editor .mtk102 { color: #9CDCFE !important; }
    `;
    document.head.appendChild(styleEl);

    theme.getTokenStyleMetadata = (
      type: string,
      _modifiers: string[],
      _modelLanguage: string
    ) => {
      const colorMapping: Record<string, { foreground: number }> = {
        variable: { foreground: 102 },
      };
      return colorMapping[type];
    };
    console.log('Semantic token styling configured');
  } catch (e) {
    console.warn('Could not override theme token metadata:', e);
  }

  // Parse on content change (debounced)
  let parseTimeout: ReturnType<typeof setTimeout>;
  editor.onDidChangeModelContent(() => {
    clearTimeout(parseTimeout);
    parseTimeout = setTimeout(() => {
      if (watLSP && watLSP.ready) {
        watLSP.parse(editor.getValue());
        updateDiagnostics();
        updateLSPDebugPanel();
      }
    }, 300);
  });
}

// Update diagnostics (error markers) from LSP
function updateDiagnostics(): void {
  if (!watLSP || !editor) return;

  const diagnostics = watLSP.provideDiagnostics();
  const markers: monaco.editor.IMarkerData[] = diagnostics.map((diag) => ({
    startLineNumber: diag.range.start.line + 1,
    startColumn: diag.range.start.character + 1,
    endLineNumber: diag.range.end.line + 1,
    endColumn: diag.range.end.character + 1,
    message: diag.message,
    severity:
      diag.severity === 1
        ? monaco.MarkerSeverity.Error
        : diag.severity === 2
          ? monaco.MarkerSeverity.Warning
          : diag.severity === 3
            ? monaco.MarkerSeverity.Info
            : monaco.MarkerSeverity.Hint,
  }));

  monaco.editor.setModelMarkers(editor.getModel()!, 'wat-lsp', markers);

  // Update current file's error state in sidebar
  updateCurrentFileErrors();

  // Update diagnostics panel
  if (diagnostics.length === 0) {
    elements.diagnosticsView.innerHTML = '<p class="placeholder">No errors</p>';
  } else {
    elements.diagnosticsView.innerHTML = diagnostics
      .map((diag) => {
        const line = diag.range.start.line + 1;
        const col = diag.range.start.character + 1;
        const severityClass =
          diag.severity === 1
            ? 'error'
            : diag.severity === 2
              ? 'warning'
              : 'info';
        const severityIcon =
          diag.severity === 1 ? '⛔' : diag.severity === 2 ? '⚠️' : 'ℹ️';
        return `<div class="diagnostic-item ${severityClass}" onclick="goToLine(${line}, ${col})">
          <span class="diagnostic-icon">${severityIcon}</span>
          <span class="diagnostic-location">[Ln ${line}, Col ${col}]</span>
          <span class="diagnostic-message">${escapeHtml(diag.message)}</span>
        </div>`;
      })
      .join('');
  }
}

// Update LSP debug panel
function updateLSPDebugPanel(): void {
  if (!watLSP) return;
  elements.symbolTableView.innerHTML = watLSP.getSymbolTableHTML();
  updateOutlineView();
}

// Symbol kind to icon/class mapping
const kindInfo: Record<number, { icon: string; class: string; label: string }> = {
  11: { icon: 'I', class: 'interface', label: 'type' },     // Interface (type)
  12: { icon: 'F', class: 'function', label: 'function' },   // Function
  13: { icon: 'V', class: 'variable', label: 'variable' },   // Variable
  14: { icon: 'C', class: 'constant', label: 'global' },     // Constant (global)
  15: { icon: 'S', class: 'string', label: 'data' },         // String (data)
  18: { icon: 'A', class: 'array', label: 'table/elem' },    // Array (table/elem)
  19: { icon: 'M', class: 'object', label: 'memory' },       // Object (memory)
  20: { icon: 'L', class: 'key', label: 'label' },           // Key (block label)
  24: { icon: 'T', class: 'event', label: 'tag' },           // Event (tag)
};

// Update outline view with document symbols
function updateOutlineView(): void {
  if (!elements.outlineView || !watLSP || !watLSP.ready || !watLSP.provideDocumentSymbols) {
    if (elements.outlineView) {
      elements.outlineView.innerHTML = '<p class="placeholder">No symbols found</p>';
    }
    return;
  }

  const symbols = watLSP.provideDocumentSymbols();
  if (!symbols || symbols.length === 0) {
    elements.outlineView.innerHTML = '<p class="placeholder">No symbols found</p>';
    return;
  }

  interface SymbolInfo {
    name: string;
    detail?: string;
    kind: number;
    range?: { start: { line: number; character: number }; end: { line: number; character: number } };
    children?: SymbolInfo[];
  }

  function renderSymbol(sym: SymbolInfo, depth: number = 0): string {
    const info = kindInfo[sym.kind] || { icon: '?', class: 'unknown', label: 'unknown' };
    const line = sym.range?.start?.line ?? 0;
    const hasChildren = sym.children && sym.children.length > 0;

    let html = `
      <div class="outline-item depth-${depth}" onclick="goToLine(${line + 1}, 1)" title="${info.label}: ${sym.name}${sym.detail ? ' - ' + sym.detail : ''}">
        <span class="outline-icon ${info.class}">${info.icon}</span>
        <span class="outline-name">${escapeHtml(sym.name)}</span>
        ${sym.detail ? `<span class="outline-detail">${escapeHtml(sym.detail)}</span>` : ''}
      </div>
    `;

    if (hasChildren) {
      html += `<div class="outline-children">`;
      for (const child of sym.children!) {
        html += renderSymbol(child, depth + 1);
      }
      html += `</div>`;
    }

    return html;
  }

  elements.outlineView.innerHTML = symbols.map((sym) => renderSymbol(sym)).join('');
}

// Update hover debug info
function updateHoverDebug(
  line: number,
  col: number,
  hover: ReturnType<WatLSP['provideHover']>
): void {
  if (!hover) {
    elements.hoverDebug.innerHTML = `<p class="placeholder">No hover at Ln ${line}, Col ${col}</p>`;
    return;
  }

  const content = hover.contents?.value || '(no content)';
  const rangeInfo = hover.range
    ? `Ln ${hover.range.start.line + 1}:${hover.range.start.character + 1} - Ln ${hover.range.end.line + 1}:${hover.range.end.character + 1}`
    : 'No range';

  elements.hoverDebug.innerHTML = `
    <div class="hover-debug-info">
      <div class="hover-position"><strong>Position:</strong> Ln ${line}, Col ${col}</div>
      <div class="hover-range"><strong>Range:</strong> ${rangeInfo}</div>
      <div class="hover-content"><strong>Content:</strong><pre>${escapeHtml(content)}</pre></div>
    </div>
  `;
}

// Helper to escape HTML
function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// Navigate to line/column in editor
function goToLine(line: number, col: number): void {
  if (editor) {
    editor.setPosition({ lineNumber: line, column: col });
    editor.revealLineInCenter(line);
    editor.focus();
  }
}
(window as unknown as { goToLine: typeof goToLine }).goToLine = goToLine;

// Run diagnostics on all example files and update the fileErrors map
function updateAllFileErrors(): void {
  if (!watLSP || !watLSP.ready) return;

  for (const example of examples) {
    watLSP.parse(example.code);
    const diagnostics = watLSP.provideDiagnostics();
    // Count only errors (severity 1)
    const errorCount = diagnostics.filter(d => d.severity === 1).length;
    fileErrors.set(example.id, errorCount);
  }

  // Re-parse current editor content to restore LSP state
  if (editor) {
    watLSP.parse(editor.getValue());
  }

  updateFileTreeErrors();
}

// Update file tree styling based on fileErrors
function updateFileTreeErrors(): void {
  const fileTree = elements.fileTree;
  if (!fileTree) return;

  const treeItems = fileTree.querySelectorAll<HTMLElement>('.tree-item');
  treeItems.forEach(item => {
    const id = item.getAttribute('data-id');
    if (id) {
      const errorCount = fileErrors.get(id) || 0;
      if (errorCount > 0) {
        item.classList.add('has-error');
      } else {
        item.classList.remove('has-error');
      }
    }
  });
}

// Update error state for the current file
function updateCurrentFileErrors(): void {
  if (!watLSP || !watLSP.ready || !currentExampleId) return;

  const diagnostics = watLSP.provideDiagnostics();
  const errorCount = diagnostics.filter(d => d.severity === 1).length;
  fileErrors.set(currentExampleId, errorCount);
  updateFileTreeErrors();
}

// Select a file in the file tree by ID
function selectFileInTree(id: string): void {
  const fileTree = elements.fileTree;
  if (!fileTree) return;

  // Remove selection from all items
  fileTree.querySelectorAll('.tree-item').forEach(el => el.classList.remove('selected'));

  // Select the item with matching data-id
  const item = fileTree.querySelector<HTMLElement>(`.tree-item[data-id="${id}"]`);
  if (item) {
    item.classList.add('selected');
  }
}

// Register semantic tokens provider for tree-sitter based syntax highlighting
function registerSemanticTokensProvider(): void {
  if (!watLSP) {
    console.warn('registerSemanticTokensProvider: watLSP not available');
    return;
  }

  try {
    const legend = watLSP.getSemanticTokensLegend();

    monaco.languages.registerDocumentSemanticTokensProvider('wat', {
      getLegend: () => legend,

      provideDocumentSemanticTokens: (
        model: monaco.editor.ITextModel
      ): monaco.languages.ProviderResult<monaco.languages.SemanticTokens> => {
        if (!watLSP || !watLSP.ready) {
          console.log('provideDocumentSemanticTokens: LSP not ready');
          return { data: new Uint32Array(0) };
        }

        watLSP.parse(model.getValue());
        const tokens = watLSP.provideSemanticTokens();

        return {
          data: tokens,
          resultId: String(Date.now()),
        };
      },

      releaseDocumentSemanticTokens: () => {
        // Nothing to clean up
      },
    });

    consoleOutput.info('Semantic tokens provider registered');
  } catch (e) {
    console.error('Failed to register semantic tokens provider:', e);
    consoleOutput.error(
      'Failed to register semantic tokens: ' + (e as Error).message
    );
  }
}

// Initialize LSP
async function initLSP(): Promise<boolean> {
  setLSPStatus(false, 'Loading WASM LSP...');

  try {
    const lsp = await createWatLSP({
      treeSitterWasmPath: '/tree-sitter.wasm',
      watLspWasmPath: '/wat_lsp_rust_bg.wasm',
    });
    watLSP = lsp as WatLSP;

    setLSPStatus(true, 'LSP Ready (Rust WASM)');
    consoleOutput.info('LSP initialized with @emnudge/wat-lsp');
    consoleOutput.info(
      'Features: Hover, Go to Definition (F12), Find References (Shift+F12), Semantic Highlighting'
    );

    registerSemanticTokensProvider();

    if (editor && watLSP) {
      watLSP.parse(editor.getValue());
      updateDiagnostics();
      updateLSPDebugPanel();
    }

    // Run diagnostics on all files to populate sidebar error indicators
    updateAllFileErrors();

    (window as unknown as { watLSP: WatLSP | null }).watLSP = watLSP;

    return true;
  } catch (error) {
    console.error('Failed to initialize WASM LSP:', error);
    setLSPStatus(false, 'LSP Error');
    consoleOutput.error(
      `LSP initialization failed: ${(error as Error).message}`
    );
    return false;
  }
}

// Initialize wabt.js
async function initWabt(): Promise<void> {
  wabt = await wabtInit();
  consoleOutput.info('wabt.js initialized');
}

// Compile WAT to WASM
async function compile(): Promise<boolean> {
  const source = editor.getValue();

  setStatus('Compiling...', 'compiling');
  consoleOutput.clear();
  consoleOutput.info('Starting compilation...');

  monaco.editor.setModelMarkers(editor.getModel()!, 'wat', []);

  try {
    const module = wabt.parseWat('input.wat', source, {
      bulk_memory: true,
      exceptions: true,
      gc: true,
      memory64: false,
      multi_value: true,
      mutable_globals: true,
      reference_types: true,
      relaxed_simd: false,
      saturating_float_to_int: true,
      sign_extension: true,
      simd: true,
      tail_call: true,
      threads: false,
    });

    module.validate();

    const result = module.toBinary({ log: false, write_debug_names: true });
    wasmBytes = result.buffer;

    consoleOutput.log(`Compiled successfully (${wasmBytes.byteLength} bytes)`);

    wasmModule = await WebAssembly.compile(wasmBytes.buffer as ArrayBuffer);

    displayModuleInfo(wasmModule);
    displayHexView(wasmBytes);

    elements.runBtn.disabled = false;
    elements.downloadBtn.disabled = false;

    setStatus('Compiled successfully', 'success');

    module.destroy();

    return true;
  } catch (error) {
    consoleOutput.error(`Compilation failed: ${(error as Error).message}`);
    setStatus('Compilation failed', 'error');

    const match = (error as Error).message.match(/input\.wat:(\d+):(\d+)/);
    if (match) {
      const lineNumber = parseInt(match[1], 10);
      const column = parseInt(match[2], 10);
      monaco.editor.setModelMarkers(editor.getModel()!, 'wat', [
        {
          startLineNumber: lineNumber,
          startColumn: column,
          endLineNumber: lineNumber,
          endColumn: column + 1,
          message: (error as Error).message,
          severity: monaco.MarkerSeverity.Error,
        },
      ]);
    }

    return false;
  }
}

// Display module imports and exports
function displayModuleInfo(module: WebAssembly.Module): void {
  const imports = WebAssembly.Module.imports(module);
  const exports = WebAssembly.Module.exports(module);

  if (imports.length === 0) {
    elements.importsList.innerHTML = '<p class="placeholder">No imports</p>';
  } else {
    elements.importsList.innerHTML = imports
      .map(
        (imp) => `
      <div class="import-item">
        <span class="item-name">${imp.module}.${imp.name}</span>
        <span class="item-type">${imp.kind}</span>
      </div>
    `
      )
      .join('');
  }

  if (exports.length === 0) {
    elements.exportsList.innerHTML = '<p class="placeholder">No exports</p>';
  } else {
    elements.exportsList.innerHTML = exports
      .map(
        (exp) => `
      <div class="export-item">
        <span class="item-name">${exp.name}</span>
        <span class="item-type">${exp.kind}</span>
      </div>
    `
      )
      .join('');
  }

  const fnExports = exports.filter((e) => e.kind === 'function');
  elements.exportFnSelect.innerHTML =
    '<option value="">Select function...</option>' +
    fnExports.map((e) => `<option value="${e.name}">${e.name}</option>`).join('');
  elements.exportFnSelect.disabled = fnExports.length === 0;
  elements.fnArgs.disabled = fnExports.length === 0;
  elements.callFnBtn.disabled = fnExports.length === 0;
}

// Display hex view of WASM binary
function displayHexView(bytes: Uint8Array): void {
  const arr = new Uint8Array(bytes);
  const lines: string[] = [];

  for (let i = 0; i < arr.length; i += 16) {
    const offset = i.toString(16).padStart(8, '0');
    const bytesHex: string[] = [];
    const ascii: string[] = [];

    for (let j = 0; j < 16 && i + j < arr.length; j++) {
      const byte = arr[i + j];
      bytesHex.push(byte.toString(16).padStart(2, '0'));
      ascii.push(byte >= 32 && byte < 127 ? String.fromCharCode(byte) : '.');
    }

    lines.push(
      `<span class="hex-offset">${offset}</span>` +
        `<span class="hex-bytes">${bytesHex.join(' ').padEnd(48, ' ')}</span>` +
        `<span class="hex-ascii">${ascii.join('')}</span>`
    );
  }

  elements.hexView.innerHTML = lines
    .map((l) => `<div class="hex-line">${l}</div>`)
    .join('');
}

// Create import object with stubs
function createImportObject(
  module: WebAssembly.Module
): WebAssembly.Imports {
  const imports = WebAssembly.Module.imports(module);
  const importObject: WebAssembly.Imports = {};

  for (const imp of imports) {
    if (!importObject[imp.module]) {
      importObject[imp.module] = {};
    }

    const moduleImports = importObject[imp.module] as Record<string, unknown>;

    switch (imp.kind) {
      case 'function':
        moduleImports[imp.name] = (...args: unknown[]) => {
          consoleOutput.log(`[import] ${imp.module}.${imp.name}(${args.join(', ')})`);
          return 0;
        };
        break;

      case 'memory':
        moduleImports[imp.name] = new WebAssembly.Memory({
          initial: 1,
          maximum: 10,
        });
        break;

      case 'table':
        moduleImports[imp.name] = new WebAssembly.Table({
          initial: 10,
          element: 'anyfunc',
        });
        break;

      case 'global':
        moduleImports[imp.name] = new WebAssembly.Global(
          { value: 'i32', mutable: true },
          0
        );
        break;
    }
  }

  // Override specific imports with useful implementations
  const envImports = importObject.env as Record<string, unknown> | undefined;
  if (envImports) {
    if (envImports.log) {
      envImports.log = (value: number) => {
        consoleOutput.log(`[log] ${value}`);
      };
    }
    if (envImports.logFloat) {
      envImports.logFloat = (value: number) => {
        consoleOutput.log(`[log] ${value}`);
      };
    }
  }

  return importObject;
}

// Instantiate and run the module
async function run(): Promise<void> {
  if (!wasmModule) {
    consoleOutput.error('No module compiled');
    return;
  }

  try {
    consoleOutput.info('Instantiating module...');

    const importObject = createImportObject(wasmModule);
    wasmInstance = await WebAssembly.instantiate(wasmModule, importObject);

    consoleOutput.log('Module instantiated successfully');
    consoleOutput.info('Available exports:');

    for (const [name, value] of Object.entries(wasmInstance.exports)) {
      const type =
        typeof value === 'function' ? 'function' : (value as object).constructor.name;
      consoleOutput.log(`  ${name}: ${type}`);
    }

    setStatus('Module ready', 'success');
  } catch (error) {
    consoleOutput.error(`Instantiation failed: ${(error as Error).message}`);
    setStatus('Instantiation failed', 'error');
  }
}

// Call an exported function
function callExportedFunction(): void {
  const fnName = elements.exportFnSelect.value;
  const argsStr = elements.fnArgs.value;

  if (!fnName) {
    elements.fnResult.textContent = 'Select a function first';
    elements.fnResult.className = 'fn-result error';
    return;
  }

  if (!wasmInstance) {
    elements.fnResult.textContent = 'Module not instantiated. Click "Run" first.';
    elements.fnResult.className = 'fn-result error';
    return;
  }

  const fn = wasmInstance.exports[fnName];
  if (typeof fn !== 'function') {
    elements.fnResult.textContent = `"${fnName}" is not a function`;
    elements.fnResult.className = 'fn-result error';
    return;
  }

  let args: number[] = [];
  if (argsStr.trim()) {
    args = argsStr.split(',').map((arg) => {
      const trimmed = arg.trim();
      if (trimmed.includes('.')) {
        return parseFloat(trimmed);
      }
      return parseInt(trimmed, 10);
    });
  }

  try {
    consoleOutput.info(`Calling ${fnName}(${args.join(', ')})`);
    const result = fn(...args);
    consoleOutput.log(`Result: ${result}`);
    elements.fnResult.textContent = `Result: ${result}`;
    elements.fnResult.className = 'fn-result success';
  } catch (error) {
    consoleOutput.error(`Error: ${(error as Error).message}`);
    elements.fnResult.textContent = `Error: ${(error as Error).message}`;
    elements.fnResult.className = 'fn-result error';
  }
}

// Download WASM binary
function downloadWasm(): void {
  if (!wasmBytes) return;

  const blob = new Blob([wasmBytes.buffer as ArrayBuffer], { type: 'application/wasm' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'module.wasm';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);

  consoleOutput.info('Downloaded module.wasm');
}

// Load example
function loadExample(id: string): void {
  const example = getExampleById(id);
  if (!example) return;

  currentExampleId = id;
  editor.setValue(example.code);
  monaco.editor.setModelMarkers(editor.getModel()!, 'wat', []);

  // Update editor title with file name
  const editorTitle = document.getElementById('editor-title');
  if (editorTitle) {
    editorTitle.textContent = `${id}.wat`;
  }

  // Reset state
  wasmModule = null;
  wasmInstance = null;
  wasmBytes = null;

  elements.runBtn.disabled = true;
  elements.downloadBtn.disabled = true;
  elements.importsList.innerHTML =
    '<p class="placeholder">Compile to see imports</p>';
  elements.exportsList.innerHTML =
    '<p class="placeholder">Compile to see exports</p>';
  elements.exportFnSelect.innerHTML =
    '<option value="">Select function...</option>';
  elements.exportFnSelect.disabled = true;
  elements.fnArgs.disabled = true;
  elements.callFnBtn.disabled = true;
  elements.fnResult.textContent = '';
  elements.fnResult.className = 'fn-result';
  elements.hexView.innerHTML = '';

  consoleOutput.clear();
  consoleOutput.info(`Loaded example: ${example.label}`);

  if (watLSP && watLSP.ready) {
    watLSP.parse(editor.getValue());
    updateLSPDebugPanel();
  }

  setStatus('Ready');
}

// Tab switching
function initTabs(): void {
  const tabs = document.querySelectorAll<HTMLButtonElement>('.tab');
  tabs.forEach((tab) => {
    tab.addEventListener('click', () => {
      tabs.forEach((t) => t.classList.remove('active'));
      document
        .querySelectorAll('.tab-pane')
        .forEach((p) => p.classList.remove('active'));

      tab.classList.add('active');
      const paneId = tab.dataset.tab!;
      document.getElementById(paneId)!.classList.add('active');
    });
  });
}

// Initialize resizable panel
function initResizablePanel(): void {
  const resizeHandle = document.querySelector<HTMLElement>('.resize-handle');
  const outputPanel = document.querySelector<HTMLElement>('.output-panel');

  if (!resizeHandle || !outputPanel) return;

  const overlay = document.createElement('div');
  overlay.style.cssText =
    'position: fixed; inset: 0; z-index: 9999; cursor: col-resize; display: none;';
  document.body.appendChild(overlay);

  let isResizing = false;
  let startX = 0;
  let startWidth = 0;

  resizeHandle.addEventListener('mousedown', (e) => {
    isResizing = true;
    startX = e.clientX;
    startWidth = outputPanel.offsetWidth;
    resizeHandle.classList.add('active');
    overlay.style.display = 'block';
    e.preventDefault();
  });

  document.addEventListener('mousemove', (e) => {
    if (!isResizing) return;

    const delta = startX - e.clientX;
    const newWidth = Math.min(
      Math.max(startWidth + delta, 200),
      window.innerWidth * 0.8
    );
    outputPanel.style.width = `${newWidth}px`;
  });

  document.addEventListener('mouseup', () => {
    if (isResizing) {
      isResizing = false;
      resizeHandle.classList.remove('active');
      overlay.style.display = 'none';
    }
  });
}

// Initialize resizable file tree panel
function initFileTreeResize(): void {
  const resizeHandle = document.getElementById('file-tree-resize');
  const fileTreePanel = document.getElementById('file-tree-panel');

  if (!resizeHandle || !fileTreePanel) return;

  const overlay = document.createElement('div');
  overlay.style.cssText =
    'position: fixed; inset: 0; z-index: 9999; cursor: col-resize; display: none;';
  document.body.appendChild(overlay);

  let isResizing = false;
  let startX = 0;
  let startWidth = 0;

  resizeHandle.addEventListener('mousedown', (e) => {
    isResizing = true;
    startX = e.clientX;
    startWidth = fileTreePanel.offsetWidth;
    resizeHandle.classList.add('active');
    overlay.style.display = 'block';
    e.preventDefault();
  });

  document.addEventListener('mousemove', (e) => {
    if (!isResizing) return;

    const delta = e.clientX - startX;
    const newWidth = Math.min(
      Math.max(startWidth + delta, 150),
      400
    );
    fileTreePanel.style.width = `${newWidth}px`;
  });

  document.addEventListener('mouseup', () => {
    if (isResizing) {
      isResizing = false;
      resizeHandle.classList.remove('active');
      overlay.style.display = 'none';
    }
  });
}

// Initialize DOM element references
function initElements(): void {
  elements = {
    status: document.getElementById('status')!,
    lspIndicator: document.getElementById('lsp-indicator')!,
    lspStatusText: document.getElementById('lsp-status-text')!,
    consoleOutput: document.getElementById('console-output')!,
    fileTree: document.getElementById('file-tree')!,
    compileBtn: document.getElementById('compile-btn') as HTMLButtonElement,
    runBtn: document.getElementById('run-btn') as HTMLButtonElement,
    downloadBtn: document.getElementById('download-btn') as HTMLButtonElement,
    importsList: document.getElementById('imports-list')!,
    exportsList: document.getElementById('exports-list')!,
    exportFnSelect: document.getElementById(
      'export-fn-select'
    ) as HTMLSelectElement,
    fnArgs: document.getElementById('fn-args') as HTMLInputElement,
    callFnBtn: document.getElementById('call-fn-btn') as HTMLButtonElement,
    fnResult: document.getElementById('fn-result')!,
    hexView: document.getElementById('hex-view')!,
    diagnosticsView: document.getElementById('diagnostics-view')!,
    symbolTableView: document.getElementById('symbol-table-view')!,
    hoverDebug: document.getElementById('hover-debug')!,
    outlineView: document.getElementById('outline-view')!,
    testLine: document.getElementById('test-line') as HTMLInputElement,
    testCol: document.getElementById('test-col') as HTMLInputElement,
    testHoverBtn: document.getElementById(
      'test-hover-btn'
    ) as HTMLButtonElement,
    testResult: document.getElementById('test-result')!,
  };
}

// Initialize everything
async function init(): Promise<void> {
  initElements();
  consoleOutput.info('Initializing WAT LSP Playground...');

  // Populate file tree from data
  populateFileTree();

  initTabs();
  initResizablePanel();
  initFileTreeResize();

  // Initialize Monaco, wabt, and LSP in parallel
  await Promise.all([initMonaco(), initWabt(), initLSP()]);

  // Select the default example in the file tree
  selectFileInTree(currentExampleId);

  // Bind event handlers
  elements.compileBtn.addEventListener('click', compile);
  elements.runBtn.addEventListener('click', run);
  elements.downloadBtn.addEventListener('click', downloadWasm);
  elements.callFnBtn.addEventListener('click', callExportedFunction);

  // LSP Debug test hover button
  elements.testHoverBtn.addEventListener('click', () => {
    const line = parseInt(elements.testLine.value, 10) || 0;
    const col = parseInt(elements.testCol.value, 10) || 0;

    if (!watLSP || !watLSP.ready) {
      elements.testResult.textContent = 'LSP not ready';
      return;
    }

    watLSP.parse(editor.getValue());

    const hover = watLSP.provideHover(line, col);
    updateHoverDebug(line + 1, col + 1, hover);

    if (hover) {
      elements.testResult.textContent = `Found: ${hover.contents.value.substring(0, 200)}...`;
    } else {
      elements.testResult.textContent = `No hover at line ${line}, col ${col}`;
    }
  });

  // Keyboard shortcuts
  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      compile();
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'Enter') {
      e.preventDefault();
      compile().then((success) => {
        if (success) run();
      });
    }
  });

  consoleOutput.log('Playground ready!');
  consoleOutput.info('Keyboard shortcuts:');
  consoleOutput.info('  Ctrl+Enter: Compile');
  consoleOutput.info('  Ctrl+Shift+Enter: Compile and Run');
  consoleOutput.info('  F12: Go to Definition');
  consoleOutput.info('  Shift+F12: Find References');
  setStatus('Ready');
}

// Start the app
init().catch((error) => {
  console.error('Failed to initialize:', error);
  setStatus('Initialization failed', 'error');
});
