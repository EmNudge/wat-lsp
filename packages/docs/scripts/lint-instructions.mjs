import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const instructionsPath = path.join(__dirname, '..', 'instructions.md');

const content = fs.readFileSync(instructionsPath, 'utf-8');
const lines = content.split('\n');

const errors = [];
const seen = new Set();

let inCodeBlock = false;
let currentName = null;
let currentStart = 0;
let hasDescription = false;
let hasExample = false;
let exampleHasFence = false;

function finishBlock() {
  if (!currentName) return;

  if (!hasDescription) {
    errors.push(`line ${currentStart}: "${currentName}" has no description`);
  }
  if (hasExample && !exampleHasFence) {
    errors.push(`line ${currentStart}: "${currentName}" has Example: but no \`\`\`wat code fence`);
  }
}

for (let i = 0; i < lines.length; i++) {
  const line = lines[i];
  const trimmed = line.trim();
  const lineNum = i + 1;

  // Track code block boundaries
  if (trimmed.startsWith('```')) {
    if (!inCodeBlock) {
      inCodeBlock = true;
      // Check if this fence follows an Example: line (within the current block)
      if (currentName && hasExample && !exampleHasFence) {
        if (trimmed === '```wat') {
          exampleHasFence = true;
        }
      }
    } else {
      inCodeBlock = false;
    }
    continue;
  }

  // Skip everything inside code blocks
  if (inCodeBlock) continue;

  // Separator
  if (trimmed === '---') {
    finishBlock();
    currentName = null;
    continue;
  }

  // New instruction header
  if (trimmed.startsWith('## ')) {
    const name = trimmed.slice(3).trim();

    // If previous block didn't end with ---
    if (currentName) {
      errors.push(`line ${currentStart}: "${currentName}" is not terminated with ---`);
      finishBlock();
    }

    // Skip the template example in the file header
    if (name === 'instruction.name') {
      errors.push(
        `line ${lineNum}: found template placeholder "instruction.name" — use actual instruction name`,
      );
      currentName = null;
      continue;
    }

    // Duplicate check
    if (seen.has(name)) {
      errors.push(`line ${lineNum}: duplicate instruction "${name}"`);
    }
    seen.add(name);

    currentName = name;
    currentStart = lineNum;
    hasDescription = false;
    hasExample = false;
    exampleHasFence = false;
    continue;
  }

  // Skip top-level title
  if (trimmed.startsWith('# ')) continue;

  // Content lines within a block
  if (!currentName) continue;

  // First non-empty line after ## is the description
  if (!hasDescription && trimmed.length > 0) {
    hasDescription = true;
  }

  // Signature line
  if (trimmed.startsWith('Signature:')) {
    const afterColon = trimmed.slice('Signature:'.length).trim();
    if (!afterColon) {
      errors.push(`line ${lineNum}: "${currentName}" Signature: is empty`);
    }
  }

  // Example line
  if (trimmed.startsWith('Example:')) {
    hasExample = true;
  }
}

// Handle last block if file doesn't end with ---
if (currentName) {
  errors.push(`line ${currentStart}: "${currentName}" is not terminated with ---`);
  finishBlock();
}

if (errors.length > 0) {
  console.error(`Found ${errors.length} issue(s) in instructions.md:\n`);
  for (const err of errors) {
    console.error(`  ${err}`);
  }
  process.exit(1);
} else {
  console.log(`instructions.md OK — ${seen.size} instructions, all well-formed`);
}
