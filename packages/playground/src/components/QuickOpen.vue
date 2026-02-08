<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { usePlaygroundStore } from '../store/playground';
import { examples } from '../../examples';

const store = usePlaygroundStore();
const search = ref('');
const selectedIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);

const filteredExamples = computed(() => {
  const query = search.value.toLowerCase();
  if (!query) return examples.slice(0, 10);
  return examples.filter(ex => 
    ex.filename.toLowerCase().includes(query) || 
    ex.id.toLowerCase().includes(query)
  ).slice(0, 10);
});

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    store.isQuickOpenVisible = false;
  } else if (e.key === 'ArrowDown') {
    selectedIndex.value = (selectedIndex.value + 1) % filteredExamples.value.length;
    e.preventDefault();
  } else if (e.key === 'ArrowUp') {
    selectedIndex.value = (selectedIndex.value - 1 + filteredExamples.value.length) % filteredExamples.value.length;
    e.preventDefault();
  } else if (e.key === 'Enter') {
    const selected = filteredExamples.value[selectedIndex.value];
    if (selected) {
      selectExample(selected.id);
    }
  }
}

function selectExample(id: string) {
  store.openFile(id);
  store.isQuickOpenVisible = false;
}

onMounted(() => {
  inputRef.value?.focus();
  window.addEventListener('keydown', handleGlobalKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
});

function handleGlobalKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && store.isQuickOpenVisible) {
    store.isQuickOpenVisible = false;
  }
}

watch(filteredExamples, () => {
  selectedIndex.value = 0;
});
</script>

<template>
  <div class="quick-open-overlay" @click.self="store.isQuickOpenVisible = false">
    <div class="quick-open-modal">
      <div class="search-input-container">
        <input 
          ref="inputRef"
          v-model="search" 
          type="text" 
          placeholder="Type the name of a file to open..."
          @keydown="handleKeydown"
        />
      </div>
      <div class="results-list">
        <div 
          v-for="(ex, index) in filteredExamples" 
          :key="ex.id"
          class="result-item"
          :class="{ selected: index === selectedIndex }"
          @click="selectExample(ex.id)"
          @mouseenter="selectedIndex = index"
        >
          <span class="icon">📄</span>
          <span class="filename">{{ ex.filename }}</span>
          <span class="path" v-if="ex.id.startsWith('invalid_')">invalid/</span>
          <span class="path" v-else>examples/</span>
        </div>
        <div v-if="filteredExamples.length === 0" class="no-results">
          No matching files found.
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.quick-open-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  justify-content: center;
  padding-top: 10vh;
  z-index: 1000;
}

.quick-open-modal {
  width: 600px;
  max-width: 90vw;
  background-color: var(--bg-soft);
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  border: 1px solid var(--border-color);
  overflow: hidden;
  height: fit-content;
}

.search-input-container {
  padding: 0.75rem;
  border-bottom: 1px solid var(--border-color);
}

.search-input-container input {
  width: 100%;
  background: var(--bg-color);
  border: 1px solid var(--border-color);
  color: var(--fg-color);
  padding: 0.5rem 0.75rem;
  font-size: 1rem;
  border-radius: 4px;
  outline: none;
}

.search-input-container input:focus {
  border-color: var(--primary-color);
}

.results-list {
  max-height: 400px;
  overflow-y: auto;
}

.result-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem 1rem;
  cursor: pointer;
  font-size: 0.9rem;
}

.result-item:hover, .result-item.selected {
  background-color: var(--hover-bg);
}

.result-item.selected {
  background-color: var(--primary-muted);
}

.filename {
  flex: 1;
  color: var(--fg-color);
}

.path {
  color: var(--fg-muted);
  font-size: 0.8rem;
}

.icon {
  font-size: 1rem;
}

.no-results {
  padding: 1rem;
  color: var(--fg-muted);
  text-align: center;
  font-style: italic;
}
</style>
