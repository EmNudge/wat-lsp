<script setup lang="ts">
import { usePlaygroundStore } from '../store/playground';

const store = usePlaygroundStore();

function handleClose(e: MouseEvent, id: string) {
  e.stopPropagation();
  store.closeFile(id);
}

function resetWat() {
  store.resetWatCode();
}
</script>

<template>
  <div class="editor-tabs">
    <div
      v-for="file in store.openFiles"
      :key="file.id"
      class="tab"
      :class="{ active: file.id === store.activeFileId, dirty: file.isDirty }"
      @click="store.activeFileId = file.id"
    >
      <span class="tab-label">{{ file.filename }}</span>
      <button class="close-btn" @click="handleClose($event, file.id)">
        <svg v-if="!file.isDirty" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
        <span v-else class="dirty-indicator">●</span>
      </button>
    </div>
    <div class="tabs-spacer"></div>
    <button
      v-if="store.currentFile?.isDirty"
      class="btn-reset"
      @click="resetWat"
      title="Reset to original example code"
    >
      Reset
    </button>
  </div>
</template>

<style scoped>
.editor-tabs {
  display: flex;
  align-items: center;
  background-color: var(--bg-soft);
  border-bottom: 1px solid var(--border-color);
  overflow-x: auto;
  scrollbar-width: none;
}

.editor-tabs::-webkit-scrollbar {
  display: none;
}

.tab {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  font-size: 0.8rem;
  color: var(--fg-muted);
  cursor: pointer;
  border-right: 1px solid var(--border-color);
  background-color: var(--bg-soft);
  min-width: 120px;
  max-width: 200px;
  user-select: none;
  position: relative;
}

.tab:hover {
  background-color: var(--hover-bg);
  color: var(--fg-color);
}

.tab.active {
  background-color: var(--bg-color);
  color: var(--fg-color);
}

.tab.active::after {
  content: '';
  position: absolute;
  bottom: -1px;
  left: 0;
  right: 0;
  height: 2px;
  background-color: var(--primary-color);
}

.tab-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.tabs-spacer {
  flex: 1;
}

.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border: none;
  background: transparent;
  color: inherit;
  border-radius: 2px;
  padding: 0;
  cursor: pointer;
  opacity: 0.6;
}

.close-btn:hover {
  background-color: var(--hover-bg);
  opacity: 1;
}

.dirty-indicator {
  font-size: 8px;
  color: var(--warning-color);
}

.btn-reset {
  padding: 0.15rem 0.5rem;
  margin-right: 0.5rem;
  font-size: 0.75rem;
  background: none;
  border: 1px solid var(--border-color);
  color: var(--fg-muted);
  border-radius: 3px;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}

.btn-reset:hover {
  background-color: var(--bg-soft);
  color: var(--fg-color);
  border-color: var(--fg-muted);
}
</style>
