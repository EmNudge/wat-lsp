import { useEffect, useRef } from 'react';
import { EditorState } from '@codemirror/state';
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
  hoverTooltip,
} from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { syntaxHighlighting, HighlightStyle, StreamLanguage } from '@codemirror/language';
import { linter } from '@codemirror/lint';
import { tags } from '@lezer/highlight';
import { oneDark } from '@codemirror/theme-one-dark';
import { createWatLSP } from '@emnudge/wat-lsp';
import treeSitterWasmUrl from '@emnudge/wat-lsp/wasm/tree-sitter.wasm?url';
import watLspWasmUrl from '@emnudge/wat-lsp/wasm/wat_lsp_rust_bg.wasm?url';

// WAT language definition for CodeMirror StreamLanguage
const watLanguage = StreamLanguage.define({
  name: 'wat',
  startState: () => ({ inBlockComment: false }),
  token(stream, state) {
    // Block comment
    if (state.inBlockComment) {
      if (stream.match(/.*?;\)/)) {
        state.inBlockComment = false;
      } else {
        stream.skipToEnd();
      }
      return 'comment';
    }

    // Start of block comment
    if (stream.match(/\(;/)) {
      state.inBlockComment = true;
      return 'comment';
    }

    // Line comment
    if (stream.match(/;;.*/)) {
      return 'comment';
    }

    // String
    if (stream.match(/"([^"\\]|\\.)*"/)) {
      return 'string';
    }

    // Numbers (hex and decimal)
    if (stream.match(/[+-]?0x[0-9a-fA-F_]+(\.[0-9a-fA-F_]*)?([pP][+-]?[0-9_]+)?/)) {
      return 'number';
    }
    if (stream.match(/[+-]?\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d[\d_]*)?/)) {
      return 'number';
    }

    // Identifiers starting with $
    if (stream.match(/\$[a-zA-Z0-9_!#$%&'*+\-./:<=>?@\\^`|~]+/)) {
      return 'variableName';
    }

    // Instructions with dot notation (i32.add, local.get, etc.)
    if (
      stream.match(/(i32|i64|f32|f64|v128|i8x16|i16x8|i32x4|i64x2|f32x4|f64x2)\.[a-z_][a-z0-9_]*/)
    ) {
      return 'keyword';
    }
    if (stream.match(/(local|global|memory|table|ref|struct|array|i31)\.[a-z_][a-z0-9_]*/)) {
      return 'keyword';
    }

    // Type keywords
    if (
      stream.match(
        /\b(i32|i64|f32|f64|v128|funcref|externref|anyref|eqref|i31ref|structref|arrayref)\b/,
      )
    ) {
      return 'typeName';
    }

    // Keywords
    if (
      stream.match(
        /\b(module|func|param|result|local|global|table|memory|type|import|export|start|elem|data|offset|declare|item|field|mut|block|loop|if|then|else|end|br|br_if|br_table|return|call|call_indirect|call_ref|drop|select|unreachable|nop|ref|null|struct|array|rec|sub|final|tag|try|catch|throw)\b/,
      )
    ) {
      return 'keyword';
    }

    // Parentheses
    if (stream.match(/[()]/)) {
      return 'bracket';
    }

    // Skip whitespace
    if (stream.eatSpace()) {
      return null;
    }

    // Any other word
    if (stream.match(/[a-zA-Z_][a-zA-Z0-9_]*/)) {
      return 'name';
    }

    // Skip unknown character
    stream.next();
    return null;
  },
});

// Custom highlighting style matching the site theme
const watHighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: '#569cd6' },
  { tag: tags.typeName, color: '#4ec9b0' },
  { tag: tags.variableName, color: '#9cdcfe' },
  { tag: tags.string, color: '#ce9178' },
  { tag: tags.number, color: '#b5cea8' },
  { tag: tags.comment, color: '#6a9955' },
  { tag: tags.bracket, color: '#d4d4d4' },
  { tag: tags.name, color: '#dcdcaa' },
]);

