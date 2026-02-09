<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import * as monaco from 'monaco-editor';
import { usePlaygroundStore } from '../store/playground';
import { examples } from '../../examples';
import { watLanguage } from '../../wat-language';
import { createWatLSP } from '@emnudge/wat-lsp';
import * as vsctm from 'vscode-textmate';
import * as oniguruma from 'vscode-oniguruma';
import EditorTabs from './EditorTabs.vue';
// @ts-ignore
import { StandaloneServices } from 'monaco-editor/esm/vs/editor/standalone/browser/standaloneServices';

const store = usePlaygroundStore();
const editorContainer = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let watLSP: any = null;
let watGrammar: vsctm.IGrammar | null = null;
let isUpdatingFromStore = false;

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
  clone(): TMState { return new TMState(this.ruleStack); }
  equals(other: monaco.languages.IState): boolean {
    if (!other || !(other instanceof TMState)) return false;
    if (!this.ruleStack && !other.ruleStack) return true;
    if (!this.ruleStack || !other.ruleStack) return false;
    return this.ruleStack.equals(other.ruleStack);
  }
}
const models = new Map<string, monaco.editor.ITextModel>();
const viewStates = new Map<string, monaco.editor.ICodeEditorViewState | null>();

onMounted(async () => {
  if (!editorContainer.value) return;
  await initMonaco();
});

onBeforeUnmount(() => {
  editor?.dispose();
  models.forEach(model => model.dispose());
});

