import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseInstructionsFile } from './parse-instructions.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const instructionsPath = path.join(__dirname, '..', 'instructions.md');

const { instructions, errors } = parseInstructionsFile(instructionsPath);

if (errors.length > 0) {
  console.error(`Found ${errors.length} issue(s) in instructions.md:\n`);
  for (const err of errors) {
    console.error(`  ${err}`);
  }
  process.exit(1);
} else {
  console.log(`instructions.md OK — ${instructions.length} instructions, all well-formed`);
}
