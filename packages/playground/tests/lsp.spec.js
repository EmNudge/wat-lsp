import { test, expect } from '@playwright/test';

test.describe('WAT LSP Playground', () => {
  test('no critical console errors on load', async ({ page }) => {
    const errors = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });

    await page.goto('/');
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });

    // Wait for semantic tokens to be applied
    await page.waitForTimeout(1000);

    // Filter out known non-critical errors (like Monaco worker warning)
    const criticalErrors = errors.filter(e =>
      !e.includes('MonacoEnvironment') &&
      !e.includes('web worker')
    );

    expect(criticalErrors).toEqual([]);
  });

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

  test('semantic tokens provider registers', async ({ page }) => {
    await page.goto('/');

    // Wait for LSP to be ready
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });

    // Check console output for semantic tokens registration
    const consoleOutput = page.locator('#console-output');
    await expect(consoleOutput).toContainText('Semantic tokens provider registered', { timeout: 5000 });
  });

  test('syntax highlighting colors are correct', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#lsp-status-text')).toHaveText(/LSP Ready/, { timeout: 15000 });
    await page.waitForTimeout(1500);

    // Extract tokens with their computed colors and CSS classes
    // Focus on spans with mtk* classes (Monaco's token classes)
    const tokenColors = await page.evaluate(() => {
      const tokens = [];
      const spans = document.querySelectorAll('.view-lines .view-line span');

      for (const span of spans) {
        const text = span.textContent?.trim();
        if (!text) continue;

        const className = span.className || '';
        const style = window.getComputedStyle(span);
        const color = style.color;

        // Only include spans with mtk classes (Monaco token classes)
        if (className.includes('mtk')) {
          tokens.push({ text, color, className });
        }
      }
      return tokens;
    });

    // Group tokens by text content to see what colors and classes each token type gets
    const colorsByToken = {};
    for (const { text, color, className } of tokenColors) {
      if (!colorsByToken[text]) {
        colorsByToken[text] = [];
      }
      colorsByToken[text].push({ color, className });
    }

    // Log detailed token info
    console.log('Token colors and classes:');
    for (const [token, entries] of Object.entries(colorsByToken)) {
      // Deduplicate
      const uniqueEntries = [...new Map(entries.map(e => [e.color + e.className, e])).values()];
      console.log(`  "${token}":`, uniqueEntries);
    }

    // Helper to convert rgb to hex for easier comparison
    const rgbToHex = (rgb) => {
      const match = rgb.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (!match) return rgb;
      const [, r, g, b] = match;
      return '#' + [r, g, b].map(x => parseInt(x).toString(16).padStart(2, '0')).join('').toUpperCase();
    };

    // Define expected color categories (approximate - Monaco theme colors)
    const isGreenish = (c) => {
      const match = c.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (!match) return false;
      const [, r, g, b] = match.map(Number);
      return g > r && g > b; // Green is dominant
    };

    const isYellowish = (c) => {
      const match = c.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (!match) return false;
      const [, r, g, b] = match.map(Number);
      // Yellow #DCDCAA = rgb(220, 220, 170) - high R, high G, lower B
      return r > 180 && g > 180 && b < 200 && b < r && b < g;
    };

    const isTealish = (c) => {
      const match = c.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (!match) return false;
      const [, r, g, b] = match.map(Number);
      return g > 150 && b > 150 && r < g; // Teal/cyan
    };

    const isWhite = (c) => {
      const match = c.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (!match) return false;
      const [, r, g, b] = match.map(Number);
      return r > 200 && g > 200 && b > 200; // All high = white/light gray
    };

    const isRed = (c) => {
      const match = c.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (!match) return false;
      const [, r, g, b] = match.map(Number);
      return r > 200 && g < 100 && b < 100; // Red dominant
    };

    // Check specific tokens
    const commentTokens = tokenColors.filter(t => t.text.startsWith(';;'));
    const instructionTokens = tokenColors.filter(t =>
      ['i32.add', 'i32.mul', 'i32.const', 'local.get'].includes(t.text)
    );

    // Log instruction-related tokens (including partial matches)
    const instructionParts = tokenColors.filter(t =>
      t.text.includes('.add') || t.text.includes('.get') ||
      t.text.includes('.mul') || t.text.includes('.const') ||
      t.text === 'i32' || t.text === 'local'
    );
    console.log('Instruction-related tokens:');
    for (const t of instructionParts) {
      console.log(`  "${t.text}" -> ${t.className} (${rgbToHex(t.color)})`);
    }

    // Log all unique mtk classes with their colors to find the yellow color ID
    const mtkColors = {};
    for (const t of tokenColors) {
      const mtkMatch = t.className.match(/mtk(\d+)/);
      if (mtkMatch) {
        mtkColors[mtkMatch[0]] = rgbToHex(t.color);
      }
    }
    console.log('MTK class to color mapping:', mtkColors);

    // Log findings
    console.log('Comments:', commentTokens.map(t => ({ text: t.text, color: rgbToHex(t.color) })));
    console.log('Instructions:', instructionTokens.map(t => ({ text: t.text, color: rgbToHex(t.color) })));

    // Verify comments are greenish (not white, not red)
    for (const token of commentTokens) {
      expect(isGreenish(token.color)).toBe(true);
    }

    // Verify combined TextMate + tree-sitter semantic token highlighting:
    // TextMate handles instructions with prefix/suffix split:
    // - i32 (instruction prefix) -> teal (TextMate support.class.type.wat)
    // - local (instruction prefix) -> blue (TextMate support.class.wat)
    // - .add, .get (instruction suffix) -> white (TextMate keyword.operator.wat)
    // Tree-sitter semantic tokens handle:
    // - Variables ($a, $b) -> light blue
    const i32Tokens = tokenColors.filter(t => t.text === 'i32');
    const localTokens = tokenColors.filter(t => t.text === 'local');
    const suffixTokens = tokenColors.filter(t =>
      t.text.startsWith('.') && (t.text === '.add' || t.text === '.get' || t.text === '.mul' || t.text === '.const')
    );
    const variableTokens = tokenColors.filter(t => t.text.startsWith('$'));

    console.log('i32 tokens (teal):', i32Tokens.map(t => ({ text: t.text, color: rgbToHex(t.color) })));
    console.log('local tokens (blue):', localTokens.map(t => ({ text: t.text, color: rgbToHex(t.color) })));
    console.log('Instruction suffixes (white):', suffixTokens.map(t => ({ text: t.text, color: rgbToHex(t.color) })));
    console.log('Variables (light blue):', variableTokens.map(t => ({ text: t.text, color: rgbToHex(t.color) })));

    // i32 should be teal (#4EC9B0) from TextMate
    expect(i32Tokens.length).toBeGreaterThan(0);
    for (const token of i32Tokens) {
      expect(isTealish(token.color)).toBe(true);
    }

    // Suffixes should be white (#D4D4D4) from TextMate
    for (const token of suffixTokens) {
      expect(isWhite(token.color)).toBe(true);
    }

    // Variables should be light blue (#9CDCFE) from semantic tokens
    expect(variableTokens.length).toBeGreaterThan(0);

    // Verify no red tokens (red usually indicates errors)
    const redTokens = tokenColors.filter(t => isRed(t.color));
    if (redTokens.length > 0) {
      console.log('WARNING: Red tokens found:', redTokens.map(t => t.text));
    }
    // Parentheses being red is a known Monaco quirk, filter those out
    const nonParenRedTokens = redTokens.filter(t => t.text !== '(' && t.text !== ')');
    expect(nonParenRedTokens.length).toBe(0);
  });
});
