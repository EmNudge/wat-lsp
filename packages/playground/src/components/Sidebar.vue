<script setup lang="ts">
import { usePlaygroundStore } from '../store/playground';
import { examples } from '../../examples';
import { ref, computed } from 'vue';

const store = usePlaygroundStore();
const isExamplesExpanded = ref(true);
const isInvalidExpanded = ref(false);

const validExamples = computed(() => examples.filter(ex => !ex.id.startsWith('invalid_')));
const invalidExamples = computed(() => examples.filter(ex => ex.id.startsWith('invalid_')));

function selectExample(id: string) {
  store.openFile(id);
}
</script>

<template>
  <aside class="sidebar">
    <div class="panel-header">
      <span>Examples</span>
    </div>
    <div class="tree-container">
      <div class="tree-group">
        <div class="tree-group-header" @click="isExamplesExpanded = !isExamplesExpanded">
          <span class="chevron">{{ isExamplesExpanded ? '▼' : '▶' }}</span>
          <span class="icon">📁</span>
          <span>examples</span>
        </div>
        <div v-show="isExamplesExpanded" class="tree-items">
          <div
            v-for="ex in validExamples"
            :key="ex.id"
            class="tree-item"
            :data-id="ex.id"
            :class="{
              selected: store.activeFileId === ex.id,
              'has-error': store.activeFileId === ex.id && store.diagnostics.length > 0
            }"
            @click="selectExample(ex.id)"
          >
            <span class="icon">📄</span>
            <span class="file-name">{{ ex.filename }}</span>
          </div>
        </div>
      </div>

      <div v-if="invalidExamples.length > 0" class="tree-group">
        <div class="tree-group-header" @click="isInvalidExpanded = !isInvalidExpanded">
          <span class="chevron">{{ isInvalidExpanded ? '▼' : '▶' }}</span>
          <span class="icon">📁</span>
          <span>invalid</span>
        </div>
        <div v-show="isInvalidExpanded" class="tree-items">
          <div
            v-for="ex in invalidExamples"
            :key="ex.id"
            class="tree-item"
            :data-id="ex.id"
            :class="{
              selected: store.activeFileId === ex.id,
              'has-error': true
            }"
            @click="selectExample(ex.id)"
          >
            <span class="icon">⚠️</span>
            <span class="file-name">{{ ex.filename }}</span>
          </div>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  background-color: var(--bg-soft);
  overflow: hidden;
  height: 100%;
  width: 100%;
}

.panel-header {
  padding: 0.5rem 1rem;
  background-color: var(--bg-muted);
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-weight: 600;
  color: var(--fg-muted);
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.tree-container {
  padding: 0.5rem 0;
  flex: 1;
  overflow-y: auto;
}

.tree-group-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.25rem 1rem;
  cursor: pointer;
  font-size: 0.9rem;
}

.tree-group-header:hover {
  background-color: var(--hover-bg);
}

.chevron {
  font-size: 0.7rem;
  width: 12px;
  color: var(--fg-muted);
}

.icon {
  font-size: 1rem;
  flex-shrink: 0;
}

.file-name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.tree-items {
  display: flex;
  flex-direction: column;
}

.tree-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.25rem 1rem 0.25rem 2.25rem;
  cursor: pointer;
  font-size: 0.9rem;
  color: var(--fg-muted);
  min-width: 0;
}

.tree-item:hover {
  background-color: var(--hover-bg);
  color: var(--fg-color);
}

.tree-item.selected {
  background-color: var(--primary-muted);
  color: var(--primary-color);
  border-right: 2px solid var(--primary-color);
}

.tree-item.has-error .file-name {
  color: var(--error-color, #da702c);
}
</style>
