/**
 * Diagnostic Parity Tests
 *
 * These tests verify that the WASM LSP implementation produces the same
 * diagnostics as the native implementation for a shared corpus of test files.
 *
 * The corpus lives in tests/diagnostic_corpus/ and is also validated by
 * the native Rust tests (cargo test diagnostic_parity).
 */

import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// ES module equivalent of __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Path to diagnostic corpus (relative to project root)
const CORPUS_DIR = path.join(__dirname, '../../../tests/diagnostic_corpus');

/**
 * Load all test cases from the corpus directory
 */
function loadCorpusTestCases() {
  const testCases = [];

  if (!fs.existsSync(CORPUS_DIR)) {
    console.warn(`Corpus directory not found: ${CORPUS_DIR}`);
    return testCases;
  }

  const files = fs.readdirSync(CORPUS_DIR);

  for (const file of files) {
    if (file.endsWith('.wat')) {
      const baseName = file.replace('.wat', '');
      const expectedFile = `${baseName}.expected.json`;

      if (files.includes(expectedFile)) {
        const watPath = path.join(CORPUS_DIR, file);
        const expectedPath = path.join(CORPUS_DIR, expectedFile);

        const watContent = fs.readFileSync(watPath, 'utf-8');
        const expectedContent = JSON.parse(fs.readFileSync(expectedPath, 'utf-8'));

        testCases.push({
          name: baseName,
          wat: watContent,
          expected: expectedContent,
        });
      }
    }
  }

  return testCases;
}

const testCases = loadCorpusTestCases();

test.describe('Diagnostic Parity (WASM vs Native)', () => {
  // Skip all tests if no corpus found
  test.skip(testCases.length === 0, 'No diagnostic corpus found');

  for (const testCase of testCases) {
    test(`${testCase.name}: ${testCase.expected.description}`, async ({ page }) => {
      // Navigate to playground
      await page.goto('/');

      // Wait for LSP to be ready
      await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, {
        timeout: 15000,
      });

      // Set editor content to our test WAT
      await page.evaluate((wat) => {
        // Access Monaco editor instance
        const editor = window.monacoEditor;
        if (editor) {
          editor.setValue(wat);
        }
      }, testCase.wat);

      // Wait for diagnostics to update (debounced at 300ms + processing time)
      // Use longer timeout and also trigger a reparse
      await page.waitForTimeout(800);

      // Force a reparse by making a small edit and reverting
      await page.evaluate(() => {
        const editor = window.monacoEditor;
        if (editor) {
          const model = editor.getModel();
          const content = model.getValue();
          model.setValue(content + ' ');
          model.setValue(content);
        }
      });

      // Wait for the debounced diagnostics to update
      await page.waitForTimeout(600);

      // Get diagnostics from the LSP via exposed window function or markers
      const diagnostics = await page.evaluate(() => {
        const editor = window.monacoEditor;
        if (!editor) return [];

        const model = editor.getModel();
        if (!model) return [];

        // Get markers (diagnostics) from Monaco
        const markers = window.monaco.editor.getModelMarkers({ resource: model.uri });

        return markers.map((m) => ({
          line: m.startLineNumber - 1, // Convert to 0-indexed like our corpus
          message: m.message,
          severity: m.severity, // Monaco: 8=error, 4=warning, 2=info, 1=hint
        }));
      });

      // Convert Monaco severity to LSP severity (1=error, 2=warning, 3=info, 4=hint)
      const normalizedDiagnostics = diagnostics.map((d) => ({
        ...d,
        severity:
          d.severity === 8
            ? 1
            : d.severity === 4
              ? 2
              : d.severity === 2
                ? 3
                : d.severity === 1
                  ? 4
                  : d.severity,
      }));

      // Verify expected diagnostics
      for (let i = 0; i < testCase.expected.diagnostics.length; i++) {
        const exp = testCase.expected.diagnostics[i];

        const found = normalizedDiagnostics.some((actual) => {
          // Check line
          if (actual.line !== exp.line) return false;

          // Check severity
          if (actual.severity !== exp.severity) return false;

          // Check message contains
          if (exp.message_contains && !actual.message.includes(exp.message_contains)) {
            return false;
          }

          // Check message contains all
          if (exp.message_contains_all) {
            for (const s of exp.message_contains_all) {
              if (!actual.message.includes(s)) return false;
            }
          }

          return true;
        });

        const actualOnLine = normalizedDiagnostics
          .filter((d) => d.line === exp.line)
          .map((d) => `  - ${d.message}`)
          .join('\n');

        expect(found, `Expected diagnostic #${i + 1} not found:
  Line: ${exp.line}
  Message should contain: ${exp.message_contains || '(any)'}
  Message should contain all: ${JSON.stringify(exp.message_contains_all || [])}
  Severity: ${exp.severity}

Actual diagnostics on line ${exp.line}:
${actualOnLine || '  (none)'}

All actual diagnostics:
${normalizedDiagnostics.map((d) => `  L${d.line}: ${d.message}`).join('\n') || '  (none)'}`).toBe(true);
      }

      // For "no errors expected" tests, verify we got none
      if (testCase.expected.diagnostics.length === 0) {
        const errors = normalizedDiagnostics.filter((d) => d.severity === 1);
        expect(
          errors,
          `Expected no errors but found ${errors.length}:
${errors.map((d) => `  L${d.line}: ${d.message}`).join('\n')}`
        ).toHaveLength(0);
      }
    });
  }
});

// Meta-test to ensure corpus is being loaded
test('diagnostic corpus is loaded', () => {
  console.log(`Loaded ${testCases.length} diagnostic parity test cases`);
  expect(testCases.length).toBeGreaterThan(0);
});
