// WAT Playground - Main Application with LSP Integration
import * as monaco from 'monaco-editor';
import wabtInit from 'wabt';
import { Parser } from 'web-tree-sitter';
import { watLanguage } from './wat-language.js';
import { watExamples } from './examples.js';
import initWasm, { WatLSP } from './wat_lsp_rust.js';
import * as vsctm from 'vscode-textmate';
import * as oniguruma from 'vscode-oniguruma';

// Pre-initialize web-tree-sitter with correct WASM path before our module loads
await Parser.init({
  locateFile: (file) => `/tree-sitter.wasm`
});

let editor;
let wabt;
let watLSP;
let wasmModule = null;
let wasmInstance = null;
let wasmBytes = null;

// Console output helper
const consoleOutput = {
  element: null,

  init() {
    this.element = document.getElementById('console-output');
  },

  clear() {
    this.element.innerHTML = '';
  },

  log(message, type = 'log') {
    const line = document.createElement('div');
    line.className = `console-line ${type}`;
    line.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
    this.element.appendChild(line);
    this.element.scrollTop = this.element.scrollHeight;
  },

  error(message) {
    this.log(message, 'error');
  },

  warn(message) {
    this.log(message, 'warn');
  },

  info(message) {
    this.log(message, 'info');
  }
};

// Status helpers
function setStatus(message, type = '') {
  const status = document.getElementById('status');
  status.textContent = message;
  status.className = type;
}

function setLSPStatus(ready, message) {
  const indicator = document.getElementById('lsp-indicator');
  const text = document.getElementById('lsp-status-text');
  indicator.className = 'lsp-indicator' + (ready ? ' ready' : (message.includes('Error') ? ' error' : ''));
  text.textContent = message;
}

// Global TextMate grammar instance
let watGrammar = null;