// Custom theme adjustments - reset inherited styles from Starlight
const customTheme = EditorView.theme({
  '&': {
    fontSize: '14px',
    lineHeight: '1.5',
    backgroundColor: '#1e1e1e',
    height: '100%',
  },
  // Override Starlight's .sl-markdown-content spacing rule
  '& *': {
    marginTop: '0 !important',
  },
  '.cm-scroller': {
    overflow: 'auto',
    fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
    lineHeight: '1.5 !important',
    margin: '0 !important',
    padding: '8px 0 !important',
  },
  '.cm-content': {
    padding: '0',
    margin: '0',
    caretColor: '#fff',
    lineHeight: '1.5 !important',
  },
  '.cm-line': {
    padding: '0 4px',
    margin: '0',
    lineHeight: '1.5 !important',
  },
  '.cm-gutters': {
    backgroundColor: '#1e1e1e',
    borderRight: '1px solid #333',
    margin: '0',
  },
  '.cm-gutter': {
    margin: '0',
    lineHeight: '1.5 !important',
  },
  '.cm-activeLineGutter': {
    backgroundColor: '#2a2a2a',
  },
  '.cm-activeLine': {
    backgroundColor: '#2a2a2a40',
  },
  '&.cm-focused .cm-cursor': {
    borderLeftColor: '#fff',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
    backgroundColor: '#264f78',
  },
  // Tooltip styles
  '.cm-tooltip.cm-lsp-tooltip': {
    backgroundColor: '#252526',
    border: '1px solid #454545',
    borderRadius: '4px',
    padding: '8px 12px',
    maxWidth: '600px',
    maxHeight: '400px',
    overflow: 'auto',
    fontSize: '13px',
    lineHeight: '1.5',
    color: '#d4d4d4',
    boxShadow: '0 2px 8px rgba(0,0,0,0.4)',
  },
  '.cm-tooltip.cm-lsp-tooltip code': {
    backgroundColor: '#1e1e1e',
    padding: '2px 4px',
    borderRadius: '3px',
    fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
    fontSize: '12px',
  },
  '.cm-tooltip.cm-lsp-tooltip .cm-tooltip-codeblock': {
    backgroundColor: '#1e1e1e',
    padding: '8px 12px',
    borderRadius: '4px',
    margin: '8px 0',
    overflow: 'auto',
  },
  '.cm-tooltip.cm-lsp-tooltip .cm-tooltip-codeblock code': {
    backgroundColor: 'transparent',
    padding: '0',
    display: 'block',
    whiteSpace: 'pre',
    fontSize: '12px',
    lineHeight: '1.4',
  },
  // Diagnostic styles
  '.cm-diagnostic': {
    padding: '4px 8px',
    marginLeft: '0',
  },
  '.cm-diagnostic-error': {
    borderLeft: '3px solid #f44747',
  },
  '.cm-diagnostic-warning': {
    borderLeft: '3px solid #cca700',
  },
  '.cm-lintRange-error': {
    backgroundImage: 'none',
    textDecoration: 'wavy underline #f44747',
    textUnderlineOffset: '3px',
  },
  '.cm-lintRange-warning': {
    backgroundImage: 'none',
    textDecoration: 'wavy underline #cca700',
    textUnderlineOffset: '3px',
  },
});

// Helper to escape HTML
function escapeHtml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// Token colors matching the editor theme
const tokenColors: Record<string, string> = {
  keyword: '#569cd6',
  typeName: '#4ec9b0',
  variableName: '#9cdcfe',
  string: '#ce9178',
  number: '#b5cea8',
  comment: '#6a9955',
  bracket: '#d4d4d4',
  name: '#dcdcaa',
};

