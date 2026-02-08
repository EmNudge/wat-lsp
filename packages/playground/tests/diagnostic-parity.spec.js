/**
 * Diagnostic Parity Tests (via File Coloring)
 *
 * These tests verify that the WASM LSP correctly identifies files with errors
 * by checking the file tree coloring. Files from the diagnostic corpus are
 * loaded and verified to have the expected error state.
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

        // Determine if this test expects errors
        const expectsErrors = expectedContent.diagnostics.some((d) => d.severity === 1);

        testCases.push({
          name: baseName,
          wat: watContent,
          description: expectedContent.description,
          expectsErrors,
        });
      }
    }
  }

  return testCases;
}

const testCases = loadCorpusTestCases();

/** Wait for the WASM LSP to finish initializing. */
async function waitForLSP(page) {
  await expect(page.locator('.lsp-status')).toContainText('LSP Ready', {
    timeout: 15000,
  });
}

test.describe('Diagnostic Parity (WASM vs Native)', () => {
  // Skip all tests if no corpus found
  test.skip(testCases.length === 0, 'No diagnostic corpus found');

  test('corpus files with expected errors should show error indicators', async ({ page }) => {
    test.setTimeout(60000);
    await page.goto('/');
    await waitForLSP(page);

    const filesExpectingErrors = testCases.filter((tc) => tc.expectsErrors);
    const filesExpectingNoErrors = testCases.filter((tc) => !tc.expectsErrors);

    // Test files that should have errors
    const errorFailures = [];
    for (const tc of filesExpectingErrors) {
      // Set editor content
      await page.evaluate((wat) => {
        window.monacoEditor?.setValue(wat);
      }, tc.wat);

      // Wait for diagnostics
      await page.waitForTimeout(500);

      // Check if editor has error markers
      const hasErrors = await page.evaluate(() => {
        const model = window.monacoEditor?.getModel();
        if (!model) return false;
        const markers = window.monaco.editor.getModelMarkers({ resource: model.uri });
        return markers.some((m) => m.severity === 8); // Monaco error severity
      });

      if (!hasErrors) {
        errorFailures.push(`${tc.name}: ${tc.description}`);
      }
    }

    // Test files that should NOT have errors
    const noErrorFailures = [];
    for (const tc of filesExpectingNoErrors) {
      // Set editor content
      await page.evaluate((wat) => {
        window.monacoEditor?.setValue(wat);
      }, tc.wat);

      // Wait for diagnostics
      await page.waitForTimeout(500);

      // Check if editor has error markers
      const hasErrors = await page.evaluate(() => {
        const model = window.monacoEditor?.getModel();
        if (!model) return false;
        const markers = window.monaco.editor.getModelMarkers({ resource: model.uri });
        return markers.some((m) => m.severity === 8); // Monaco error severity
      });

      if (hasErrors) {
        noErrorFailures.push(`${tc.name}: ${tc.description}`);
      }
    }

    // Report results
    if (errorFailures.length > 0) {
      console.log('Files that should have errors but do not:');
      errorFailures.forEach((f) => console.log(`  - ${f}`));
    }

    if (noErrorFailures.length > 0) {
      console.log('Files that should NOT have errors but do:');
      noErrorFailures.forEach((f) => console.log(`  - ${f}`));
    }

    expect(
      errorFailures,
      `${errorFailures.length} file(s) should show errors but do not:\n${errorFailures.join('\n')}`
    ).toHaveLength(0);

    expect(
      noErrorFailures,
      `${noErrorFailures.length} file(s) should NOT show errors but do:\n${noErrorFailures.join('\n')}`
    ).toHaveLength(0);

    console.log(
      `Verified ${filesExpectingErrors.length} files with expected errors, ` +
        `${filesExpectingNoErrors.length} files without expected errors`
    );
  });
});

// Meta-test to ensure corpus is being loaded
test('diagnostic corpus is loaded', () => {
  console.log(`Loaded ${testCases.length} diagnostic parity test cases`);
  expect(testCases.length).toBeGreaterThan(0);
});
