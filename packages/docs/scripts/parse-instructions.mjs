// Shared parser for `packages/docs/instructions.md`.
//
// Both `lint-instructions.mjs` (CI validation) and
// `generate-instruction-docs.mjs` (docs site codegen) import this module so the
// two agree on one parsed instruction set instead of maintaining separate,
// drifting ad-hoc parsers. It mirrors the semantics of the Rust build-time
// parser in `build_support/instruction_docs.rs`:
//   * `## name` starts an entry, `---` (outside code fences) ends it,
//   * `## name` lines inside a ``` fence are content, not new entries,
//   * the first non-empty content line is the description,
//   * `Signature:` and `Example:` are recognized fields.
//
// `parseInstructions` returns the structured entries plus any validation errors
// so callers can either fail (lint) or filter/render (codegen).

import fs from 'node:fs';

/**
 * Parse instruction markdown into structured blocks.
 *
 * @param {string} content - raw markdown
 * @returns {{ instructions: Array<object>, errors: string[] }}
 */
export function parseInstructions(content) {
  const lines = content.split('\n');

  const instructions = [];
  const errors = [];
  const seen = new Map(); // name -> first line number

  let inCodeBlock = false;
  let current = null; // { name, startLine, bodyLines, hasDescription, hasExample, exampleHasFence }

  function finishBlock() {
    if (!current) return;
    if (!current.hasDescription) {
      errors.push(`line ${current.startLine}: "${current.name}" has no description`);
    }
    if (current.hasExample && !current.exampleHasFence) {
      errors.push(
        `line ${current.startLine}: "${current.name}" has Example: but no \`\`\`wat code fence`,
      );
    }
    instructions.push(current);
    current = null;
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    const lineNum = i + 1;

    // Code block boundaries.
    if (trimmed.startsWith('```')) {
      if (!inCodeBlock) {
        inCodeBlock = true;
        if (current && current.hasExample && !current.exampleHasFence && trimmed === '```wat') {
          current.exampleHasFence = true;
        }
      } else {
        inCodeBlock = false;
      }
      if (current) current.bodyLines.push(line);
      continue;
    }

    // Inside a code block: content only, never a new header/separator.
    if (inCodeBlock) {
      if (current) current.bodyLines.push(line);
      continue;
    }

    // Separator ends the current block.
    if (trimmed === '---') {
      finishBlock();
      continue;
    }

    // New instruction header.
    if (trimmed.startsWith('## ')) {
      const name = trimmed.slice(3).trim();

      // Missing terminating `---` before the next header.
      if (current) {
        errors.push(`line ${current.startLine}: "${current.name}" is not terminated with ---`);
        finishBlock();
      }

      // Template placeholder must not ship as a real entry.
      if (name === 'instruction.name') {
        errors.push(
          `line ${lineNum}: found template placeholder "instruction.name" — use actual instruction name`,
        );
        continue;
      }

      if (seen.has(name)) {
        errors.push(
          `line ${lineNum}: duplicate instruction "${name}" (first defined at line ${seen.get(name)})`,
        );
      } else {
        seen.set(name, lineNum);
      }

      current = {
        name,
        startLine: lineNum,
        bodyLines: [],
        hasDescription: false,
        hasExample: false,
        exampleHasFence: false,
      };
      continue;
    }

    // Top-level document title.
    if (trimmed.startsWith('# ')) continue;

    // Content lines within a block.
    if (!current) continue;
    current.bodyLines.push(line);

    if (!current.hasDescription && trimmed.length > 0) {
      current.hasDescription = true;
    }

    if (trimmed.startsWith('Signature:')) {
      const afterColon = trimmed.slice('Signature:'.length).trim();
      if (!afterColon) {
        errors.push(`line ${lineNum}: "${current.name}" Signature: is empty`);
      }
    }

    if (trimmed.startsWith('Example:')) {
      current.hasExample = true;
    }
  }

  // Trailing block with no closing `---`.
  if (current) {
    errors.push(`line ${current.startLine}: "${current.name}" is not terminated with ---`);
    finishBlock();
  }

  // Derive the fields the docs generator needs from each block's body.
  for (const instr of instructions) {
    const body = instr.bodyLines.join('\n').trim();
    instr.rawContent = body;

    const sigMatch = body.match(/Signature:\s*`([^`]+)`/);
    instr.signature = sigMatch ? sigMatch[1] : null;

    const sigIdx = body.indexOf('Signature:');
    const exIdx = body.indexOf('Example:');
    const descEnd = sigIdx > 0 ? sigIdx : exIdx > 0 ? exIdx : -1;
    instr.description = descEnd > 0 ? body.slice(0, descEnd).trim() : body.split('\n')[0];

    const exampleMatch = body.match(/Example:\s*```wat\n([\s\S]*?)```/);
    instr.example = exampleMatch ? exampleMatch[1].trim() : null;
  }

  return { instructions, errors };
}

/** Read and parse the instructions markdown at `filePath`. */
export function parseInstructionsFile(filePath) {
  const content = fs.readFileSync(filePath, 'utf-8');
  return parseInstructions(content);
}
