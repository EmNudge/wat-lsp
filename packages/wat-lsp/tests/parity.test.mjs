/**
 * Cross-build parity tests — run the same JSON fixtures that the native
 * Rust tests use (tests/e2e_fixtures/) against the WASM build.
 *
 * This catches any divergence between native and WASM behavior.
 */

import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, it, expect, beforeAll } from 'vitest';
import { createLSP } from './helpers.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURES_DIR = join(__dirname, '..', '..', '..', 'tests', 'e2e_fixtures');

let lsp;

beforeAll(async () => {
  lsp = await createLSP();
});

function loadFixtures() {
  const files = readdirSync(FIXTURES_DIR).filter((f) => f.endsWith('.json'));
  return files.map((f) => {
    const content = readFileSync(join(FIXTURES_DIR, f), 'utf-8');
    return { name: f.replace('.json', ''), ...JSON.parse(content) };
  });
}

const fixtures = loadFixtures();

describe('parity fixtures', () => {
  for (const fixture of fixtures) {
    describe(fixture.name, () => {
      for (const [i, test] of fixture.tests.entries()) {
        const label = `[${i}] ${test.feature}`;

        it(label, () => {
          const doc = test.document_override || fixture.document;
          lsp.parse(doc);
          const ex = test.expect;

          switch (test.feature) {
            case 'diagnostics': {
              const diags = Array.from(lsp.provideDiagnostics());
              if (ex.error_count !== undefined) {
                expect(diags).toHaveLength(ex.error_count);
              }
              if (ex.min_error_count !== undefined) {
                expect(diags.length).toBeGreaterThanOrEqual(ex.min_error_count);
              }
              break;
            }

            case 'hover': {
              const [line, col] = test.position;
              const hover = lsp.provideHover(line, col);
              if (ex.non_null) {
                expect(hover).not.toBeNull();
                expect(hover).toBeDefined();
              }
              if (ex.contains) {
                // WASM hover returns { contents: { kind, value } }
                const text = hover.contents?.value ?? hover.contents;
                expect(text).toContain(ex.contains);
              }
              break;
            }

            case 'definition': {
              const [line, col] = test.position;
              const def = lsp.provideDefinition(line, col);
              if (ex.start_line !== undefined) {
                expect(def).not.toBeNull();
                expect(def.range.start.line).toBe(ex.start_line);
              }
              break;
            }

            case 'references': {
              const [line, col] = test.position;
              const inclDecl =
                test.include_declaration !== undefined
                  ? test.include_declaration
                  : true;
              const refs = Array.from(
                lsp.provideReferences(line, col, inclDecl)
              );
              if (ex.min_count !== undefined) {
                expect(refs.length).toBeGreaterThanOrEqual(ex.min_count);
              }
              break;
            }

            case 'document_symbols': {
              const symbols = Array.from(lsp.provideDocumentSymbols());
              if (ex.min_count !== undefined) {
                expect(symbols.length).toBeGreaterThanOrEqual(ex.min_count);
              }
              if (ex.contains_name) {
                const names = symbols.map((s) => s.name);
                expect(names.some((n) => n.includes(ex.contains_name))).toBe(
                  true
                );
              }
              break;
            }

            case 'completion': {
              const [line, col] = test.position;
              const items = Array.from(lsp.provideCompletion(line, col));
              if (ex.non_empty) {
                expect(items.length).toBeGreaterThan(0);
              }
              if (ex.contains_label) {
                const labels = items.map((c) => c.label);
                expect(
                  labels.some((l) => l.includes(ex.contains_label))
                ).toBe(true);
              }
              break;
            }

            case 'folding_ranges': {
              const ranges = Array.from(lsp.provideFoldingRanges());
              if (ex.non_empty) {
                expect(ranges.length).toBeGreaterThan(0);
              }
              break;
            }

            case 'rename': {
              const [line, col] = test.position;
              const result = lsp.rename(line, col, test.new_name);
              const edits = Array.from(result.changes);
              if (ex.min_edit_count !== undefined) {
                expect(edits.length).toBeGreaterThanOrEqual(ex.min_edit_count);
              }
              if (ex.new_text) {
                for (const edit of edits) {
                  expect(edit.newText).toBe(ex.new_text);
                }
              }
              break;
            }

            default:
              throw new Error(`Unknown fixture feature: ${test.feature}`);
          }
        });
      }
    });
  }
});
