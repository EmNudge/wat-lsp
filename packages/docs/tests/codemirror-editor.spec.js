import { test, expect } from '@playwright/test';

test.describe('CodeMirror Editor Integration', () => {
  test('page loads without critical errors', async ({ page }) => {
    const errors = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });

    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const criticalErrors = errors.filter(
      (e) =>
        !e.includes('SharedArrayBuffer') &&
        !e.includes('wasm streaming compile failed') &&
        !e.includes('falling back to ArrayBuffer instantiation'),
    );

    expect(criticalErrors).toEqual([]);
  });

  test('CodeMirror editor replaces code blocks', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    // Check that CodeMirror editors are present
    const cmEditors = page.locator('.cm-editor');
    await expect(cmEditors.first()).toBeVisible({ timeout: 10000 });

    // Verify the original expressive-code blocks are replaced
    const expressiveCodeBlocks = page.locator('.expressive-code:not([data-cm-enhanced])');
    const watBlocks = await expressiveCodeBlocks.locator('pre[data-language="wat"]').count();
    expect(watBlocks).toBe(0);
  });

  test('code displays with proper line breaks', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const cmEditor = page.locator('.cm-editor').first();
    await expect(cmEditor).toBeVisible({ timeout: 10000 });

    // Check that multiple lines are rendered
    const cmLines = cmEditor.locator('.cm-line');
    const lineCount = await cmLines.count();

    expect(lineCount).toBeGreaterThan(1);
    console.log(`Found ${lineCount} lines in editor`);
  });

  test('editor is editable', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const cmEditor = page.locator('.cm-editor').first();
    await expect(cmEditor).toBeVisible({ timeout: 10000 });

    // Click to focus
    await cmEditor.click();
    await page.waitForTimeout(100);

    // Type some text
    await page.keyboard.type('test');
    await page.waitForTimeout(100);

    // Verify text was added
    const content = cmEditor.locator('.cm-content');
    await expect(content).toContainText('test');
  });

  test('editor accepts keyboard input and can undo', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const cmEditor = page.locator('.cm-editor').first();
    await expect(cmEditor).toBeVisible({ timeout: 10000 });

    // Click to focus and type
    await cmEditor.click();
    await page.waitForTimeout(100);
    await page.keyboard.type('hello');
    await page.waitForTimeout(100);

    // Verify text was added
    const modifiedContent = await cmEditor.locator('.cm-content').textContent();
    expect(modifiedContent).toContain('hello');

    // Test undo (Ctrl+Z)
    await page.keyboard.press('ControlOrMeta+z');
    await page.waitForTimeout(100);

    // After undo, 'hello' should be removed
    const afterUndoContent = await cmEditor.locator('.cm-content').textContent();
    expect(afterUndoContent).not.toContain('hello');
    console.log('Keyboard input and undo work correctly');
  });

  test('line numbers display correctly', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const cmEditor = page.locator('.cm-editor').first();
    await expect(cmEditor).toBeVisible({ timeout: 10000 });

    // Check line numbers are present
    const lineNumbers = cmEditor.locator('.cm-lineNumbers .cm-gutterElement');
    const count = await lineNumbers.count();

    expect(count).toBeGreaterThan(0);
    console.log(`Found ${count} line numbers`);
  });

  test('syntax highlighting is applied', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const cmEditor = page.locator('.cm-editor').first();
    await expect(cmEditor).toBeVisible({ timeout: 10000 });

    // Check that syntax tokens have styling (CodeMirror uses ͼ prefix classes)
    const content = await cmEditor.locator('.cm-content').innerHTML();

    // Should have some span elements with styling
    expect(content).toContain('<span');
    console.log('Syntax highlighting applied');
  });

  test('editor has correct height', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const container = page.locator('.wat-editor-container').first();
    await expect(container).toBeVisible({ timeout: 10000 });

    const box = await container.boundingBox();
    expect(box).not.toBeNull();
    expect(box.height).toBeGreaterThanOrEqual(120);
    expect(box.height).toBeLessThanOrEqual(450);

    console.log(`Editor container height: ${box.height}px`);
  });

  test('take screenshot of current state', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    const editor = page.locator('.cm-editor').first();
    await editor.screenshot({ path: 'test-results/codemirror-state.png' });
  });

  test('debug CSS styles on editor', async ({ page }) => {
    await page.goto('/stack/drop/');
    await page.waitForTimeout(2000);

    // Check computed styles on editor elements including gutters
    const styles = await page.evaluate(() => {
      const content = document.querySelector('.cm-content');
      const line = document.querySelector('.cm-line');
      const gutters = document.querySelector('.cm-gutters');
      const lineNumbers = document.querySelector('.cm-lineNumbers');
      const gutterElement = document.querySelector('.cm-gutterElement');

      const getStyles = (el, name) => {
        if (!el) return null;
        const s = window.getComputedStyle(el);
        return {
          name,
          fontSize: s.fontSize,
          lineHeight: s.lineHeight,
          padding: s.padding,
          margin: s.margin,
          height: s.height,
        };
      };

      return [
        getStyles(content, 'content'),
        getStyles(line, 'line'),
        getStyles(gutters, 'gutters'),
        getStyles(lineNumbers, 'lineNumbers'),
        getStyles(gutterElement, 'gutterElement'),
      ];
    });

    console.log('Computed styles:');
    styles.forEach((s) => s && console.log(JSON.stringify(s)));
  });
});
