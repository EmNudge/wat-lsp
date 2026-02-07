/**
 * Tests all WAT module examples from documentation against the WAT LSP.
 *
 * Scans every markdown file in src/content/docs/ for ```wat code blocks
 * that contain `(module`, runs each through the LSP, and reports diagnostics.
 */

import { readFileSync } from 'node:fs';
import { join, dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { glob } from 'node:fs/promises';
import { Parser } from 'web-tree-sitter';
import { describe, it, expect, beforeAll } from 'vitest';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const DOCS_DIR = join(ROOT, 'src', 'content', 'docs');

/**
 * Known LSP false positives — valid WAT code that the LSP incorrectly flags
 * due to missing support for proposals or type-checking bugs.
 */
const KNOWN_LSP_ISSUES = new Set([]);

/**
 * Extract all ```wat code blocks from a markdown string.
 * Returns an array of { code, lineNumber } where lineNumber is 1-indexed.
 */
function extractWatBlocks(markdown) {
  const blocks = [];
  const lines = markdown.split('\n');

  let inBlock = false;
  let blockLines = [];
  let blockStart = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (!inBlock && /^```wat\s*$/.test(line)) {
      inBlock = true;
      blockLines = [];
      blockStart = i + 1;
    } else if (inBlock && line.startsWith('```')) {
      inBlock = false;
      blocks.push({ code: blockLines.join('\n'), lineNumber: blockStart + 1 });
    } else if (inBlock) {
      blockLines.push(line);
    }
  }

  return blocks;
}

function formatDiagnostics(diagnostics) {
  return diagnostics
    .map((d) => {
      const severity = d.severity === 1 ? 'error' : d.severity === 2 ? 'warning' : 'info';
      return `  ${severity} (${d.range.start.line + 1}:${d.range.start.character + 1}): ${d.message}`;
    })
    .join('\n');
}

// Collect all test cases before describing them
async function collectExamples() {
  const examples = [];
  const mdFiles = [];

  for await (const entry of glob('**/*.{md,mdx}', { cwd: DOCS_DIR })) {
    mdFiles.push(entry);
  }
  mdFiles.sort();

  for (const relPath of mdFiles) {
    const absPath = join(DOCS_DIR, relPath);
    const markdown = readFileSync(absPath, 'utf8');
    const blocks = extractWatBlocks(markdown);
    const moduleBlocks = blocks.filter((b) => b.code.includes('(module'));

    for (const block of moduleBlocks) {
      const displayPath = relative(join(ROOT, 'src', 'content', 'docs'), absPath);
      const key = `${displayPath}:${block.lineNumber}`;
      examples.push({
        key,
        code: block.code,
        knownIssue: KNOWN_LSP_ISSUES.has(key),
      });
    }
  }

  return examples;
}

let lsp;

async function initLSP() {
  const distWasmDir = join(ROOT, 'node_modules', '@emnudge', 'wat-lsp', 'dist', 'wasm');
  const treeSitterWasmPath = join(distWasmDir, 'tree-sitter.wasm');
  const watLspWasmPath = join(distWasmDir, 'wat_lsp_rust_bg.wasm');

  await Parser.init({
    locateFile: (file) => {
      if (file === 'tree-sitter.wasm') {
        return `file://${treeSitterWasmPath}`;
      }
      return file;
    },
  });

  const { default: initWasm, WatLSP } = await import('@emnudge/wat-lsp/wasm');
  const wasmBuffer = readFileSync(watLspWasmPath);
  await initWasm(wasmBuffer);

  const instance = new WatLSP();
  const success = await instance.initialize();
  if (!success) {
    throw new Error('Failed to initialize WAT LSP');
  }
  return instance;
}

const examples = await collectExamples();

describe('WAT documentation examples', () => {
  beforeAll(async () => {
    lsp = await initLSP();
  });

  for (const example of examples) {
    const testFn = example.knownIssue ? it.skip : it;
    testFn(`${example.key} has no LSP errors`, () => {
      lsp.parse(example.code);
      const diagnostics = Array.from(lsp.provideDiagnostics());
      const errors = diagnostics.filter((d) => d.severity === 1);

      expect(errors, `LSP errors in ${example.key}:\n${formatDiagnostics(errors)}`).toHaveLength(0);
    });
  }
});
