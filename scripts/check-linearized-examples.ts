#!/usr/bin/env npx tsx
// Validates that linearized (stack-form) WAT examples produce no new errors.
// Converts each playground example from folded to linear form using the
// docs linearizer, then runs wat-check on both versions and compares.

import { foldedToLinear } from '../packages/docs/src/lib/wat-linearizer';
import { readFileSync, writeFileSync, mkdirSync, readdirSync } from 'fs';
import { join, basename } from 'path';
import { execSync } from 'child_process';

const WAT_CHECK = process.env.WAT_CHECK ?? join(__dirname, '..', 'target', 'release', 'wat-check');
const EXAMPLES_DIR = join(__dirname, '..', 'packages', 'playground', 'examples');
const OUT_DIR = join(__dirname, '..', 'target', 'linearized-examples');

mkdirSync(OUT_DIR, { recursive: true });

function runWatCheck(filePath: string): string[] {
  try {
    const output = execSync(`"${WAT_CHECK}" "${filePath}" --format compact --errors-only 2>&1`, {
      encoding: 'utf-8',
      timeout: 10000,
    });
    return output.trim().split('\n').filter(l => l.trim().length > 0);
  } catch (e: any) {
    const output = ((e.stdout ?? '') + (e.stderr ?? '')).trim();
    return output.split('\n').filter(l => l.trim().length > 0);
  }
}

function normalizeMessage(diag: string): string {
  // Strip file path and line:col, keep just the severity and message
  return diag.replace(/^[^:]+:\d+:\d+:/, 'X:');
}

const exampleFiles = readdirSync(EXAMPLES_DIR)
  .filter(f => f.endsWith('.wat'))
  .sort();

let failures = 0;

for (const file of exampleFiles) {
  const origPath = join(EXAMPLES_DIR, file);
  const origSource = readFileSync(origPath, 'utf-8');

  let linearSource: string;
  try {
    linearSource = foldedToLinear(origSource);
  } catch (e: any) {
    console.log(`  FAIL  ${file}: linearizer error: ${e.message}`);
    failures++;
    continue;
  }

  if (linearSource === origSource) {
    console.log(`  skip  ${file} (unchanged)`);
    continue;
  }

  const linearPath = join(OUT_DIR, file);
  writeFileSync(linearPath, linearSource);

  const origErrors = runWatCheck(origPath);
  const linearErrors = runWatCheck(linearPath);

  // Normalize: strip paths/positions, compare error messages only
  const origMessages = new Set(origErrors.map(normalizeMessage));
  const newErrors = linearErrors.filter(d => !origMessages.has(normalizeMessage(d)));

  if (newErrors.length > 0) {
    console.log(`  FAIL  ${file}: ${newErrors.length} new error(s) after linearization`);
    for (const e of newErrors) {
      console.log(`        ${e}`);
    }
    failures++;
  } else {
    console.log(`  ok    ${file}`);
  }
}

console.log('');
if (failures > 0) {
  console.log(`${failures} file(s) failed linearization check`);
  process.exit(1);
} else {
  console.log(`All ${exampleFiles.length} examples pass linearization check`);
}
