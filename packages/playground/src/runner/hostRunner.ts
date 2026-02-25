/**
 * Main-thread runner that spawns a Web Worker to execute host code.
 * Provides timeout protection and single-flight execution.
 */

import type { ConsoleLine } from './preview-worker';
export type { ConsoleLine };

const TIMEOUT_MS = 3000;

let activeCleanup: (() => void) | null = null;

/**
 * Run JavaScript host code with the given WASM bytes in a sandboxed worker.
 * Returns captured console output lines.
 */
export function runHostCode(wasmBytes: Uint8Array, hostCode: string): Promise<ConsoleLine[]> {
  // Cancel previous run if still active
  if (activeCleanup) {
    activeCleanup();
    activeCleanup = null;
  }

  return new Promise((resolve) => {
    const worker = new Worker(
      new URL('./preview-worker.ts', import.meta.url),
      { type: 'module' }
    );

    let settled = false;

    function cleanup() {
      if (!settled) {
        settled = true;
        worker.terminate();
      }
      if (activeCleanup === cleanup) {
        activeCleanup = null;
      }
    }

    activeCleanup = cleanup;

    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        worker.terminate();
        resolve([{ level: 'error', args: [`Execution timed out after ${TIMEOUT_MS / 1000}s`] }]);
      }
    }, TIMEOUT_MS);

    worker.onmessage = (e) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      worker.terminate();
      resolve(e.data.lines ?? []);
    };

    worker.onerror = (e) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      worker.terminate();
      resolve([{ level: 'error', args: [e.message ?? 'Worker error'] }]);
    };

    // Transfer the buffer for efficiency
    const buffer = wasmBytes.buffer.slice(0);
    worker.postMessage({ wasmBytes: buffer, hostCode }, [buffer]);
  });
}
