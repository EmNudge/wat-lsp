import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    include: ['monaco-editor', 'wabt', 'web-tree-sitter'],
    exclude: ['wat_lsp_rust']
  },
  build: {
    target: 'esnext'
  },
  server: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp'
    }
  }
});
