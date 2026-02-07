import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.join(__dirname, '..');
const instructionsPath = path.join(rootDir, 'instructions.md');
const outputDir = path.join(rootDir, 'src/content/docs/instructions');

// Category definitions
const categories = {
  i32: {
    title: 'i32 Instructions',
    description: '32-bit integer operations',
    prefix: 'i32.',
  },
  i64: {
    title: 'i64 Instructions',
    description: '64-bit integer operations',
    prefix: 'i64.',
  },
  f32: {
    title: 'f32 Instructions',
    description: '32-bit floating-point operations',
    prefix: 'f32.',
  },
  f64: {
    title: 'f64 Instructions',
    description: '64-bit floating-point operations',
    prefix: 'f64.',
  },
  'local-global': {
    title: 'Local & Global',
    description: 'Local and global variable operations',
    match: (name) => name.startsWith('local.') || name.startsWith('global.'),
  },
  control: {
    title: 'Control Flow',
    description: 'Control flow instructions',
    match: (name) =>
      [
        'block',
        'loop',
        'if',
        'then',
        'else',
        'br',
        'br_if',
        'br_table',
        'call',
        'call_indirect',
        'return',
        'return_call',
        'return_call_indirect',
        'unreachable',
        'nop',
        'call_ref',
        'return_call_ref',
        'br_on_null',
        'br_on_non_null',
      ].includes(name),
  },
  parametric: {
    title: 'Parametric',
    description: 'Stack manipulation instructions',
    match: (name) => ['drop', 'select'].includes(name),
  },
  memory: {
    title: 'Memory',
    description: 'Memory operations',
    match: (name) =>
      (name.startsWith('memory.') && !name.includes('.atomic.')) || name.startsWith('data.'),
  },
  table: {
    title: 'Table',
    description: 'Table operations',
    match: (name) => name.startsWith('table.') || name.startsWith('elem.'),
  },
  reference: {
    title: 'Reference',
    description: 'Reference type operations',
    match: (name) => {
      const gcCasts = ['ref.test', 'ref.cast', 'ref.cast_null', 'ref.eq', 'ref.i31'];
      if (gcCasts.includes(name)) return false;
      return name.startsWith('ref.') || ['any.convert_extern', 'extern.convert_any'].includes(name);
    },
  },
  module: {
    title: 'Module Structure',
    description: 'Module definition constructs',
    match: (name) =>
      [
        'module',
        'func',
        'param',
        'result',
        'local',
        'global',
        'table',
        'memory',
        'import',
        'export',
        'start',
        'type',
        'elem',
        'data',
      ].includes(name),
  },
  'gc-types': {
    title: 'GC Types',
    description: 'WasmGC type definitions',
    match: (name) =>
      ['sub', 'final', 'rec', 'field', 'struct', 'array', 'mut', 'shared', 'null', 'ref'].includes(
        name,
      ),
  },
  'gc-struct': {
    title: 'GC Struct',
    description: 'WasmGC struct operations',
    prefix: 'struct.',
  },
  'gc-array': {
    title: 'GC Array',
    description: 'WasmGC array operations',
    prefix: 'array.',
  },
  'gc-i31': {
    title: 'GC i31',
    description: 'WasmGC i31 operations',
    match: (name) => name.startsWith('i31.') || name === 'ref.i31',
  },
  'gc-casts': {
    title: 'GC Casts',
    description: 'WasmGC type cast operations',
    match: (name) =>
      ['ref.test', 'ref.cast', 'ref.cast_null', 'br_on_cast', 'br_on_cast_fail', 'ref.eq'].includes(
        name,
      ),
  },
  exceptions: {
    title: 'Exceptions',
    description: 'Exception handling operations',
    match: (name) =>
      [
        'throw',
        'throw_ref',
        'rethrow',
        'tag',
        'try_table',
        'catch',
        'catch_all',
        'catch_ref',
        'catch_all_ref',
      ].includes(name),
  },
  simd: {
    title: 'SIMD (v128)',
    description: '128-bit SIMD operations',
    match: (name) =>
      name.startsWith('v128.') ||
      name.startsWith('i8x16.') ||
      name.startsWith('i16x8.') ||
      name.startsWith('i32x4.') ||
      name.startsWith('i64x2.') ||
      name.startsWith('f32x4.') ||
      name.startsWith('f64x2.'),
  },
  atomic: {
    title: 'Atomics',
    description: 'Atomic memory operations (threads)',
    match: (name) => name.includes('.atomic.') || name === 'atomic.fence',
  },
  types: {
    title: 'Type Names',
    description: 'Value type names used in signatures',
    match: (name) =>
      [
        'i32',
        'i64',
        'f32',
        'f64',
        'v128',
        'i8x16',
        'i16x8',
        'i32x4',
        'i64x2',
        'f32x4',
        'f64x2',
        'funcref',
        'externref',
        'anyref',
        'eqref',
        'i31ref',
        'structref',
        'arrayref',
        'nullref',
        'nullfuncref',
        'nullexternref',
        'exnref',
        'nullexnref',
        'i8',
        'i16',
      ].includes(name),
  },
  misc: {
    title: 'Miscellaneous',
    description: 'Other instructions',
    match: () => true, // Catch-all
  },
};