async function initMonaco() {
  // Configure Monaco web workers to avoid fallback warning
  self.MonacoEnvironment = {
    getWorker: function () {
      return new Worker(
        new URL('monaco-editor/esm/vs/editor/editor.worker.js', import.meta.url),
        { type: 'module' }
      );
    }
  };

  // Load oniguruma WASM
  try {
    const onigResponse = await fetch('./onig.wasm');
    const onigBuffer = await onigResponse.arrayBuffer();
    await oniguruma.loadWASM(onigBuffer);
    console.log('Oniguruma WASM loaded');
  } catch (e) {
    console.error('Failed to load Oniguruma WASM:', e);
  }

  // Register WAT language
  monaco.languages.register({
    id: 'wat',
    extensions: ['.wat', '.wast'],
    aliases: ['WebAssembly Text', 'WAT'],
  });

  monaco.languages.setLanguageConfiguration('wat', watLanguage.languageConfiguration);

  // Set up TextMate grammar
  const onigLib = {
    createOnigScanner: (patterns: string[]) => new oniguruma.OnigScanner(patterns),
    createOnigString: (s: string) => new oniguruma.OnigString(s),
  };

  const registry = new vsctm.Registry({
    onigLib: Promise.resolve(onigLib),
    loadGrammar: async (scopeName) => {
      if (scopeName === 'source.wat') {
        const response = await fetch('./wat.tmLanguage.json');
        const grammar = await response.json();
        return vsctm.parseRawGrammar(JSON.stringify(grammar), 'wat.tmLanguage.json');
      }
      return null;
    },
  });

  watGrammar = await registry.loadGrammar('source.wat');

  monaco.languages.setTokensProvider('wat', {
    getInitialState: () => new TMState(vsctm.INITIAL),
    tokenize: (line, state) => {
      if (!watGrammar) return { tokens: [], endState: state };
      const tmState = state as TMState;
      const result = watGrammar.tokenizeLine(line, tmState.ruleStack);
      const tokens: monaco.languages.IToken[] = [];
      for (const token of result.tokens) {
        let tokenType = '';
        for (let i = token.scopes.length - 1; i >= 0; i--) {
          const mapped = scopeToToken(token.scopes[i]);
          if (mapped) {
            tokenType = mapped;
            break;
          }
        }
        tokens.push({ startIndex: token.startIndex, scopes: tokenType });
      }
      return { tokens, endState: new TMState(result.ruleStack) };
    },
  });

  // Set up theme
  monaco.editor.defineTheme('flexoki-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [],
    colors: {
      'editor.background': '#10100e',
      'editor.foreground': '#cecdc3',
      'editorLineNumber.foreground': '#40403e',
      'editor.selectionBackground': '#282826',
      'editorCursor.foreground': '#cecdc3',
    },
  });

  editor = monaco.editor.create(editorContainer.value!, {
    language: 'wat',
    theme: 'flexoki-dark',
    fontSize: 14,
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    minimap: { enabled: false },
    automaticLayout: true,
    tabSize: 2,
    scrollBeyondLastLine: false,
    lineNumbers: 'on',
  });

  // Expose for Playwright tests
  (window as any).monacoEditor = editor;
  (window as any).monaco = monaco;

  // Initial model setup
  if (store.currentFile) {
    const model = monaco.editor.createModel(store.currentFile.code, 'wat');
    models.set(store.currentFile.id, model);
    editor.setModel(model);
  }

  // Native Quick Open using Monaco's internal services
  editor.addAction({
    id: 'builtin.quickOpen',
    label: 'Go to File...',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyP],
    run: (ed) => {
      console.log('Quick Open Action triggered');
      
      // Try StandaloneServices first if available
      let quickInputService: any;
      try {
        // Search by name in common symbols/strings
        quickInputService = (StandaloneServices as any).get('IQuickInputService');
      } catch (e) {
        console.log('StandaloneServices.get failed, falling back to discovery');
      }

      if (!quickInputService) {
        // Robust discovery of QuickInput service
        quickInputService = (ed as any)._getService?.('IQuickInputService');
      }
      
      if (!quickInputService) {
        console.log('IQuickInputService not found via conventional means, searching in instantiation service...');
        const instantiationService = (ed as any)._instantiationService;
        const services = instantiationService?._services || (instantiationService?._serviceCollection?._entries);
        
        if (services) {
          const serviceValues = services instanceof Map ? services.values() : Object.values(services);
          for (const service of serviceValues) {
            const s = (service as any)._service || service;
            // The service that has 'pick' and 'provide' or 'input' is usually the QuickInputService
            if (s && typeof s.pick === 'function' && (typeof s.input === 'function' || typeof s.createQuickPick === 'function')) {
              quickInputService = s;
              console.log('Found QuickInputService in service collection via heuristics');
              break;
            }
          }
        }
      }

      if (!quickInputService) {
        console.error('QuickInputService NOT found! Defaulting to command palette.');
        ed.trigger('any', 'editor.action.quickCommand', {});
        return;
      }

      const openIds = new Set(store.openFiles.map(f => f.id));
      const recentIds = store.recentlyUsedIds.filter(id => openIds.has(id));

      console.log('Quick Open Picks:', { openIds: Array.from(openIds), recentIds });

      const picks: any[] = [];
      
      // Open editors first
      if (recentIds.length > 0) {
        picks.push({ type: 'separator', label: 'open editors' });
        for (const id of recentIds) {
          const file = store.openFiles.find(f => f.id === id);
          if (file) {
            picks.push({
              label: file.filename,
              description: file.id.startsWith('invalid_') ? 'invalid/' : 'examples/',
              id: file.id
            });
          }
        }
      }

      // Other examples
      const otherExamples = examples.filter(ex => !openIds.has(ex.id));
      if (otherExamples.length > 0) {
        picks.push({ type: 'separator', label: 'examples' });
        for (const ex of otherExamples) {
          picks.push({
            label: ex.filename,
            description: ex.id.startsWith('invalid_') ? 'invalid/' : 'examples/',
            id: ex.id
          });
        }
      }

      quickInputService.pick(picks, { placeHolder: 'Select a file to open' }).then((selected: any) => {
        if (selected) {
          store.openFile(selected.id);
        }
      });
    }
  });

  editor.onDidChangeCursorPosition((e) => {
    if (!watLSP || !editor) return;
    const model = editor.getModel();
    if (!model) return;
    watLSP.parse(model.getValue());
    const result = watLSP.provideHover(e.position.lineNumber - 1, e.position.column - 1);
    store.setHoverInfo({
      line: e.position.lineNumber,
      col: e.position.column,
      result: result ?? null,
    });
  });

  editor.onDidChangeModelContent(() => {
    if (isUpdatingFromStore) return;
    const value = editor?.getValue() || '';
    store.updateCode(value);
    updateDiagnostics(value);
    updateSymbols(value);
  });

  // Load LSP
  try {
    watLSP = await createWatLSP({
      treeSitterWasmPath: './tree-sitter.wasm',
      watLspWasmPath: './wat_lsp_rust_bg.wasm'
    });
    store.lspReady = true;
    store.lspStatus = 'LSP Ready';
    
    // Initial diagnostics and symbols
    if (store.currentFile) {
        updateDiagnostics(store.currentFile.code);
        updateSymbols(store.currentFile.code);
    }
    
    // Register providers (hover, completion, etc.) using watLSP
    registerLSPProviders();
  } catch (e) {
    console.error('LSP Initialization Error:', e);
    store.lspStatus = 'LSP Error';
  }
}

