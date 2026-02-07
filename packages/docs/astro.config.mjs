// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightThemeFlexoki from 'starlight-theme-flexoki';
import fs from 'node:fs';
import react from '@astrojs/react';
const watGrammar = JSON.parse(
  fs.readFileSync(new URL('./src/grammars/wat.tmLanguage.json', import.meta.url), 'utf-8'),
);

// https://astro.build/config
export default defineConfig({
  vite: {
    optimizeDeps: {
      include: ['monaco-editor', 'vscode-textmate', 'vscode-oniguruma'],
      exclude: ['@emnudge/wat-lsp'],
    },
    resolve: {
      dedupe: ['web-tree-sitter'],
    },
    ssr: {
      noExternal: ['monaco-editor', 'vscode-textmate', 'vscode-oniguruma'],
    },
    server: {
      headers: {
        'Cross-Origin-Opener-Policy': 'same-origin',
        'Cross-Origin-Embedder-Policy': 'require-corp',
      },
    },
  },
  integrations: [
    starlight({
      plugins: [starlightThemeFlexoki()],
      title: 'WAT Docs',
      customCss: ['./src/styles/custom.css'],
      social: [{ icon: 'github', label: 'Watlings', href: 'https://github.com/EmNudge/watlings' }],
      components: {
        MarkdownContent: './src/components/CustomMarkdownContent.astro',
        SiteTitle: './src/components/SiteTitle.astro',
        Sidebar: './src/components/Sidebar.astro',
      },
      expressiveCode: {
        shiki: {
          langs: [
            { ...watGrammar, name: 'wat', aliases: ['wast'] },
            { ...watGrammar, name: 'wat-snippet', aliases: [] },
          ],
        },
      },
      sidebar: [
        {
          label: 'Intro',
          items: [
            { label: 'What is WAT?', slug: 'intro/what-is-wat' },
            { label: 'Hello World', slug: 'intro/hello-world' },
          ],
        },
        {
          label: 'Guides',
          items: [
            {
              label: 'Language',
              collapsed: true,
              items: [
                { label: 'Types', slug: 'types' },
                { label: 'Reference Types', slug: 'reference-types' },
                { label: 'Operators', slug: 'operators' },
                { label: 'Interfacing', slug: 'interfacing' },
                { label: 'Text Format', slug: 'text-format' },
                { label: 'Subtyping', slug: 'subtyping' },
                { label: 'Constant Expressions', slug: 'constant-expressions' },
                { label: 'Execution Model', slug: 'execution-model' },
              ],
            },
            {
              label: 'Control Flow',
              collapsed: true,
              items: [
                { label: 'block', slug: 'control/block' },
                { label: 'loop', slug: 'control/loop' },
                { label: 'if / else', slug: 'control/if' },
                { label: 'br & br_if', slug: 'control/branching' },
                { label: 'br_table', slug: 'control/br-table' },
                { label: 'return', slug: 'control/return' },
                { label: 'call & call_indirect', slug: 'control/calls' },
                { label: 'unreachable & nop', slug: 'control/unreachable-nop' },
                { label: 'select', slug: 'control/select' },
              ],
            },
            {
              label: 'Stack & Memory',
              collapsed: true,
              items: [
                { label: 'Parametric', slug: 'stack/parametric' },
                { label: 'drop', slug: 'stack/drop' },
                { label: 'local.tee', slug: 'stack/tee' },
                { label: 'Memory & Data', slug: 'stack/memory-data' },
                { label: 'Traps', slug: 'stack/traps' },
              ],
            },
            {
              label: 'Numeric Ops',
              collapsed: true,
              items: [
                { label: 'Integer Arithmetic', slug: 'ops/integer-arithmetic' },
                { label: 'Integer Comparisons', slug: 'ops/integer-comparisons' },
                { label: 'Float Arithmetic', slug: 'ops/float-arithmetic' },
                { label: 'Float Comparisons', slug: 'ops/float-comparisons' },
                { label: 'Bitwise & Shifts', slug: 'ops/bitwise-shifts' },
                { label: 'Conversions', slug: 'ops/conversions' },
                { label: 'Float Unary Ops', slug: 'ops/float-unary' },
                { label: 'Constants', slug: 'ops/constants' },
              ],
            },
            {
              label: 'Extensions',
              collapsed: true,
              items: [
                { label: 'JS BigInt ↔ i64', slug: 'extensions/js-bigint-i64' },
                { label: 'Bulk Memory Ops', slug: 'extensions/bulk-memory' },
                { label: 'Extended Const Expressions', slug: 'extensions/extended-const' },
                { label: 'Garbage Collection (WasmGC)', slug: 'extensions/wasm-gc' },
                { label: 'Multi-value', slug: 'extensions/multi-value' },
                { label: 'Import/Export Mutable Globals', slug: 'extensions/mutable-globals' },
                { label: 'Memory64', slug: 'extensions/memory64' },
                { label: 'Reference Types', slug: 'extensions/reference-types' },
                { label: 'Non-trapping Float→Int', slug: 'extensions/nontrapping-f2i' },
                { label: 'Sign-extension Ops', slug: 'extensions/sign-extension' },
                { label: 'SIMD (128-bit)', slug: 'extensions/simd' },
                { label: 'Tail Calls', slug: 'extensions/tail-call' },
                { label: 'Typed Function Refs', slug: 'extensions/typed-func-refs' },
                { label: 'Threads & Atomics', slug: 'extensions/threads' },
                { label: 'Exception Handling', slug: 'extensions/exception-handling' },
              ],
            },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'i32 Instructions', slug: 'instructions/i32' },
            { label: 'i64 Instructions', slug: 'instructions/i64' },
            { label: 'f32 Instructions', slug: 'instructions/f32' },
            { label: 'f64 Instructions', slug: 'instructions/f64' },
            { label: 'Local & Global', slug: 'instructions/local-global' },
            { label: 'Control Flow', slug: 'instructions/control' },
            { label: 'Parametric', slug: 'instructions/parametric' },
            { label: 'Memory', slug: 'instructions/memory' },
            { label: 'Table', slug: 'instructions/table' },
            { label: 'Reference', slug: 'instructions/reference' },
            { label: 'Module Structure', slug: 'instructions/module' },
            { label: 'Type Names', slug: 'instructions/types' },
            { label: 'GC Types', slug: 'instructions/gc-types' },
            { label: 'GC Struct', slug: 'instructions/gc-struct' },
            { label: 'GC Array', slug: 'instructions/gc-array' },
            { label: 'GC i31', slug: 'instructions/gc-i31' },
            { label: 'GC Casts', slug: 'instructions/gc-casts' },
            { label: 'Exceptions', slug: 'instructions/exceptions' },
            { label: 'SIMD (v128)', slug: 'instructions/simd' },
            { label: 'Atomics', slug: 'instructions/atomic' },
          ],
        },
      ],
    }),
    react(),
  ],
});
