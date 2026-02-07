import { useEffect, useRef } from 'react';
import * as monaco from 'monaco-editor';
import { createWatLSP } from '@emnudge/wat-lsp';
import treeSitterWasmUrl from '@emnudge/wat-lsp/wasm/tree-sitter.wasm?url';
import watLspWasmUrl from '@emnudge/wat-lsp/wasm/wat_lsp_rust_bg.wasm?url';
import * as vsctm from 'vscode-textmate';
import * as oniguruma from 'vscode-oniguruma';

// Types
interface WatLSP {
  ready: boolean;
  parse(source: string): void;
  provideHover(
    line: number,
    col: number,
  ): {
    contents: { value: string };
    range?: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  } | null;
  provideDefinition(
    line: number,
    col: number,
  ): {
    range: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  } | null;
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
    col: number,
  ): Array<{
    label: string;
    kind?: number;
    detail?: string;
    insertText?: string;
    insertTextRules?: number;
    documentation?: string;
  }>;
  getSemanticTokensLegend(): monaco.languages.SemanticTokensLegend;
  provideSemanticTokens(): Uint32Array;
}

// Global state
let watLSP: WatLSP | null = null;
let watGrammar: vsctm.IGrammar | null = null;
let languageRegistered = false;
let lspInitialized = false;
let initPromise: Promise<void> | null = null;

// TextMate state class
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

// WAT language configuration
const watLanguageConfig: monaco.languages.LanguageConfiguration = {
  comments: {
    lineComment: ';;',
    blockComment: ['(;', ';)'],
  },
  brackets: [['(', ')']],
  autoClosingPairs: [
    { open: '(', close: ')' },
    { open: '"', close: '"', notIn: ['string'] },
  ],
  surroundingPairs: [
    { open: '(', close: ')' },
    { open: '"', close: '"' },
  ],
};

async function initializeMonacoAndLSP(): Promise<void> {
  if (lspInitialized) return;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      // Load oniguruma WASM
      const onigResponse = await fetch('/onig.wasm');
      const onigBuffer = await onigResponse.arrayBuffer();
      await oniguruma.loadWASM(onigBuffer);

      // Create oniguruma library interface
      const onigLib = {
        createOnigScanner: (patterns: string[]) => new oniguruma.OnigScanner(patterns),
        createOnigString: (s: string) => new oniguruma.OnigString(s),
      };

      // Create TextMate registry
      const registry = new vsctm.Registry({
        onigLib: Promise.resolve(onigLib),
        loadGrammar: async (scopeName: string) => {
          if (scopeName === 'source.wat') {
            const response = await fetch('/wat.tmLanguage.json');
            const grammar = await response.json();
            return vsctm.parseRawGrammar(JSON.stringify(grammar), 'wat.tmLanguage.json');
          }
          return null;
        },
      });

      // Load the WAT grammar
      watGrammar = await registry.loadGrammar('source.wat');

      // Register WAT language if not already done
      if (!languageRegistered) {
        monaco.languages.register({
          id: 'wat',
          extensions: ['.wat', '.wast'],
          aliases: ['WebAssembly Text', 'WAT'],
        });

        monaco.languages.setLanguageConfiguration('wat', watLanguageConfig);

        // Register TextMate-based token provider
        monaco.languages.setTokensProvider('wat', {
          getInitialState: () => new TMState(vsctm.INITIAL),
          tokenize: (
            line: string,
            state: monaco.languages.IState,
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

        languageRegistered = true;
      }

      // Initialize the LSP
      const lsp = await createWatLSP({
        treeSitterWasmPath: treeSitterWasmUrl,
        watLspWasmPath: watLspWasmUrl,
      });
      watLSP = lsp as WatLSP;

      // Register hover provider
      monaco.languages.registerHoverProvider('wat', {
        provideHover: (
          model: monaco.editor.ITextModel,
          position: monaco.Position,
        ): monaco.languages.ProviderResult<monaco.languages.Hover> => {
          if (!watLSP || !watLSP.ready) return null;

          watLSP.parse(model.getValue());
          const hover = watLSP.provideHover(position.lineNumber - 1, position.column - 1);

          if (!hover) return null;

          return {
            contents: [{ value: hover.contents.value }],
            range: hover.range
              ? new monaco.Range(
                  hover.range.start.line + 1,
                  hover.range.start.character + 1,
                  hover.range.end.line + 1,
                  hover.range.end.character + 1,
                )
              : undefined,
          };
        },
      });

      // Register completion provider
      monaco.languages.registerCompletionItemProvider('wat', {
        triggerCharacters: ['.', '$', '@', '2', '4'],
        provideCompletionItems: (
          model: monaco.editor.ITextModel,
          position: monaco.Position,
        ): monaco.languages.ProviderResult<monaco.languages.CompletionList> => {
          if (!watLSP || !watLSP.ready) return { suggestions: [] };

          watLSP.parse(model.getValue());
          const completions = watLSP.provideCompletion(
            position.lineNumber - 1,
            position.column - 1,
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
        },
      });

      // Register definition provider
      monaco.languages.registerDefinitionProvider('wat', {
        provideDefinition: (
          model: monaco.editor.ITextModel,
          position: monaco.Position,
        ): monaco.languages.ProviderResult<monaco.languages.Definition> => {
          if (!watLSP || !watLSP.ready) return null;

          watLSP.parse(model.getValue());
          const definition = watLSP.provideDefinition(position.lineNumber - 1, position.column - 1);

          if (!definition || !definition.range) return null;

          return {
            uri: model.uri,
            range: new monaco.Range(
              definition.range.start.line + 1,
              definition.range.start.character + 1,
              definition.range.end.line + 1,
              definition.range.end.character + 1,
            ),
          };
        },
      });

      // Register semantic tokens provider
      try {
        const legend = watLSP.getSemanticTokensLegend();
        monaco.languages.registerDocumentSemanticTokensProvider('wat', {
          getLegend: () => legend,
          provideDocumentSemanticTokens: (
            model: monaco.editor.ITextModel,
          ): monaco.languages.ProviderResult<monaco.languages.SemanticTokens> => {
            if (!watLSP || !watLSP.ready) {
              return { data: new Uint32Array(0) };
            }
            watLSP.parse(model.getValue());
            const tokens = watLSP.provideSemanticTokens();
            return {
              data: tokens,
              resultId: String(Date.now()),
            };
          },
          releaseDocumentSemanticTokens: () => {},
        });
      } catch (e) {
        console.warn('Failed to register semantic tokens provider:', e);
      }

      // Define custom theme
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
        colors: {},
      });

      lspInitialized = true;
      console.log('[WAT LSP] Initialized successfully');
    } catch (err) {
      console.error('[WAT LSP] Failed to initialize:', err);
      // Fall back to basic mode - just register the language without LSP
      if (!languageRegistered) {
        monaco.languages.register({
          id: 'wat',
          extensions: ['.wat', '.wast'],
        });
        monaco.languages.setLanguageConfiguration('wat', watLanguageConfig);
        languageRegistered = true;
      }
    }
  })();

  return initPromise;
}

