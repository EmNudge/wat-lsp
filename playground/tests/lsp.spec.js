import { test, expect } from '@playwright/test';

test.describe('WAT LSP Playground', () => {
  test('LSP initializes successfully', async ({ page }) => {
    await page.goto('/');

    // Wait for LSP status to show ready (either full or fallback mode)
    const lspStatus = page.locator('#lsp-status-text');
    await expect(lspStatus).toHaveText(/LSP Ready/, { timeout: 15000 });

    // Verify the indicator has the ready class
    const indicator = page.locator('#lsp-indicator');
    await expect(indicator).toHaveClass(/ready/);
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

  test('wabt initializes successfully', async ({ page }) => {
    await page.goto('/');

    // Check console output for wabt initialization
    const consoleOutput = page.locator('#console-output');
    await expect(consoleOutput).toContainText('wabt.js initialized', { timeout: 10000 });
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
});
