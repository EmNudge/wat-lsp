/**
 * File Tree Error Coloring Tests
 *
 * These tests verify that the sidebar correctly colors files based on
 * whether they have errors:
 *
 * - docs/examples/*.wat (valid examples) should NOT have .has-error class
 * - docs/examples/invalid/*.wat (invalid examples) should have .has-error class
 *
 * This is a simple proxy test for LSP correctness - if the LSP is working,
 * valid files will be white and invalid files will be red.
 */

import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// ES module equivalent of __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Path to docs examples (relative to project root)
const DOCS_DIR = path.join(__dirname, '../../../docs/examples');
const INVALID_DIR = path.join(DOCS_DIR, 'invalid');

/**
 * Load all .wat files from a directory
 */
function loadWatFiles(dir) {
  if (!fs.existsSync(dir)) {
    return [];
  }

  return fs
    .readdirSync(dir)
    .filter((file) => file.endsWith('.wat'))
    .map((file) => file.replace('.wat', ''));
}

const validExampleIds = loadWatFiles(DOCS_DIR);
const invalidExampleIds = loadWatFiles(INVALID_DIR).map((id) => `invalid_${id}`);

/** Wait for the WASM LSP to finish initializing. */
async function waitForLSP(page) {
  await expect(page.locator('.lsp-status')).toContainText('LSP Ready', {
    timeout: 15000,
  });
}

test.describe('File Tree Error Coloring', () => {
  test('valid examples should not have error coloring', async ({ page }) => {
    test.setTimeout(30000);
    await page.goto('/');
    await waitForLSP(page);

    // Wait a bit for file tree errors to be updated
    await page.waitForTimeout(500);

    // Check that valid example files do NOT have .has-error class
    const failures = [];

    for (const id of validExampleIds) {
      const item = page.locator(`.tree-item[data-id="${id}"]`);
      const hasError = await item.evaluate((el) => el.classList.contains('has-error'));

      if (hasError) {
        failures.push(id);
      }
    }

    expect(
      failures,
      `${failures.length} valid example(s) incorrectly marked with errors:\n` +
        failures.map((f) => `  - ${f}`).join('\n')
    ).toHaveLength(0);

    console.log(`Verified ${validExampleIds.length} valid examples have no error coloring`);
  });

  test('invalid examples should have error coloring', async ({ page }) => {
    test.setTimeout(30000);
    await page.goto('/');
    await waitForLSP(page);

    // Wait a bit for file tree errors to be updated
    await page.waitForTimeout(500);

    // Invalid items are in a collapsed section (v-show) but still in the DOM.
    // Check that they have .has-error class.
    const failures = [];

    for (const id of invalidExampleIds) {
      const item = page.locator(`.tree-item[data-id="${id}"]`);
      const hasError = await item.evaluate((el) => el.classList.contains('has-error'));

      if (!hasError) {
        failures.push(id);
      }
    }

    expect(
      failures,
      `${failures.length} invalid example(s) missing error coloring:\n` +
        failures.map((f) => `  - ${f}`).join('\n')
    ).toHaveLength(0);

    console.log(`Verified ${invalidExampleIds.length} invalid examples have error coloring`);
  });

  test('error coloring uses red color', async ({ page }) => {
    test.setTimeout(30000);
    await page.goto('/');
    await waitForLSP(page);
    await page.waitForTimeout(500);

    // Expand the invalid folder so error items are visible
    await page.locator('.tree-group-header:has-text("invalid")').click();
    await page.waitForTimeout(200);

    // Find an item with .has-error and verify its .file-name child has red color
    const errorItems = page.locator('.tree-item.has-error');
    const count = await errorItems.count();

    if (count > 0) {
      const color = await errorItems.first().evaluate((el) => {
        const fileName = el.querySelector('.file-name');
        return window.getComputedStyle(fileName || el).color;
      });

      // Check it's reddish (CSS --error-color is #d14d41 = rgb(209, 77, 65))
      const match = color.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      expect(match, 'Color should be in rgb format').toBeTruthy();

      if (match) {
        const [, r, g, b] = match.map(Number);
        expect(r, 'Red component should be high').toBeGreaterThan(200);
        expect(g, 'Green component should be low').toBeLessThan(100);
        expect(b, 'Blue component should be low').toBeLessThan(100);
      }
    }
  });
});

// Meta-tests to ensure examples are being loaded
test('valid examples are loaded', () => {
  console.log(`Loaded ${validExampleIds.length} valid example files`);
  expect(validExampleIds.length).toBeGreaterThan(0);
});

test('invalid examples are loaded', () => {
  console.log(`Loaded ${invalidExampleIds.length} invalid example files`);
  expect(invalidExampleIds.length).toBeGreaterThan(0);
});
