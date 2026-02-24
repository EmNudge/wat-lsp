/**
 * Web Worker that executes JS host code with a compiled WASM module.
 * Captures console output and posts it back to the main thread.
 */

export interface ConsoleLine {
  level: 'log' | 'info' | 'warn' | 'error';
  args: string[];
}

interface RunMessage {
  wasmBytes: ArrayBuffer;
  hostCode: string;
}

/** Pretty-print a value for console output */
function formatValue(value: unknown): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'bigint' || typeof value === 'boolean') {
    return String(value);
  }
  if (value instanceof Uint8Array || value instanceof Int32Array || value instanceof Float64Array) {
    const name = value.constructor.name;
    const items = Array.from(value.slice(0, 20) as any).join(', ');
    const suffix = value.length > 20 ? `, ... (${value.length} items)` : '';
    return `${name} [${items}${suffix}]`;
  }
  if (Array.isArray(value)) {
    return `[${value.map(formatValue).join(', ')}]`;
  }
  if (typeof value === 'object') {
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

/** Create a fake console that captures output */
function createConsoleCapture(lines: ConsoleLine[]) {
  function capture(level: ConsoleLine['level']) {
    return (...args: unknown[]) => {
      lines.push({ level, args: args.map(formatValue) });
    };
  }
  return {
    log: capture('log'),
    info: capture('info'),
    warn: capture('warn'),
    error: capture('error'),
    debug: capture('log'),
    trace: capture('log'),
    dir: capture('log'),
    table: capture('log'),
    clear: () => {},
    assert: (condition: unknown, ...args: unknown[]) => {
      if (!condition) {
        lines.push({ level: 'error', args: ['Assertion failed:', ...args.map(formatValue)] });
      }
    },
    count: (() => {
      const counts = new Map<string, number>();
      return (label = 'default') => {
        const count = (counts.get(label) ?? 0) + 1;
        counts.set(label, count);
        lines.push({ level: 'log', args: [`${label}: ${count}`] });
      };
    })(),
    time: (() => {
      const timers = new Map<string, number>();
      return (label = 'default') => { timers.set(label, performance.now()); };
    })(),
    timeEnd: (() => {
      const timers = new Map<string, number>();
      return (label = 'default') => {
        const start = timers.get(label);
        if (start !== undefined) {
          lines.push({ level: 'log', args: [`${label}: ${(performance.now() - start).toFixed(3)}ms`] });
          timers.delete(label);
        }
      };
    })(),
  };
}

self.onmessage = async (e: MessageEvent<RunMessage>) => {
  const { wasmBytes, hostCode } = e.data;
  const lines: ConsoleLine[] = [];
  const fakeConsole = createConsoleCapture(lines);

  try {
    // Create an async function with wasmBytes, WebAssembly, and console injected
    const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
    const fn = new AsyncFunction('wasmBytes', 'WebAssembly', 'console', hostCode);
    await fn(new Uint8Array(wasmBytes), WebAssembly, fakeConsole);
    self.postMessage({ type: 'done', lines });
  } catch (err: any) {
    lines.push({ level: 'error', args: [err?.message ?? String(err)] });
    self.postMessage({ type: 'error', lines });
  }
};