// Simple WAT syntax highlighter for tooltips
function highlightWatCode(code: string): string {
  const lines = code.split('\n');
  return lines
    .map((line) => {
      let result = '';
      let i = 0;

      while (i < line.length) {
        // Skip whitespace
        if (/\s/.test(line[i])) {
          result += line[i];
          i++;
          continue;
        }

        // Line comment
        if (line.slice(i, i + 2) === ';;') {
          result += `<span style="color:${tokenColors.comment}">${escapeHtml(line.slice(i))}</span>`;
          break;
        }

        // String
        if (line[i] === '"') {
          let end = i + 1;
          while (end < line.length && (line[end] !== '"' || line[end - 1] === '\\')) end++;
          end++;
          result += `<span style="color:${tokenColors.string}">${escapeHtml(line.slice(i, end))}</span>`;
          i = end;
          continue;
        }

        // Parentheses
        if (line[i] === '(' || line[i] === ')') {
          result += `<span style="color:${tokenColors.bracket}">${line[i]}</span>`;
          i++;
          continue;
        }

        // Identifier starting with $
        if (line[i] === '$') {
          let end = i + 1;
          while (end < line.length && /[a-zA-Z0-9_]/.test(line[end])) end++;
          result += `<span style="color:${tokenColors.variableName}">${escapeHtml(line.slice(i, end))}</span>`;
          i = end;
          continue;
        }

        // Words (keywords, types, instructions)
        if (/[a-zA-Z_]/.test(line[i])) {
          let end = i;
          while (end < line.length && /[a-zA-Z0-9_.]/.test(line[end])) end++;
          const word = line.slice(i, end);

          // Check token type
          let color = tokenColors.name;
          if (/^(i32|i64|f32|f64|v128|funcref|externref|anyref)$/.test(word)) {
            color = tokenColors.typeName;
          } else if (
            /^(module|func|param|result|local|global|table|memory|type|import|export|block|loop|if|then|else|end|br|br_if|return|call|drop|select|unreachable|nop)$/.test(
              word,
            )
          ) {
            color = tokenColors.keyword;
          } else if (/\.(get|set|const|add|sub|mul|div|load|store|eq|ne|lt|gt|le|ge)/.test(word)) {
            color = tokenColors.keyword;
          }

          result += `<span style="color:${color}">${escapeHtml(word)}</span>`;
          i = end;
          continue;
        }

        // Numbers
        if (/[0-9]/.test(line[i]) || (line[i] === '-' && /[0-9]/.test(line[i + 1]))) {
          let end = i;
          if (line[end] === '-') end++;
          while (end < line.length && /[0-9a-fA-Fx_.]/.test(line[end])) end++;
          result += `<span style="color:${tokenColors.number}">${escapeHtml(line.slice(i, end))}</span>`;
          i = end;
          continue;
        }

        // Default
        result += escapeHtml(line[i]);
        i++;
      }

      return result;
    })
    .join('\n');
}

// Global LSP instance
let watLSP: any = null;
let lspInitPromise: Promise<any> | null = null;

async function initLSP() {
  if (watLSP) return watLSP;
  if (lspInitPromise) return lspInitPromise;

  lspInitPromise = (async () => {
    try {
      watLSP = await createWatLSP({
        treeSitterWasmPath: treeSitterWasmUrl,
        watLspWasmPath: watLspWasmUrl,
      });
      console.log('WAT LSP initialized');
      return watLSP;
    } catch (err) {
      console.error('Failed to initialize WAT LSP:', err);
      return null;
    }
  })();

  return lspInitPromise;
}

