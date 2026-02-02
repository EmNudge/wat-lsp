import { test, expect } from '@playwright/test';

test.describe('WAT LSP Playground', () => {
  test('page loads without critical errors', async ({ page }) => {
    const errors = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });

    await page.goto('/');
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });

    // Wait for initialization to complete
    await page.waitForTimeout(1000);

    // Filter out known non-critical errors
    const criticalErrors = errors.filter(
      (e) => !e.includes('MonacoEnvironment') && !e.includes('web worker')
    );

    expect(criticalErrors).toEqual([]);
  });

  test('LSP initializes and colors files correctly', async ({ page }) => {
    await page.goto('/');

    // Wait for LSP to be ready
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });
    await expect(page.locator('#lsp-indicator')).toHaveClass(/ready/);

    // Wait for file tree errors to be updated
    await page.waitForTimeout(500);

    // Verify that file tree has been populated with error indicators
    // Valid files should NOT have .has-error, invalid files SHOULD
    const validItemCount = await page.locator('.tree-item:not(.has-error)').count();
    const errorItemCount = await page.locator('.tree-item.has-error').count();

    // We should have at least some of each
    expect(validItemCount, 'Should have valid (non-error) files').toBeGreaterThan(0);
    expect(errorItemCount, 'Should have invalid (error) files').toBeGreaterThan(0);
  });

  test('editor loads with example code', async ({ page }) => {
    await page.goto('/');

    // Wait for Monaco editor to be present
    const editor = page.locator('#editor .monaco-editor');
    await expect(editor).toBeVisible({ timeout: 10000 });

    // Verify some WAT code is loaded (the hello example)
    const editorContent = page.locator('.view-lines');
    await expect(editorContent).toContainText('module');
  });

  test('compile button works', async ({ page }) => {
    await page.goto('/');

    // Wait for initialization
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });

    // Click compile
    await page.click('#compile-btn');

    // Wait for successful compilation
    const status = page.locator('#status');
    await expect(status).toHaveText('Compiled successfully', { timeout: 10000 });

    // Verify run button is enabled
    const runBtn = page.locator('#run-btn');
    await expect(runBtn).toBeEnabled();
  });

  test('can run compiled module', async ({ page }) => {
    await page.goto('/');

    // Wait for initialization
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });

    // Compile and run
    await page.click('#compile-btn');
    await expect(page.locator('#status')).toHaveText('Compiled successfully', { timeout: 10000 });

    await page.click('#run-btn');
    await expect(page.locator('#status')).toHaveText('Module ready', { timeout: 5000 });

    // Select a function and call it
    await page.selectOption('#export-fn-select', 'add');
    await page.fill('#fn-args', '2, 3');
    await page.click('#call-fn-btn');

    // Verify result
    const result = page.locator('#fn-result');
    await expect(result).toContainText('5');
  });

  test('file selection updates editor and error state', async ({ page }) => {
    await page.goto('/');

    // Wait for LSP to be ready
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });
    await page.waitForTimeout(500);

    // Expand the invalid folder
    await page.click('.tree-folder-header:has-text("invalid")');
    await page.waitForTimeout(200);

    // Click on an invalid file (should have .has-error)
    const invalidItem = page.locator('.tree-item.has-error').first();
    await invalidItem.click();
    await page.waitForTimeout(300);

    // Verify it's selected
    await expect(invalidItem).toHaveClass(/selected/);

    // Verify editor has some content
    const editorContent = page.locator('.view-lines');
    await expect(editorContent).toContainText('module');
  });

  test('editing a file updates its error state', async ({ page }) => {
    await page.goto('/');

    // Wait for LSP to be ready
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });
    await page.waitForTimeout(500);

    // Get initial state of hello file (should be valid)
    const helloItem = page.locator('.tree-item[data-id="hello"]');
    await expect(helloItem).not.toHaveClass(/has-error/);

    // Break the code by introducing an error
    await page.evaluate(() => {
      window.monacoEditor?.setValue('(module (func (i32.add)))'); // Missing operands
    });

    // Wait for diagnostics to update
    await page.waitForTimeout(500);

    // The current file should now show as having an error
    await expect(helloItem).toHaveClass(/has-error/);

    // Fix the code
    await page.evaluate(() => {
      window.monacoEditor?.setValue('(module (func (result i32) (i32.add (i32.const 1) (i32.const 2))))');
    });

    // Wait for diagnostics to update
    await page.waitForTimeout(500);

    // The file should no longer have an error
    await expect(helloItem).not.toHaveClass(/has-error/);
  });
});
