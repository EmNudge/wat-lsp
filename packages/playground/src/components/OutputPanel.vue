<script setup lang="ts">
import { ref, computed } from 'vue';
import { usePlaygroundStore } from '../store/playground';

const store = usePlaygroundStore();
const activeTab = ref('console');

const tabs = [
  { id: 'info', label: 'Module Info' },
  { id: 'outline', label: 'Outline' },
  { id: 'console', label: 'Console' },
  { id: 'hex', label: 'Hex View' },
  { id: 'hover', label: 'Hover' },
];

function formatHex(buffer: Uint8Array) {
  let result = '';
  for (let i = 0; i < buffer.length; i += 16) {
    const chunk = buffer.slice(i, i + 16);
    const address = i.toString(16).padStart(8, '0');
    const hex = Array.from(chunk).map(b => b.toString(16).padStart(2, '0')).join(' ');
    const ascii = Array.from(chunk).map(b => (b >= 32 && b <= 126) ? String.fromCharCode(b) : '.').join('');
    result += `${address}  ${hex.padEnd(47, ' ')}  |${ascii}|\n`;
  }
  return result;
}

const hexView = computed(() => {
  if (!store.wasmBytes) return 'Compile to see hex view';
  return formatHex(store.wasmBytes);
});
</script>

<template>
  <div class="output-panel">
    <div class="tabs">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="tab-btn"
        :class="{ active: activeTab === tab.id }"
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
      </button>
      <div class="tabs-spacer"></div>
      <button
        class="btn btn-primary compile-btn"
        @click="store.compile()"
        :disabled="store.isCompiling"
      >
        {{ store.isCompiling ? 'Compiling...' : 'Compile' }}
      </button>
    </div>

    <div class="tab-content">
      <div v-if="activeTab === 'console'" class="console-pane">
        <div v-for="(log, i) in store.consoleOutput" :key="i" class="log-line" :class="log.type">
          <span class="timestamp">[{{ log.timestamp }}]</span>
          <span class="message">{{ log.message }}</span>
        </div>
        <div v-if="store.consoleOutput.length === 0" class="placeholder">
          Console is empty.
        </div>
      </div>
      
      <div v-else-if="activeTab === 'info'" class="info-pane">
        <div class="section">
          <h3>Imports</h3>
          <div v-if="store.moduleImports.length > 0">
            <div v-for="(imp, i) in store.moduleImports" :key="i" class="info-item">
              <span class="kind">{{ imp.kind }}</span>
              <span class="name">{{ imp.module }}.{{ imp.name }}</span>
            </div>
          </div>
          <p v-else class="placeholder">No imports</p>
        </div>
        <div class="section">
          <h3>Exports</h3>
          <div v-if="store.moduleExports.length > 0">
            <div v-for="(exp, i) in store.moduleExports" :key="i" class="info-item">
              <span class="kind">{{ exp.kind }}</span>
              <span class="name">{{ exp.name }}</span>
            </div>
          </div>
          <p v-else class="placeholder">No exports</p>
        </div>
      </div>

      <div v-else-if="activeTab === 'outline'" class="outline-pane">
        <div v-if="store.symbols.length > 0">
          <div v-for="(sym, i) in store.symbols" :key="i" class="outline-item">
            <span class="kind">[{{ sym.kind }}]</span>
            <span class="name">{{ sym.name }}</span>
          </div>
        </div>
        <p v-else class="placeholder">No symbols found</p>
      </div>

      <div v-else-if="activeTab === 'hex'" class="hex-pane">
        <pre class="hex-dump">{{ hexView }}</pre>
      </div>

      <div v-else-if="activeTab === 'hover'" class="hover-pane">
        <template v-if="store.hoverInfo">
          <div class="hover-position">Line {{ store.hoverInfo.line }}, Col {{ store.hoverInfo.col }}</div>
          <template v-if="store.hoverInfo.result">
            <pre class="hover-content">{{ store.hoverInfo.result.contents?.value ?? 'No content' }}</pre>
          </template>
          <p v-else class="placeholder">No hover at this position</p>
        </template>
        <p v-else class="placeholder">Move cursor in editor to see hover info</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.output-panel {
  display: flex;
  flex-direction: column;
  background-color: var(--bg-soft);
  height: 100%;
  min-height: 0;
}

.tabs {
  display: flex;
  background-color: var(--bg-muted);
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.tab-btn {
  padding: 0.5rem 1rem;
  background: none;
  border: none;
  border-right: 1px solid var(--border-color);
  color: var(--fg-muted);
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s;
}

.tab-btn:hover {
  background-color: var(--bg-soft);
  color: var(--fg-color);
}

.tab-btn.active {
  background-color: var(--bg-soft);
  color: var(--primary-color);
  border-bottom: 2px solid var(--primary-color);
}

.tabs-spacer {
  flex: 1;
}

.compile-btn {
  margin: 0.25rem 0.5rem;
  padding: 0.25rem 0.75rem;
  font-size: 0.8rem;
  flex-shrink: 0;
}

.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 1rem;
  min-height: 0;
}

.console-pane {
  font-family: var(--font-mono);
  font-size: 0.85rem;
}

.log-line {
  margin-bottom: 0.25rem;
}

.timestamp {
  color: var(--fg-muted);
  margin-right: 0.5rem;
}

.message {
  white-space: pre-wrap;
}

.log-line.error .message {
  color: var(--error-color);
}

.log-line.warn .message {
  color: var(--warning-color);
}

.info-item, .outline-item {
  display: flex;
  gap: 1rem;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  margin-bottom: 0.25rem;
}

.kind {
  color: var(--fg-muted);
  min-width: 60px;
}

.name {
  color: var(--fg-color);
}

.hex-dump {
  font-family: var(--font-mono);
  font-size: 0.85rem;
  margin: 0;
  white-space: pre;
  color: var(--fg-color);
}

.placeholder {
  color: var(--fg-muted);
  font-style: italic;
  font-size: 0.9rem;
}

.section h3 {
  font-size: 0.9rem;
  margin: 1rem 0 0.5rem;
  color: var(--fg-color);
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 0.25rem;
}

.section:first-child h3 {
  margin-top: 0;
}

.hover-pane {
  font-family: var(--font-mono);
  font-size: 0.85rem;
}

.hover-position {
  color: var(--fg-muted);
  margin-bottom: 0.5rem;
}

.hover-content {
  margin: 0;
  white-space: pre-wrap;
  color: var(--fg-color);
}
</style>