// Initialize Monaco Editor with LSP features
async function initMonaco() {
  // Load oniguruma WASM for TextMate grammar support
  const onigResponse = await fetch('/onig.wasm');
  const onigBuffer = await onigResponse.arrayBuffer();
  await oniguruma.loadWASM(onigBuffer);

  // Create oniguruma library interface for vscode-textmate
  const onigLib = {
    createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
    createOnigString: (s) => new oniguruma.OnigString(s)
  };

  // Create TextMate registry
  const registry = new vsctm.Registry({
    onigLib: Promise.resolve(onigLib),
    loadGrammar: async (scopeName) => {
      if (scopeName === 'source.wat') {
        const response = await fetch('/wat.tmLanguage.json');
        const grammar = await response.json();
        return vsctm.parseRawGrammar(JSON.stringify(grammar), 'wat.tmLanguage.json');
      }
      return null;
    }
  });

  // Load the WAT grammar
  watGrammar = await registry.loadGrammar('source.wat');
  console.log('TextMate grammar loaded:', watGrammar ? 'success' : 'failed');

  // Register WAT language
  monaco.languages.register({
    id: 'wat',
    extensions: ['.wat', '.wast'],
    aliases: ['WebAssembly Text', 'WAT']
  });

  // Set language configuration
  monaco.languages.setLanguageConfiguration('wat', watLanguage.languageConfiguration);

  // Map TextMate scopes to Monaco token types
  const scopeToToken = (scope) => {
    if (scope.startsWith('comment')) return 'comment';
    if (scope.startsWith('string')) return 'string';
    if (scope.startsWith('constant.character.escape')) return 'string.escape';
    if (scope.startsWith('constant.numeric')) return 'number';
    if (scope.startsWith('support.class.type')) return 'type';  // instruction prefix type
    if (scope.startsWith('support.class')) return 'keyword';     // other instruction prefix
    if (scope.startsWith('keyword.operator')) return 'delimiter'; // instruction suffix - white
    if (scope.startsWith('storage.type')) return 'keyword';
    if (scope.startsWith('keyword.control')) return 'keyword.control';
    if (scope.startsWith('storage.modifier')) return 'keyword';
    if (scope.startsWith('entity.name.type')) return 'type';
    if (scope.startsWith('entity.other.attribute-name')) return 'attribute';
    if (scope.startsWith('variable')) return 'variable';
    return '';
  };

  // TextMate tokenizer state class
  class TMState {
    constructor(ruleStack) {
      this.ruleStack = ruleStack;
    }
    clone() {
      return new TMState(this.ruleStack);
    }
    equals(other) {
      if (!other || !(other instanceof TMState)) return false;
      if (!this.ruleStack && !other.ruleStack) return true;
      if (!this.ruleStack || !other.ruleStack) return false;
      return this.ruleStack.equals(other.ruleStack);
    }
  }

  // Register TextMate-based token provider
  monaco.languages.setTokensProvider('wat', {
    getInitialState: () => new TMState(vsctm.INITIAL),

    tokenize: (line, state) => {
      if (!watGrammar) {
        return { tokens: [], endState: state };
      }

      const result = watGrammar.tokenizeLine(line, state.ruleStack);
      const tokens = [];

      for (const token of result.tokens) {
        // Get the most specific scope (last one)
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
          scopes: tokenType
        });
      }

      return {
        tokens,
        endState: new TMState(result.ruleStack)
      };
    }
  });

  console.log('TextMate token provider registered');

  // Register completion provider
  monaco.languages.registerCompletionItemProvider('wat', {
    provideCompletionItems: (model, position) => {
      const suggestions = watLanguage.completionItems.map(item => ({
        label: item.label,
        kind: monaco.languages.CompletionItemKind[item.kind] || monaco.languages.CompletionItemKind.Keyword,
        insertText: item.insertText || item.label,
        insertTextRules: item.insertTextRules || 0,
        documentation: item.documentation,
        range: {
          startLineNumber: position.lineNumber,
          startColumn: position.column,
          endLineNumber: position.lineNumber,
          endColumn: position.column
        }
      }));
      return { suggestions };
    }
  });

  // Register hover provider
  monaco.languages.registerHoverProvider('wat', {
    provideHover: (model, position) => {
      if (!watLSP || !watLSP.ready) return null;

      // Parse latest content
      watLSP.parse(model.getValue());

      const hover = watLSP.provideHover(
        position.lineNumber - 1,  // Monaco is 1-indexed
        position.column - 1
      );

      // Update debug panel
      updateHoverDebug(position.lineNumber, position.column, hover);

      if (!hover) return null;

      return {
        contents: [{ value: hover.contents.value }],
        range: hover.range ? new monaco.Range(
          hover.range.start.line + 1,
          hover.range.start.character + 1,
          hover.range.end.line + 1,
          hover.range.end.character + 1
        ) : undefined
      };
    }
  });

  // Register definition provider
  monaco.languages.registerDefinitionProvider('wat', {
    provideDefinition: (model, position) => {
      if (!watLSP || !watLSP.ready) return null;

      // Parse latest content
      watLSP.parse(model.getValue());

      const definition = watLSP.provideDefinition(
        position.lineNumber - 1,
        position.column - 1
      );

      console.log('Definition request at', position.lineNumber, position.column, '-> result:', definition);

      if (!definition || !definition.range) return null;

      return {
        uri: model.uri,
        range: new monaco.Range(
          definition.range.start.line + 1,
          definition.range.start.character + 1,
          definition.range.end.line + 1,
          definition.range.end.character + 1
        )
      };
    }
  });

  // Register references provider
  monaco.languages.registerReferenceProvider('wat', {
    provideReferences: (model, position, context) => {
      if (!watLSP || !watLSP.ready) return [];

      // Parse latest content
      watLSP.parse(model.getValue());

      const references = watLSP.provideReferences(
        position.lineNumber - 1,
        position.column - 1,
        context.includeDeclaration
      );

      return references.map(ref => ({
        uri: model.uri,
        range: new monaco.Range(
          ref.range.start.line + 1,
          ref.range.start.character + 1,
          ref.range.end.line + 1,
          ref.range.end.character + 1
        )
      }));
    }
  });

  // Define custom theme matching VS Code Dark+ colors
  monaco.editor.defineTheme('wat-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      // Comments - green
      { token: 'comment', foreground: '6A9955' },
      // Strings - orange
      { token: 'string', foreground: 'CE9178' },
      { token: 'string.escape', foreground: 'D7BA7D' },
      // Numbers - light green
      { token: 'number', foreground: 'B5CEA8' },
      // Types (i32, f64, etc.) - teal
      { token: 'type', foreground: '4EC9B0' },
      // Keywords - blue
      { token: 'keyword', foreground: '569CD6' },
      // Control flow keywords - purple
      { token: 'keyword.control', foreground: 'C586C0' },
      // Variables ($name) - light blue
      { token: 'variable', foreground: '9CDCFE' },
      // Attributes - light blue
      { token: 'attribute', foreground: '9CDCFE' },
      // Instruction suffix (.add, .get) - light gray (white-ish)
      { token: 'delimiter', foreground: 'D4D4D4' },
    ],
    colors: {
      // Use standard punctuation color for brackets
      'editorBracketHighlight.foreground1': '#D4D4D4',
      'editorBracketHighlight.foreground2': '#D4D4D4',
      'editorBracketHighlight.foreground3': '#D4D4D4',
      'editorBracketHighlight.foreground4': '#D4D4D4',
      'editorBracketHighlight.foreground5': '#D4D4D4',
      'editorBracketHighlight.foreground6': '#D4D4D4',
    }
  });

  // Create editor
  editor = monaco.editor.create(document.getElementById('editor'), {
    value: watExamples.hello,
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
    'semanticHighlighting.enabled': true,  // Tree-sitter semantic tokens enhance TextMate
    'editor.gotoLocation.multipleDefinitions': 'goto',
    'editor.gotoLocation.multipleReferences': 'goto'
  });

  // Override theme's getTokenStyleMetadata for semantic token coloring
  // This is required for Monaco standalone to properly style semantic tokens
  // See: https://github.com/microsoft/monaco-editor/issues/1833
  try {
    const theme = editor._themeService._theme;

    // Inject CSS for semantic token colors that enhance TextMate
    const styleEl = document.createElement('style');
    styleEl.textContent = `
      .monaco-editor .mtk100 { color: #DCDCAA !important; }  /* yellow - function calls */
      .monaco-editor .mtk101 { color: #569CD6 !important; }  /* blue - keywords */
      .monaco-editor .mtk102 { color: #9CDCFE !important; }  /* light blue - variables */
    `;
    document.head.appendChild(styleEl);

    theme.getTokenStyleMetadata = (type, modifiers, modelLanguage) => {
      // Semantic tokens enhance TextMate highlighting
      // Only override specific types where tree-sitter provides better context
      const colorMapping = {
        'variable': { foreground: 102 },  // light blue - variables
        // 'function' intentionally NOT mapped - let TextMate handle instructions
        // so that prefix (i32, local) and suffix (.add, .get) have different colors
      };
      return colorMapping[type];
    };
    console.log('Semantic token styling configured');
  } catch (e) {
    console.warn('Could not override theme token metadata:', e);
  }

  // Parse on content change (debounced)
  let parseTimeout;
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
function updateDiagnostics() {
  if (!watLSP || !editor) return;

  const diagnostics = watLSP.provideDiagnostics();
  const markers = diagnostics.map(diag => ({
    startLineNumber: diag.range.start.line + 1,
    startColumn: diag.range.start.character + 1,
    endLineNumber: diag.range.end.line + 1,
    endColumn: diag.range.end.character + 1,
    message: diag.message,
    severity: diag.severity === 1 ? monaco.MarkerSeverity.Error :
              diag.severity === 2 ? monaco.MarkerSeverity.Warning :
              diag.severity === 3 ? monaco.MarkerSeverity.Info :
              monaco.MarkerSeverity.Hint
  }));

  monaco.editor.setModelMarkers(editor.getModel(), 'wat-lsp', markers);

  // Update diagnostics panel
  const diagView = document.getElementById('diagnostics-view');
  if (diagView) {
    if (diagnostics.length === 0) {
      diagView.innerHTML = '<p class="placeholder">No errors</p>';
    } else {
      diagView.innerHTML = diagnostics.map(diag => {
        const line = diag.range.start.line + 1;
        const col = diag.range.start.character + 1;
        const severityClass = diag.severity === 1 ? 'error' :
                              diag.severity === 2 ? 'warning' : 'info';
        const severityIcon = diag.severity === 1 ? '⛔' :
                             diag.severity === 2 ? '⚠️' : 'ℹ️';
        return `<div class="diagnostic-item ${severityClass}" onclick="goToLine(${line}, ${col})">
          <span class="diagnostic-icon">${severityIcon}</span>
          <span class="diagnostic-location">[Ln ${line}, Col ${col}]</span>
          <span class="diagnostic-message">${escapeHtml(diag.message)}</span>
        </div>`;
      }).join('');
    }
  }
}

