<script setup lang="ts">
import { ref } from 'vue';
import Editor from './Editor.vue';
import JsEditor from './JsEditor.vue';

const splitPercent = ref(60);
const isResizing = ref(false);
const container = ref<HTMLElement | null>(null);

function startResize(e: MouseEvent) {
  isResizing.value = true;
  document.addEventListener('mousemove', handleResize);
  document.addEventListener('mouseup', stopResize);
  e.preventDefault();
}

function handleResize(e: MouseEvent) {
  if (!isResizing.value || !container.value) return;
  const rect = container.value.getBoundingClientRect();
  const pct = ((e.clientX - rect.left) / rect.width) * 100;
  splitPercent.value = Math.min(Math.max(pct, 25), 75);
}

function stopResize() {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleResize);
  document.removeEventListener('mouseup', stopResize);
}
</script>

<template>
  <div
    class="split-editor-layout"
    ref="container"
    :class="{ resizing: isResizing }"
  >
    <div class="split-pane left" :style="{ width: `${splitPercent}%` }">
      <Editor />
    </div>
    <div class="split-divider" @mousedown="startResize"></div>
    <div class="split-pane right" :style="{ width: `${100 - splitPercent}%` }">
      <JsEditor />
    </div>
  </div>
</template>

<style scoped>
.split-editor-layout {
  display: flex;
  width: 100%;
  height: 100%;
  min-height: 0;
}

.split-editor-layout.resizing {
  user-select: none;
  cursor: col-resize;
}

.split-pane {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.split-pane.left {
  display: flex;
  flex-direction: column;
}

.split-pane.right {
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--border-color);
}

.split-divider {
  width: 4px;
  cursor: col-resize;
  background-color: transparent;
  transition: background-color 0.2s;
  flex-shrink: 0;
  z-index: 10;
}

.split-divider:hover,
.split-editor-layout.resizing .split-divider {
  background-color: var(--primary-color);
}
</style>