// Parse instruction from markdown
function parseInstruction(block) {
  const lines = block.trim().split('\n');
  const nameLine = lines[0];
  const match = nameLine.match(/^## (.+)$/);
  if (!match) return null;

  const name = match[1];
  if (name === 'instruction.name') return null; // Skip template

  const content = lines.slice(1).join('\n').trim();

  // Parse signature
  const sigMatch = content.match(/Signature:\s*`([^`]+)`/);
  const signature = sigMatch ? sigMatch[1] : null;

  // Parse description (everything before Signature: or Example:)
  const sigIdx = content.indexOf('Signature:');
  const exIdx = content.indexOf('Example:');
  const descEnd = sigIdx > 0 ? sigIdx : exIdx > 0 ? exIdx : -1;
  const description = descEnd > 0 ? content.slice(0, descEnd).trim() : content.split('\n')[0];

  // Parse example (supports both ```wat and ```wat-snippet)
  const exampleMatch = content.match(/Example:\s*```(wat(?:-snippet)?)\n([\s\S]*?)```/);
  const exampleLang = exampleMatch ? exampleMatch[1] : 'wat';
  const example = exampleMatch ? exampleMatch[2].trim() : null;

  return { name, description, signature, example, exampleLang, rawContent: content };
}

// Categorize instruction - order matters! More specific matches first
function categorize(name) {
  // Check atomic first (they have prefixes like i32.atomic.)
  if (name.includes('.atomic.') || name === 'atomic.fence' || name.startsWith('memory.atomic.')) {
    return 'atomic';
  }

  // Check SIMD (includes prefixes like i32x4., f32x4.)
  if (
    name.startsWith('v128.') ||
    name.startsWith('i8x16.') ||
    name.startsWith('i16x8.') ||
    name.startsWith('i32x4.') ||
    name.startsWith('i64x2.') ||
    name.startsWith('f32x4.') ||
    name.startsWith('f64x2.')
  ) {
    return 'simd';
  }

  // Check GC operations
  if (name.startsWith('struct.')) return 'gc-struct';
  if (name.startsWith('array.')) return 'gc-array';
  if (name.startsWith('i31.') || name === 'ref.i31') return 'gc-i31';

  // Check GC casts
  if (
    ['ref.test', 'ref.cast', 'ref.cast_null', 'br_on_cast', 'br_on_cast_fail', 'ref.eq'].includes(
      name,
    )
  ) {
    return 'gc-casts';
  }

  // Check prefix-based categories first (i32., i64., f32., f64., etc.)
  for (const [key, cat] of Object.entries(categories)) {
    if (cat.prefix && name.startsWith(cat.prefix)) {
      return key;
    }
  }

  // Then check match-based categories (excluding the catch-all misc)
  for (const [key, cat] of Object.entries(categories)) {
    if (key !== 'misc' && cat.match && cat.match(name)) {
      return key;
    }
  }

  return 'misc';
}

// Generate markdown for a category page
function generateCategoryPage(category, instructions) {
  const cat = categories[category];
  let content = `---
title: ${cat.title}
description: ${cat.description}
---

`;

  for (const instr of instructions) {
    content += `## ${instr.name}\n\n`;
    content += `${instr.description}\n\n`;

    if (instr.signature) {
      content += `**Signature:** \`${instr.signature}\`\n\n`;
    }

    if (instr.example) {
      content += `**Example:**\n\n\`\`\`${instr.exampleLang}\n${instr.example}\n\`\`\`\n\n`;
    }

    content += '---\n\n';
  }

  return content.trimEnd() + '\n';
}

// Main
async function main() {
  const content = fs.readFileSync(instructionsPath, 'utf-8');

  // Split by ## at the start of a line (instruction headers)
  const blocks = content.split(/\n(?=## )/g).filter((b) => b.trim() && b.startsWith('## '));

  const instructions = blocks.map(parseInstruction).filter(Boolean);

  console.log(`Parsed ${instructions.length} instructions`);

  // Group by category
  const grouped = {};
  for (const instr of instructions) {
    const cat = categorize(instr.name);
    if (!grouped[cat]) grouped[cat] = [];
    grouped[cat].push(instr);
  }

  // Create output directory
  fs.mkdirSync(outputDir, { recursive: true });

  // Generate pages
  for (const [category, instrs] of Object.entries(grouped)) {
    if (instrs.length === 0) continue;

    const filename = `${category}.md`;
    const filepath = path.join(outputDir, filename);
    const content = generateCategoryPage(category, instrs);

    fs.writeFileSync(filepath, content);
    console.log(`Generated ${filename} with ${instrs.length} instructions`);
  }

  console.log('\nDone! Update astro.config.mjs to add these pages to the sidebar.');
}

main().catch(console.error);