// Update LSP debug panel
function updateLSPDebugPanel() {
  if (!watLSP) return;

  const symbolTableView = document.getElementById('symbol-table-view');
  if (symbolTableView) {
    symbolTableView.innerHTML = watLSP.getSymbolTableHTML();
  }
}

// Update hover debug info
function updateHoverDebug(line, col, hover) {
  const hoverDebug = document.getElementById('hover-debug');
  if (!hoverDebug) return;

  if (!hover) {
    hoverDebug.innerHTML = `<p class="placeholder">No hover at Ln ${line}, Col ${col}</p>`;
    return;
  }

  const content = hover.contents?.value || '(no content)';
  const rangeInfo = hover.range
    ? `Ln ${hover.range.start.line + 1}:${hover.range.start.character + 1} - Ln ${hover.range.end.line + 1}:${hover.range.end.character + 1}`
    : 'No range';

  hoverDebug.innerHTML = `
    <div class="hover-debug-info">
      <div class="hover-position"><strong>Position:</strong> Ln ${line}, Col ${col}</div>
      <div class="hover-range"><strong>Range:</strong> ${rangeInfo}</div>
      <div class="hover-content"><strong>Content:</strong><pre>${escapeHtml(content)}</pre></div>
    </div>
  `;
}

// Helper to escape HTML
function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// Navigate to line/column in editor
function goToLine(line, col) {
  if (editor) {
    editor.setPosition({ lineNumber: line, column: col });
    editor.revealLineInCenter(line);
    editor.focus();
  }
}
window.goToLine = goToLine; // Expose for onclick handlers

