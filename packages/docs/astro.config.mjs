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
      social: [{ icon: 'github', label: 'WAT-lsp', href: 'https://github.com/EmNudge/wat-lsp' }],
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
        { label: 'Intro', autogenerate: { directory: 'intro' } },
        {
          label: 'Guides',
          items: [
            { label: 'Language', autogenerate: { directory: 'language' } },
            { label: 'Control Flow', autogenerate: { directory: 'control' } },
            { label: 'Stack & Memory', autogenerate: { directory: 'stack' } },
            { label: 'Numeric Ops', autogenerate: { directory: 'ops' } },
            { label: 'Extensions', autogenerate: { directory: 'extensions' } },
          ],
        },
        { label: 'Reference', autogenerate: { directory: 'instructions' } },
        { label: 'Playground', link: '/playground' },
      ],
    }),
    react(),
  ],
});
