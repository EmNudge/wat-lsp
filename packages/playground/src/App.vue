<script setup lang="ts">
import { ref } from 'vue';
import Sidebar from './components/Sidebar.vue';
import SplitEditorLayout from './components/SplitEditorLayout.vue';
import OutputPanel from './components/OutputPanel.vue';
import { usePlaygroundStore } from './store/playground';

const store = usePlaygroundStore();
const isSidebarOpen = ref(true);
const sidebarWidth = ref(260);
const panelHeight = ref(300);
const isResizingWidth = ref(false);
const isResizingHeight = ref(false);

function startWidthResize(e: MouseEvent) {
  isResizingWidth.value = true;
  document.addEventListener('mousemove', handleWidthResize);
  document.addEventListener('mouseup', stopWidthResize);
  e.preventDefault();
}

function handleWidthResize(e: MouseEvent) {
  if (!isResizingWidth.value) return;
  const newWidth = Math.min(Math.max(e.clientX, 150), 400);
  sidebarWidth.value = newWidth;
}

function stopWidthResize() {
  isResizingWidth.value = false;
  document.removeEventListener('mousemove', handleWidthResize);
  document.removeEventListener('mouseup', stopWidthResize);
}

function startHeightResize(e: MouseEvent) {
  isResizingHeight.value = true;
  document.addEventListener('mousemove', handleHeightResize);
  document.addEventListener('mouseup', stopHeightResize);
  e.preventDefault();
}

function handleHeightResize(e: MouseEvent) {
  if (!isResizingHeight.value) return;
  const newHeight = Math.min(Math.max(window.innerHeight - e.clientY - 80, 100), 600);
  panelHeight.value = newHeight;
}

function stopHeightResize() {
  isResizingHeight.value = false;
  document.removeEventListener('mousemove', handleHeightResize);
  document.removeEventListener('mouseup', stopHeightResize);
}
</script>

<template>
  <div class="app-container">
    <header class="header">
      <div class="logo">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 612 612">
          <path d="m376 0c0 1.08 0 2.16 0 3.3 0 38.76-31.42 70.17-70.17 70.17-38.76 0-70.17-31.42-70.17-70.17l0 0c0-1.14 0-2.22 0-3.3L0 0l0 612 612 0 0-612z" fill="#654ff0"/>
          <path d="m142.16 329.81 40.56 0 27.69 147.47 0.5 0 33.28-147.47 37.94 0 30.06 149.28 0.59 0 31.56-149.28 39.78 0-51.69 216.69-40.25 0-29.81-147.47-0.78 0-31.91 147.47-41 0zm287.69 0 63.94 0 63.5 216.69-41.84 0-13.81-48.22-72.84 0-10.66 48.22-40.75 0zm24.34 53.41-17.69 79.5 55.06 0-20.31-79.5z" fill="#fff"/>
        </svg>
        <div class="title-group">
          <h1>WAT Playground</h1>
          <p>WebAssembly Text Format editor with LSP features</p>
        </div>
      </div>
      <div class="header-actions">
        <a href="/" class="nav-link">Documentation</a>
      </div>
    </header>

    <div class="toolbar">
      <!-- Future breadcrumbs or quick actions -->
    </div>

    <main class="main-layout" :style="{ gridTemplateColumns: `${sidebarWidth}px 1fr` }">
      <div v-if="isSidebarOpen" class="sidebar-section">
        <Sidebar />
        <div class="resize-handle horizontal" @mousedown="startWidthResize"></div>
      </div>
      <div class="content-section">
        <div class="editor-section">
          <SplitEditorLayout />
          <div class="resize-handle vertical" @mousedown="startHeightResize"></div>
        </div>
        <div class="bottom-panel-section" :style="{ height: `${panelHeight}px` }">
          <OutputPanel />
        </div>
      </div>
    </main>

    <footer class="footer">
      <div class="status-bar">
        <span>Ready</span>
        <div class="lsp-status">
          <span class="lsp-indicator" :class="{ ready: store.lspReady }"></span>
          <span>{{ store.lspStatus }}</span>
        </div>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--bg-color);
  color: var(--fg-color);
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem 1.5rem;
  background-color: var(--bg-soft);
  border-bottom: 1px solid var(--border-color);
}

.logo {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.title-group h1 {
  margin: 0;
  font-size: 1.1rem;
  font-weight: 700;
}

.title-group p {
  margin: 0;
  font-size: 0.8rem;
  color: var(--fg-muted);
}

.nav-link {
  font-size: 0.9rem;
  color: var(--fg-muted);
  text-decoration: none;
  transition: color 0.2s;
}

.nav-link:hover {
  color: var(--primary-color);
}

.main-layout {
  flex: 1;
  display: grid;
  overflow: hidden;
}

.sidebar-section {
  position: relative;
  display: flex;
  background-color: var(--bg-soft);
  min-height: 0;
}

.content-section {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  border-left: 1px solid var(--border-color);
}

.resize-handle {
  position: absolute;
  background-color: transparent;
  z-index: 10;
  transition: background-color 0.2s;
}

.resize-handle.horizontal {
  right: -3px;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
}

.resize-handle.vertical {
  bottom: -2px;
  left: 0;
  right: 0;
  height: 4px;
  cursor: row-resize;
}

.resize-handle:hover, .resize-handle:active {
  background-color: var(--primary-color);
}

.editor-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  position: relative;
}

.bottom-panel-section {
  position: relative;
  background-color: var(--bg-soft);
  border-top: 1px solid var(--border-color);
  min-height: 100px;
}

.footer {
  background-color: var(--bg-soft);
  border-top: 1px solid var(--border-color);
  padding: 0.4rem 1rem;
  font-size: 0.8rem;
}

.status-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.lsp-status {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.lsp-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--fg-muted);
}

.lsp-indicator.ready {
  background-color: var(--success-color);
  box-shadow: 0 0 5px var(--success-color);
}
</style>