// Register semantic tokens provider for tree-sitter based syntax highlighting
function registerSemanticTokensProvider() {
  if (!watLSP) {
    console.warn('registerSemanticTokensProvider: watLSP not available');
    return;
  }

  try {
    // Get the legend from the LSP
    const legend = watLSP.getSemanticTokensLegend();

    monaco.languages.registerDocumentSemanticTokensProvider('wat', {
      getLegend: () => legend,

      provideDocumentSemanticTokens: (model, lastResultId, token) => {
        if (!watLSP || !watLSP.ready) {
          console.log('provideDocumentSemanticTokens: LSP not ready');
          return { data: new Uint32Array(0) };
        }

        // Ensure document is parsed with latest content
        watLSP.parse(model.getValue());

        // Get semantic tokens from the LSP
        const tokens = watLSP.provideSemanticTokens();

        return {
          data: tokens,
          resultId: String(Date.now())
        };
      },

      releaseDocumentSemanticTokens: (resultId) => {
        // Nothing to clean up
      }
    });

    consoleOutput.info('Semantic tokens provider registered');
  } catch (e) {
    console.error('Failed to register semantic tokens provider:', e);
    consoleOutput.error('Failed to register semantic tokens: ' + e.message);
  }
}

// Initialize LSP
async function initLSP() {
  setLSPStatus(false, 'Loading WASM LSP...');

  try {
    // Initialize the WASM module first
    // The WASM file is served from /wat_lsp_rust_bg.wasm in public/
    await initWasm('/wat_lsp_rust_bg.wasm');

    watLSP = new WatLSP();
    const success = await watLSP.initialize();

    if (success) {
      setLSPStatus(true, 'LSP Ready (Rust WASM)');
      consoleOutput.info('LSP initialized with Rust WASM module');
      consoleOutput.info('Features: Hover, Go to Definition (F12), Find References (Shift+F12), Semantic Highlighting');

      // Register semantic tokens provider for syntax highlighting
      registerSemanticTokensProvider();

      // Initial parse
      if (editor) {
        watLSP.parse(editor.getValue());
        updateDiagnostics();
        updateLSPDebugPanel();
      }
    } else {
      setLSPStatus(false, 'LSP Error');
      consoleOutput.warn('LSP failed to initialize. Hover/definition/references will not work.');
    }

    // Expose for debugging
    window.watLSP = watLSP;

    return success;
  } catch (error) {
    console.error('Failed to initialize WASM LSP:', error);
    setLSPStatus(false, 'LSP Error');
    consoleOutput.error(`LSP initialization failed: ${error.message}`);
    return false;
  }
}