// Create hover tooltip using LSP
function createLSPHover() {
  return hoverTooltip(async (view, pos, _side) => {
    const lsp = await initLSP();
    if (!lsp?.ready) return null;

    const doc = view.state.doc;
    const lineInfo = doc.lineAt(pos);
    const line = lineInfo.number - 1; // LSP uses 0-indexed lines
    const col = pos - lineInfo.from; // Column within line

    const content = doc.toString();
    lsp.parse(content);

    const hover = lsp.provideHover(line, col);
    if (!hover?.contents?.value) return null;

    return {
      pos,
      above: true,
      create() {
        const dom = document.createElement('div');
        dom.className = 'cm-tooltip cm-lsp-tooltip';

        // Parse markdown content
        let text = hover.contents.value;

        // Convert code blocks ```lang ... ``` to <pre><code> with syntax highlighting
        text = text.replace(/```(\w*)\n([\s\S]*?)```/g, (_: string, lang: string, code: string) => {
          const trimmed = code.trim();
          const highlighted =
            lang === 'wat' || lang === 'wast' || lang === ''
              ? highlightWatCode(trimmed)
              : escapeHtml(trimmed);
          return `<pre class="cm-tooltip-codeblock"><code>${highlighted}</code></pre>`;
        });

        // Convert inline code `...` to <code>
        text = text.replace(/`([^`]+)`/g, '<code>$1</code>');

        // Convert newlines to <br> (but not inside pre blocks)
        const parts = text.split(/(<pre[\s\S]*?<\/pre>)/);
        text = parts
          .map((part: string, _i: number) => {
            if (part.startsWith('<pre')) return part;
            return part.replace(/\n/g, '<br>');
          })
          .join('');

        dom.innerHTML = text;
        return { dom };
      },
    };
  });
}

// Create linter using LSP diagnostics
function createLSPLinter() {
  return linter(
    async (view) => {
      const lsp = await initLSP();
      if (!lsp?.ready) return [];

      const content = view.state.doc.toString();
      lsp.parse(content);

      const diagnostics = lsp.provideDiagnostics();
      if (!diagnostics?.length) return [];

      return diagnostics.map((diag: any) => {
        const startLine = view.state.doc.line(diag.range.start.line + 1);
        const endLine = view.state.doc.line(diag.range.end.line + 1);

        const from = startLine.from + diag.range.start.character;
        const to = endLine.from + diag.range.end.character;

        return {
          from: Math.min(from, view.state.doc.length),
          to: Math.min(to, view.state.doc.length),
          severity: diag.severity === 1 ? 'error' : diag.severity === 2 ? 'warning' : 'info',
          message: diag.message,
        };
      });
    },
    {
      delay: 300,
    },
  );
}

// Check if current page should have diagnostics (not instruction reference pages)
function shouldShowDiagnostics(): boolean {
  const path = window.location.pathname;
  // Disable diagnostics on instruction reference pages (incomplete snippets)
  return !path.includes('/instructions/');
}

async function enhanceCodeBlock(pre: HTMLElement) {
  const figure = pre.closest('figure');
  const target = figure || pre;

  if (target.dataset.cmEnhanced) return;
  target.dataset.cmEnhanced = 'true';

  // Extract code from this specific pre's ec-line divs
  const lines = pre.querySelectorAll('.ec-line');
  let code = '';

  if (lines.length > 0) {
    code = Array.from(lines)
      .map((line) => (line.textContent || '').replace(/^\n+|\n+$/g, ''))
      .join('\n');
  } else {
    const codeEl = pre.querySelector('code');
    code = codeEl?.textContent || '';
  }

  if (!code.trim()) return;

  // Create container - use max-height to let CodeMirror size itself properly
  const container = document.createElement('div');
  container.className = 'wat-editor-container';
  container.style.cssText = `
    max-height: 400px;
    min-height: 120px;
    border-radius: 8px;
    overflow: auto;
    margin-block: 1rem;
  `;

  // Replace only the figure, not the entire .expressive-code wrapper
  // (the wrapper may contain sibling figures for other languages)
  target.replaceWith(container);

  // Build extensions list
  const extensions = [
    lineNumbers(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    history(),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    watLanguage,
    syntaxHighlighting(watHighlightStyle),
    customTheme,
    oneDark,
    EditorView.lineWrapping,
    EditorView.editable.of(true),
    EditorView.contentAttributes.of({ tabindex: '0' }),
    createLSPHover(),
  ];

  // Only add linter on non-instruction pages
  if (shouldShowDiagnostics()) {
    extensions.push(createLSPLinter());
  }

  // Create CodeMirror editor with LSP extensions
  const state = EditorState.create({
    doc: code,
    extensions,
  });

  new EditorView({
    state,
    parent: container,
  });
}

async function enhanceAllWatBlocks() {
  // Start LSP initialization early
  initLSP();

  const pres = document.querySelectorAll('pre[data-language="wat"], pre[data-language="wast"]');

  for (const pre of pres) {
    await enhanceCodeBlock(pre as HTMLElement);
  }
}

export default function CodeMirrorEnhancer() {
  const hasRun = useRef(false);

  useEffect(() => {
    if (hasRun.current) return;
    hasRun.current = true;

    enhanceAllWatBlocks();

    const handlePageLoad = () => enhanceAllWatBlocks();
    document.addEventListener('astro:page-load', handlePageLoad);

    return () => {
      document.removeEventListener('astro:page-load', handlePageLoad);
    };
  }, []);

  return null;
}
