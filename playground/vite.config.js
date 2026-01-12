import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    include: ['monaco-editor', 'wabt', 'web-tree-sitter'],
    exclude: ['@emnudge/wat-lsp']
  },
  resolve: {
    // Ensure web-tree-sitter is resolved from playground's node_modules
    // even when imported by @emnudge/wat-lsp (which has it as optional peer dep)
    dedupe: ['web-tree-sitter']
  },
  build: {
    target: 'esnext',
    rollupOptions: {
      // Don't externalize web-tree-sitter - bundle it
      external: [],
    }
  },
  server: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp'
    }
  }
});