function updateSymbols(code: string) {
  if (!watLSP) return;
  watLSP.parse(code);
  const symbols = watLSP.provideDocumentSymbols();
  store.setSymbols(symbols);
}

function updateDiagnostics(code: string) {
  if (!watLSP || !editor) return;

  const model = editor.getModel();
  if (!model) return;

  watLSP.parse(code);
  const diagnostics = watLSP.provideDiagnostics();
  
  const markers = diagnostics.map((d: any) => ({
    severity: monaco.MarkerSeverity.Error,
    message: d.message,
    startLineNumber: d.range.start.line + 1,
    startColumn: d.range.start.character + 1,
    endLineNumber: d.range.end.line + 1,
    endColumn: d.range.end.character + 1,
  }));

  monaco.editor.setModelMarkers(model, 'wat', markers);
  store.diagnostics = markers;
}

function registerLSPProviders() {
  if (!watLSP) return;

  monaco.languages.registerHoverProvider('wat', {
    provideHover: (model, position) => {
      watLSP.parse(model.getValue());
      const hover = watLSP.provideHover(position.lineNumber - 1, position.column - 1);
      if (!hover) return null;
      return {
        contents: [{ value: hover.contents.value }],
        range: hover.range ? new monaco.Range(
          hover.range.start.line + 1, hover.range.start.character + 1,
          hover.range.end.line + 1, hover.range.end.character + 1
        ) : undefined
      };
    }
  });

  monaco.languages.registerDefinitionProvider('wat', {
    provideDefinition: (model, position) => {
      watLSP.parse(model.getValue());
      const def = watLSP.provideDefinition(position.lineNumber - 1, position.column - 1);
      if (!def) return null;
      return {
        uri: model.uri,
        range: new monaco.Range(
          def.range.start.line + 1, def.range.start.character + 1,
          def.range.end.line + 1, def.range.end.character + 1
        )
      };
    }
  });

  monaco.languages.registerReferenceProvider('wat', {
    provideReferences: (model, position, context) => {
      watLSP.parse(model.getValue());
      const refs = watLSP.provideReferences(
        position.lineNumber - 1,
        position.column - 1,
        context.includeDeclaration
      );
      if (!refs || refs.length === 0) return [];
      return refs.map((ref: any) => ({
        uri: model.uri,
        range: new monaco.Range(
          ref.range.start.line + 1, ref.range.start.character + 1,
          ref.range.end.line + 1, ref.range.end.character + 1
        )
      }));
    }
  });

  monaco.languages.registerCompletionItemProvider('wat', {
    triggerCharacters: ['.', '$', '@', '2', '4'],
    provideCompletionItems: (model, position) => {
      watLSP.parse(model.getValue());
      const completions = watLSP.provideCompletion(
        position.lineNumber - 1,
        position.column - 1
      );
      if (!completions || completions.length === 0) return { suggestions: [] };
      return {
        suggestions: completions.map((item: any) => ({
          label: item.label,
          kind: item.kind ?? monaco.languages.CompletionItemKind.Text,
          detail: item.detail,
          documentation: item.documentation,
          insertText: item.insertText ?? item.label,
          insertTextRules: item.insertTextRules,
          range: undefined as any,
        }))
      };
    }
  });
}

watch(() => store.activeFileId, (newId, oldId) => {
  if (!editor || !newId) return;

  // Save current view state
  if (oldId) {
    viewStates.set(oldId, editor.saveViewState());
  }

  const file = store.currentFile;
  if (!file) return;

  let model = models.get(newId);
  if (!model) {
    model = monaco.editor.createModel(file.code, 'wat');
    models.set(newId, model);
  }

  isUpdatingFromStore = true;
  editor.setModel(model);
  
  // Restore view state
  const state = viewStates.get(newId);
  if (state) {
    editor.restoreViewState(state);
  }
  
  editor.focus();
  
  updateDiagnostics(file.code);
  updateSymbols(file.code);
  isUpdatingFromStore = false;
});

// Watch for closed files to cleanup models
watch(() => store.openFiles.length, () => {
  const openIds = new Set(store.openFiles.map(f => f.id));
  models.forEach((model, id) => {
    if (!openIds.has(id)) {
      model.dispose();
      models.delete(id);
      viewStates.delete(id);
    }
  });
});
</script>

<template>
  <div class="editor-view">
    <EditorTabs />
    <div class="editor-container" ref="editorContainer"></div>
  </div>
</template>

<style scoped>
.editor-view {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
}

.editor-container {
  flex: 1;
  min-height: 0;
}
</style>
