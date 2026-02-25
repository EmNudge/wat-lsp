<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import * as monaco from 'monaco-editor';
import { usePlaygroundStore } from '../store/playground';
import { extractWatTypes, generateHostDts } from '../codegen';

const store = usePlaygroundStore();
const editorContainer = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let model: monaco.editor.ITextModel | null = null;
let isUpdatingFromStore = false;
let extraLibDisposable: monaco.IDisposable | null = null;

onMounted(() => {
  if (!editorContainer.value) return;
  initEditor();
});

onBeforeUnmount(() => {
  extraLibDisposable?.dispose();
  editor?.dispose();
  model?.dispose();
});

function initEditor() {
  // Configure JS/TS defaults for IntelliSense
  const jsDefaults = monaco.languages.typescript.javascriptDefaults;
  jsDefaults.setCompilerOptions({
    target: monaco.languages.typescript.ScriptTarget.ESNext,
    module: monaco.languages.typescript.ModuleKind.ESNext,
    moduleDetection: 3, // Force — treat every file as a module (suppresses "no imports/exports" warning)
    allowJs: true,
    checkJs: false,
    noEmit: true,
    lib: ['esnext', 'dom'],
    allowNonTsExtensions: true,
  });
  jsDefaults.setDiagnosticsOptions({
    noSemanticValidation: false,
    noSyntaxValidation: false,
  });

  // Add initial .d.ts
  updateExtraLib();

  // Create a model with a proper file URI so the TS worker can resolve it
  const uri = monaco.Uri.parse('file:///host.js');
  model = monaco.editor.createModel(
    store.currentFile?.hostCode ?? '',
    'javascript',
    uri
  );

  editor = monaco.editor.create(editorContainer.value!, {
    model,
    theme: 'flexoki-dark',
    fontSize: 14,
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    minimap: { enabled: false },
    automaticLayout: true,
    tabSize: 2,
    scrollBeyondLastLine: false,
    lineNumbers: 'on',
  });

  editor.onDidChangeModelContent(() => {
    if (isUpdatingFromStore) return;
    const value = editor?.getValue() || '';
    store.updateHostCode(value);
  });
}

function updateExtraLib() {
  extraLibDisposable?.dispose();
  if (store.currentFile) {
    const info = extractWatTypes(store.currentFile.code);
    const dts = generateHostDts(info);
    extraLibDisposable = monaco.languages.typescript.javascriptDefaults.addExtraLib(
      dts,
      'file:///wasm-host.d.ts'
    );
  }
}

// Watch for host code changes from store (regeneration)
watch(
  () => store.currentFile?.hostCode,
  (newCode) => {
    if (!model || newCode === undefined) return;
    if (model.getValue() === newCode) return;
    isUpdatingFromStore = true;
    model.setValue(newCode);
    isUpdatingFromStore = false;
  }
);

// Watch for file switches
watch(
  () => store.activeFileId,
  () => {
    if (!model) return;
    isUpdatingFromStore = true;
    model.setValue(store.currentFile?.hostCode ?? '');
    updateExtraLib();
    isUpdatingFromStore = false;
  }
);

// Watch for WASM module changes to update .d.ts
watch(
  () => store.wasmModule,
  () => {
    updateExtraLib();
  }
);

function regenerate() {
  store.generateHost();
}

function resetHost() {
  store.resetHostCode();
}
</script>

<template>
  <div class="js-editor-view">
    <div class="js-editor-header">
      <span class="js-filename">host.js</span>
      <div class="js-editor-actions">
        <button
          v-if="store.currentFile?.hostDirty"
          class="btn-small"
          @click="resetHost"
          title="Reset to auto-generated host code"
        >
          Reset
        </button>
        <button class="btn-small" @click="regenerate" title="Regenerate host code from WAT source">
          Regenerate
        </button>
      </div>
    </div>
    <div class="js-editor-container" ref="editorContainer"></div>
  </div>
</template>

<style scoped>
.js-editor-view {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
}

.js-editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.25rem 0.75rem;
  background-color: var(--bg-muted);
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.js-filename {
  font-size: 0.8rem;
  color: var(--fg-muted);
  font-family: var(--font-mono);
}

.js-editor-actions {
  display: flex;
  gap: 0.25rem;
}

.btn-small {
  padding: 0.15rem 0.5rem;
  font-size: 0.75rem;
  background: none;
  border: 1px solid var(--border-color);
  color: var(--fg-muted);
  border-radius: 3px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-small:hover {
  background-color: var(--bg-soft);
  color: var(--fg-color);
  border-color: var(--fg-muted);
}

.js-editor-container {
  flex: 1;
  min-height: 0;
}
</style>