// Initialize wabt.js
async function initWabt() {
  wabt = await wabtInit();
  consoleOutput.info('wabt.js initialized');
}

// Compile WAT to WASM
async function compile() {
  const source = editor.getValue();

  setStatus('Compiling...', 'compiling');
  consoleOutput.clear();
  consoleOutput.info('Starting compilation...');

  // Clear previous markers
  monaco.editor.setModelMarkers(editor.getModel(), 'wat', []);

  try {
    // Parse WAT to module
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
      threads: false
    });

    // Validate
    module.validate();

    // Generate binary
    const result = module.toBinary({ log: false, write_debug_names: true });
    wasmBytes = result.buffer;

    consoleOutput.log(`Compiled successfully (${wasmBytes.byteLength} bytes)`);

    // Compile WebAssembly module
    wasmModule = await WebAssembly.compile(wasmBytes);

    // Display module info
    displayModuleInfo(wasmModule);

    // Update hex view
    displayHexView(wasmBytes);

    // Enable buttons
    document.getElementById('run-btn').disabled = false;
    document.getElementById('download-btn').disabled = false;

    setStatus('Compiled successfully', 'success');

    // Clean up
    module.destroy();

    return true;
  } catch (error) {
    consoleOutput.error(`Compilation failed: ${error.message}`);
    setStatus('Compilation failed', 'error');

    // Mark error in editor if possible
    const match = error.message.match(/input\.wat:(\d+):(\d+)/);
    if (match) {
      const lineNumber = parseInt(match[1], 10);
      const column = parseInt(match[2], 10);
      monaco.editor.setModelMarkers(editor.getModel(), 'wat', [{
        startLineNumber: lineNumber,
        startColumn: column,
        endLineNumber: lineNumber,
        endColumn: column + 1,
        message: error.message,
        severity: monaco.MarkerSeverity.Error
      }]);
    }

    return false;
  }
}

// Display module imports and exports
function displayModuleInfo(module) {
  const imports = WebAssembly.Module.imports(module);
  const exports = WebAssembly.Module.exports(module);

  // Display imports
  const importsList = document.getElementById('imports-list');
  if (imports.length === 0) {
    importsList.innerHTML = '<p class="placeholder">No imports</p>';
  } else {
    importsList.innerHTML = imports.map(imp => `
      <div class="import-item">
        <span class="item-name">${imp.module}.${imp.name}</span>
        <span class="item-type">${imp.kind}</span>
      </div>
    `).join('');
  }

  // Display exports
  const exportsList = document.getElementById('exports-list');
  if (exports.length === 0) {
    exportsList.innerHTML = '<p class="placeholder">No exports</p>';
  } else {
    exportsList.innerHTML = exports.map(exp => `
      <div class="export-item">
        <span class="item-name">${exp.name}</span>
        <span class="item-type">${exp.kind}</span>
      </div>
    `).join('');
  }

  // Populate function dropdown
  const fnSelect = document.getElementById('export-fn-select');
  const fnExports = exports.filter(e => e.kind === 'function');
  fnSelect.innerHTML = '<option value="">Select function...</option>' +
    fnExports.map(e => `<option value="${e.name}">${e.name}</option>`).join('');
  fnSelect.disabled = fnExports.length === 0;
  document.getElementById('fn-args').disabled = fnExports.length === 0;
  document.getElementById('call-fn-btn').disabled = fnExports.length === 0;
}

