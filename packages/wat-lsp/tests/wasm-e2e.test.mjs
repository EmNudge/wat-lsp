/**
 * End-to-end tests for the WASM WatLSP API.
 *
 * These mirror the native LSP protocol tests in tests/lsp_protocol.rs
 * but exercise the WASM build directly via the WatLSP class.
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { createLSP } from './helpers.mjs';

let lsp;

beforeAll(async () => {
  lsp = await createLSP();
});

const TEST_DOC = `(module
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
  (func $main (result i32)
    (call $add (i32.const 1) (i32.const 2))))`;

// ── Diagnostics ──

describe('diagnostics', () => {
  it('produces 0 errors for valid WAT', () => {
    lsp.parse(TEST_DOC);
    const diags = lsp.provideDiagnostics();
    const errors = Array.from(diags).filter((d) => d.severity === 1);
    expect(errors).toHaveLength(0);
  });

  it('produces errors for invalid WAT', () => {
    lsp.parse('(module (func (i32.add)))');
    const diags = Array.from(lsp.provideDiagnostics());
    expect(diags.length).toBeGreaterThan(0);
  });
});

// ── Hover ──

describe('hover', () => {
  it('returns docs for i32.add', () => {
    lsp.parse(TEST_DOC);
    const hover = lsp.provideHover(2, 5);
    expect(hover).not.toBeNull();
    expect(hover).toBeDefined();
    expect(hover.contents.value).toContain('i32.add');
  });
});

// ── Completion ──

describe('completion', () => {
  it('completes after i32.', () => {
    lsp.parse('(module\n  (func\n    (i32.');
    const items = Array.from(lsp.provideCompletion(2, 9));
    expect(items.length).toBeGreaterThan(0);

    const labels = items.map((c) => c.label);
    expect(labels.some((l) => l.includes('add'))).toBe(true);
  });
});

// ── Go-to-definition ──

describe('definition', () => {
  it('resolves $add to its definition', () => {
    lsp.parse(TEST_DOC);
    // $add reference on line 4, col ~11
    const def = lsp.provideDefinition(4, 11);
    expect(def).not.toBeNull();
    expect(def).toBeDefined();
    expect(def.range.start.line).toBe(1);
  });
});

// ── Find references ──

describe('references', () => {
  it('finds definition + call site for $add', () => {
    lsp.parse(TEST_DOC);
    const refs = Array.from(lsp.provideReferences(1, 9, true));
    expect(refs.length).toBeGreaterThanOrEqual(2);
  });
});

// ── Document symbols ──

describe('document symbols', () => {
  it('returns outline with functions', () => {
    lsp.parse(TEST_DOC);
    const symbols = Array.from(lsp.provideDocumentSymbols());
    expect(symbols.length).toBeGreaterThanOrEqual(2);

    const names = symbols.map((s) => s.name);
    expect(names.some((n) => n.includes('add'))).toBe(true);
    expect(names.some((n) => n.includes('main'))).toBe(true);
  });
});

// ── Rename ──

describe('rename', () => {
  it('renames $add to $sum across all sites', () => {
    lsp.parse(TEST_DOC);
    const result = lsp.rename(1, 9, '$sum');
    expect(result).not.toBeNull();
    expect(result).toBeDefined();

    const edits = Array.from(result.changes);
    expect(edits.length).toBeGreaterThanOrEqual(2);

    for (const edit of edits) {
      expect(edit.newText).toBe('$sum');
    }
  });
});

// ── Folding ranges ──

describe('folding ranges', () => {
  it('returns ranges for multi-line doc', () => {
    lsp.parse(TEST_DOC);
    const ranges = Array.from(lsp.provideFoldingRanges());
    expect(ranges.length).toBeGreaterThan(0);
  });
});

// ── Signature help ──

describe('signature help', () => {
  it('does not crash on call context', () => {
    lsp.parse(
      '(module\n  (func $add (param i32) (param i32) (result i32)\n    (i32.add (local.get 0) (local.get 1)))\n  (func (call $add'
    );
    // Just verify it doesn't throw
    const _result = lsp.provideSignatureHelp(3, 17);
  });
});
