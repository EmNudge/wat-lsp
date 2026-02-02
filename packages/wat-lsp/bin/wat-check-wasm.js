#!/usr/bin/env node

/**
 * wat-check-wasm: A standalone CLI tool for scanning WAT files for issues
 *
 * This tool uses the WASM build of wat-lsp to provide file validation.
 * It mirrors the functionality of the native wat-check binary.
 */

import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Parser } from 'web-tree-sitter';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Parse command line arguments
function parseArgs(args) {
  const result = {
    files: [],
    format: 'text',
    errorsOnly: false,
    quiet: false,
    help: false,
  };

  let i = 0;
  while (i < args.length) {
    const arg = args[i];

    if (arg === '-h' || arg === '--help') {
      result.help = true;
    } else if (arg === '-f' || arg === '--format') {
      i++;
      result.format = args[i] || 'text';
    } else if (arg === '-e' || arg === '--errors-only') {
      result.errorsOnly = true;
    } else if (arg === '-q' || arg === '--quiet') {
      result.quiet = true;
    } else if (arg === '-') {
      // Special case: '-' means stdin
      result.files.push(arg);
    } else if (!arg.startsWith('-')) {
      result.files.push(arg);
    } else {
      console.error(`Unknown option: ${arg}`);
      process.exit(1);
    }
    i++;
  }

  return result;
}

function printHelp() {
  console.log(`wat-check-wasm - WAT file checker using WASM build

Usage: wat-check-wasm [OPTIONS] <FILES>...

Arguments:
  <FILES>  Files to check. Use '-' to read from stdin.

Options:
  -f, --format <FORMAT>  Output format: text, json, compact [default: text]
  -e, --errors-only      Only show errors (hide warnings and hints)
  -q, --quiet            Suppress all output except errors (for scripting)
  -h, --help             Print help
`);
}

function severityToString(severity) {
  switch (severity) {
    case 1:
      return 'error';
    case 2:
      return 'warning';
    case 3:
      return 'info';
    case 4:
      return 'hint';
    default:
      return 'unknown';
  }
}

function severityToCompact(severity) {
  switch (severity) {
    case 1:
      return 'E';
    case 2:
      return 'W';
    case 3:
      return 'I';
    case 4:
      return 'H';
    default:
      return '?';
  }
}

function printDiagnosticsText(filename, diagnostics) {
  for (const d of diagnostics) {
    const line = d.range.start.line + 1;
    const col = d.range.start.character + 1;
    const severity = severityToString(d.severity);
    console.log(`${filename}:${line}:${col}: ${severity}: ${d.message}`);
  }
}

function printDiagnosticsCompact(filename, diagnostics) {
  for (const d of diagnostics) {
    const line = d.range.start.line + 1;
    const col = d.range.start.character + 1;
    const severity = severityToCompact(d.severity);
    console.log(`${filename}:${line}:${col}:${severity}:${d.message}`);
  }
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString('utf8');
}

/**
 * Initialize WatLSP for Node.js environment
 */
async function initWatLSP() {
  // Paths to WASM files
  const distWasmDir = join(__dirname, '..', 'dist', 'wasm');
  const treeSitterWasmPath = join(distWasmDir, 'tree-sitter.wasm');
  const watLspWasmPath = join(distWasmDir, 'wat_lsp_rust_bg.wasm');

  // Initialize web-tree-sitter with file:// URL for Node.js
  await Parser.init({
    locateFile: (file) => {
      if (file === 'tree-sitter.wasm') {
        return `file://${treeSitterWasmPath}`;
      }
      return file;
    },
  });

  // Load the wat-lsp WASM module
  const { default: initWasm, WatLSP } = await import('../dist/wasm/wat_lsp_rust.js');

  // In Node.js, we need to pass the WASM file as a buffer or URL
  const wasmBuffer = readFileSync(watLspWasmPath);
  await initWasm(wasmBuffer);

  // Create and initialize the LSP instance
  const lsp = new WatLSP();
  const success = await lsp.initialize();

  if (!success) {
    throw new Error('Failed to initialize WAT LSP');
  }

  return lsp;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.help) {
    printHelp();
    process.exit(0);
  }

  if (args.files.length === 0) {
    console.error('Error: No files specified');
    printHelp();
    process.exit(1);
  }

  // Initialize the WASM LSP
  let lsp;
  try {
    lsp = await initWatLSP();
  } catch (e) {
    console.error(`Failed to initialize WASM LSP: ${e.message}`);
    process.exit(1);
  }

  const allResults = [];
  let totalErrors = 0;
  let totalWarnings = 0;
  let hadReadError = false;

  for (const path of args.files) {
    let filename;
    let source;

    if (path === '-') {
      // Read from stdin
      try {
        source = await readStdin();
        filename = '<stdin>';
      } catch (e) {
        console.error(`stdin: Failed to read: ${e.message}`);
        hadReadError = true;
        continue;
      }
    } else {
      // Read from file
      const resolvedPath = resolve(path);
      if (!existsSync(resolvedPath)) {
        console.error(`${path}: File not found`);
        hadReadError = true;
        continue;
      }
      try {
        source = readFileSync(resolvedPath, 'utf8');
        filename = path;
      } catch (e) {
        console.error(`${path}: Failed to read: ${e.message}`);
        hadReadError = true;
        continue;
      }
    }

    // Parse and get diagnostics
    lsp.parse(source);
    let diagnostics = lsp.provideDiagnostics();

    // Convert to array if needed (JsValue might be array-like)
    diagnostics = Array.from(diagnostics);

    // Filter errors only if requested
    if (args.errorsOnly) {
      diagnostics = diagnostics.filter((d) => d.severity === 1);
    }

    const errorCount = diagnostics.filter((d) => d.severity === 1).length;
    const warningCount = diagnostics.filter((d) => d.severity === 2).length;

    totalErrors += errorCount;
    totalWarnings += warningCount;

    switch (args.format) {
      case 'text':
        if (diagnostics.length > 0) {
          printDiagnosticsText(filename, diagnostics);
        }
        break;
      case 'compact':
        printDiagnosticsCompact(filename, diagnostics);
        break;
      case 'json':
        allResults.push({
          file: filename,
          diagnostics: diagnostics.map((d) => ({
            line: d.range.start.line + 1,
            column: d.range.start.character + 1,
            end_line: d.range.end.line + 1,
            end_column: d.range.end.character + 1,
            severity: severityToString(d.severity),
            message: d.message,
          })),
          error_count: errorCount,
          warning_count: warningCount,
        });
        break;
    }
  }

  // JSON output at the end
  if (args.format === 'json') {
    const output = {
      files: allResults,
      summary: {
        total_errors: totalErrors,
        total_warnings: totalWarnings,
        files_checked: args.files.length,
      },
    };
    console.log(JSON.stringify(output, null, 2));
  } else if (!args.quiet) {
    // Summary for text output
    if (args.files.length > 1 || totalErrors > 0 || totalWarnings > 0) {
      const fileWord = args.files.length === 1 ? 'file' : 'files';
      console.error(
        `\nChecked ${args.files.length} ${fileWord}: ${totalErrors} error(s), ${totalWarnings} warning(s)`
      );
    }
  }

  // Exit code: 1 if errors found, 2 if read errors, 0 if clean
  if (totalErrors > 0) {
    process.exit(1);
  } else if (hadReadError) {
    process.exit(2);
  } else {
    process.exit(0);
  }
}

main().catch((e) => {
  console.error(`Fatal error: ${e.message}`);
  process.exit(1);
});