// Display hex view of WASM binary
function displayHexView(bytes) {
  const hexView = document.getElementById('hex-view');
  const arr = new Uint8Array(bytes);
  const lines = [];

  for (let i = 0; i < arr.length; i += 16) {
    const offset = i.toString(16).padStart(8, '0');
    const bytesHex = [];
    const ascii = [];

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

  hexView.innerHTML = lines.map(l => `<div class="hex-line">${l}</div>`).join('');
}

// Create import object with stubs
function createImportObject(module) {
  const imports = WebAssembly.Module.imports(module);
  const importObject = {};

  for (const imp of imports) {
    if (!importObject[imp.module]) {
      importObject[imp.module] = {};
    }

    switch (imp.kind) {
      case 'function':
        // Create a stub function that logs calls
        importObject[imp.module][imp.name] = (...args) => {
          consoleOutput.log(`[import] ${imp.module}.${imp.name}(${args.join(', ')})`);
          // Return 0 for functions that might need a return value
          return 0;
        };
        break;

      case 'memory':
        // Create a memory with reasonable defaults
        importObject[imp.module][imp.name] = new WebAssembly.Memory({ initial: 1, maximum: 10 });
        break;

      case 'table':
        // Create a table with reasonable defaults
        importObject[imp.module][imp.name] = new WebAssembly.Table({ initial: 10, element: 'anyfunc' });
        break;

      case 'global':
        // Create a global with a default value
        importObject[imp.module][imp.name] = new WebAssembly.Global({ value: 'i32', mutable: true }, 0);
        break;
    }
  }

  // Override specific imports with useful implementations
  if (importObject.env) {
    if (importObject.env.log) {
      importObject.env.log = (value) => {
        consoleOutput.log(`[log] ${value}`);
      };
    }
    if (importObject.env.logFloat) {
      importObject.env.logFloat = (value) => {
        consoleOutput.log(`[log] ${value}`);
      };
    }
  }

  return importObject;
}

// Instantiate and run the module
async function run() {
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
      const type = typeof value === 'function' ? 'function' : value.constructor.name;
      consoleOutput.log(`  ${name}: ${type}`);
    }

    setStatus('Module ready', 'success');
  } catch (error) {
    consoleOutput.error(`Instantiation failed: ${error.message}`);
    setStatus('Instantiation failed', 'error');
  }
}

// Call an exported function
function callExportedFunction() {
  const fnName = document.getElementById('export-fn-select').value;
  const argsStr = document.getElementById('fn-args').value;
  const resultEl = document.getElementById('fn-result');

  if (!fnName) {
    resultEl.textContent = 'Select a function first';
    resultEl.className = 'fn-result error';
    return;
  }

  if (!wasmInstance) {
    resultEl.textContent = 'Module not instantiated. Click "Run" first.';
    resultEl.className = 'fn-result error';
    return;
  }

  const fn = wasmInstance.exports[fnName];
  if (typeof fn !== 'function') {
    resultEl.textContent = `"${fnName}" is not a function`;
    resultEl.className = 'fn-result error';
    return;
  }

  // Parse arguments
  let args = [];
  if (argsStr.trim()) {
    args = argsStr.split(',').map(arg => {
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
    resultEl.textContent = `Result: ${result}`;
    resultEl.className = 'fn-result success';
  } catch (error) {
    consoleOutput.error(`Error: ${error.message}`);
    resultEl.textContent = `Error: ${error.message}`;
    resultEl.className = 'fn-result error';
  }
}

// Download WASM binary
function downloadWasm() {
  if (!wasmBytes) return;

  const blob = new Blob([wasmBytes], { type: 'application/wasm' });
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
function loadExample(name) {
  if (!name || !watExamples[name]) return;

  editor.setValue(watExamples[name]);
  monaco.editor.setModelMarkers(editor.getModel(), 'wat', []);

  // Reset state
  wasmModule = null;
  wasmInstance = null;
  wasmBytes = null;

  document.getElementById('run-btn').disabled = true;
  document.getElementById('download-btn').disabled = true;
  document.getElementById('imports-list').innerHTML = '<p class="placeholder">Compile to see imports</p>';
  document.getElementById('exports-list').innerHTML = '<p class="placeholder">Compile to see exports</p>';
  document.getElementById('export-fn-select').innerHTML = '<option value="">Select function...</option>';
  document.getElementById('export-fn-select').disabled = true;
  document.getElementById('fn-args').disabled = true;
  document.getElementById('call-fn-btn').disabled = true;
  document.getElementById('fn-result').textContent = '';
  document.getElementById('fn-result').className = 'fn-result';
  document.getElementById('hex-view').innerHTML = '';

  consoleOutput.clear();
  consoleOutput.info(`Loaded example: ${name}`);

  // Parse with LSP
  if (watLSP && watLSP.ready) {
    watLSP.parse(editor.getValue());
    updateLSPDebugPanel();
  }

  setStatus('Ready');
}

// Tab switching
function initTabs() {
  const tabs = document.querySelectorAll('.tab');
  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      // Remove active from all tabs and panes
      tabs.forEach(t => t.classList.remove('active'));
      document.querySelectorAll('.tab-pane').forEach(p => p.classList.remove('active'));

      // Activate clicked tab and corresponding pane
      tab.classList.add('active');
      const paneId = tab.dataset.tab;
      document.getElementById(paneId).classList.add('active');
    });
  });
}