async function enhanceCodeBlock(wrapper: HTMLElement) {
  if (wrapper.dataset.monacoEnhanced) return;
  wrapper.dataset.monacoEnhanced = 'true';

  // Extract code from ec-line divs (each div is one line)
  const lines = wrapper.querySelectorAll('.ec-line');
  let code = '';

  if (lines.length > 0) {
    code = Array.from(lines)
      .map((line) => (line.textContent || '').replace(/^\n+|\n+$/g, ''))
      .join('\n');
  } else {
    // Fallback: use code element
    const codeEl = wrapper.querySelector('code');
    code = codeEl?.textContent || '';
  }

  if (!code.trim()) return;
  const lineCount = code.split('\n').length;
  const height = Math.max(120, Math.min(400, lineCount * 20 + 40));

  await initializeMonacoAndLSP();

  // Create container
  const container = document.createElement('div');
  container.className = 'wat-editor-container';
  container.style.cssText = `
    height: ${height}px;
    border-radius: 8px;
    overflow: auto;
    margin-block: 1rem;
  `;

  // Replace the wrapper
  wrapper.replaceWith(container);

  // Create editor
  const editor = monaco.editor.create(container, {
    value: code,
    language: 'wat',
    theme: 'wat-dark',
    readOnly: true,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    fontSize: 14,
    lineNumbers: 'on',
    folding: true,
    automaticLayout: true,
    wordWrap: 'off',
    padding: { top: 8, bottom: 8 },
    scrollbar: {
      vertical: 'auto',
      horizontal: 'auto',
      verticalScrollbarSize: 8,
      horizontalScrollbarSize: 8,
    },
    overviewRulerBorder: false,
    renderLineHighlight: 'none',
    contextmenu: false,
    'semanticHighlighting.enabled': true,
  });

  // Force font remeasurement after a short delay
  setTimeout(() => {
    editor.layout();
    monaco.editor.remeasureFonts();
  }, 100);

  // Parse for diagnostics display
  if (watLSP && watLSP.ready) {
    watLSP.parse(code);
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
            : monaco.MarkerSeverity.Info,
    }));
    monaco.editor.setModelMarkers(editor.getModel()!, 'wat-lsp', markers);
  }
}

async function enhanceAllWatBlocks() {
  const pres = document.querySelectorAll('pre[data-language="wat"], pre[data-language="wast"]');

  for (const pre of pres) {
    const wrapper = pre.closest('.expressive-code');
    if (wrapper && !wrapper.hasAttribute('data-monaco-enhanced')) {
      await enhanceCodeBlock(wrapper as HTMLElement);
    }
  }
}

export default function MonacoEnhancer() {
  const hasRun = useRef(false);

  useEffect(() => {
    if (hasRun.current) return;
    hasRun.current = true;

    enhanceAllWatBlocks();

    const handlePageLoad = () => enhanceAllWatBlocks();
    document.addEventListener('astro:page-load', handlePageLoad);

    return () => {
      document.removeEventListener('astro:page-load', handlePageLoad);
    };
  }, []);

  return null;
}