// Initialize resizable panel
function initResizablePanel() {
  const resizeHandle = document.querySelector('.resize-handle');
  const outputPanel = document.querySelector('.output-panel');

  if (!resizeHandle || !outputPanel) return;

  // Create overlay to capture mouse events during resize (prevents Monaco from stealing events)
  const overlay = document.createElement('div');
  overlay.style.cssText = 'position: fixed; inset: 0; z-index: 9999; cursor: col-resize; display: none;';
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

    // Calculate new width (dragging left increases width, right decreases)
    const delta = startX - e.clientX;
    const newWidth = Math.min(Math.max(startWidth + delta, 200), window.innerWidth * 0.8);
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

// Initialize everything
async function init() {
  consoleOutput.init();
  consoleOutput.info('Initializing WAT LSP Playground...');

  initTabs();
  initResizablePanel();

  // Initialize Monaco, wabt, and LSP in parallel
  await Promise.all([
    initMonaco(),
    initWabt(),
    initLSP()
  ]);

  // Bind event handlers
  document.getElementById('compile-btn').addEventListener('click', compile);
  document.getElementById('run-btn').addEventListener('click', run);
  document.getElementById('download-btn').addEventListener('click', downloadWasm);
  document.getElementById('call-fn-btn').addEventListener('click', callExportedFunction);
  document.getElementById('examples').addEventListener('change', (e) => {
    loadExample(e.target.value);
  });

  // LSP Debug test hover button
  document.getElementById('test-hover-btn').addEventListener('click', () => {
    const line = parseInt(document.getElementById('test-line').value, 10) || 0;
    const col = parseInt(document.getElementById('test-col').value, 10) || 0;
    const resultEl = document.getElementById('test-result');

    if (!watLSP || !watLSP.ready) {
      resultEl.textContent = 'LSP not ready';
      return;
    }

    // Make sure we have latest parse
    watLSP.parse(editor.getValue());

    const hover = watLSP.provideHover(line, col);
    updateHoverDebug();

    if (hover) {
      resultEl.textContent = `Found: ${hover.contents.value.substring(0, 200)}...`;
    } else {
      resultEl.textContent = `No hover at line ${line}, col ${col}`;
    }
  });

  // Keyboard shortcuts
  document.addEventListener('keydown', (e) => {
    // Ctrl/Cmd + Enter to compile
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      compile();
    }
    // Ctrl/Cmd + Shift + Enter to compile and run
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'Enter') {
      e.preventDefault();
      compile().then(success => {
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
init().catch(error => {
  console.error('Failed to initialize:', error);
  setStatus('Initialization failed', 'error');
});
