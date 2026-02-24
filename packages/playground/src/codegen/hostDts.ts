/**
 * Generates a .d.ts file for the host code, enabling Monaco JS IntelliSense.
 * Only declares globals injected by the worker runtime — the generated code
 * itself declares everything else (module, instance, exports).
 */

import type { WatModuleInfo, WatValueType } from './watTypeExtractor';

/** Map WAT value types to TypeScript types */
function watTypeToTs(type: WatValueType): string {
  switch (type) {
    case 'i32': return 'number';
    case 'i64': return 'bigint';
    case 'f32': return 'number';
    case 'f64': return 'number';
    case 'v128': return 'number';
    case 'funcref': return 'Function | null';
    case 'externref': return 'any';
    default: return 'number';
  }
}

/**
 * Generate a .d.ts declaration for the host code.
 * Keeps it minimal: only `wasmBytes` (injected by worker) and typed
 * JSDoc-style interfaces that the generated code can reference.
 */
export function generateHostDts(moduleInfo: WatModuleInfo): string {
  const lines: string[] = [];

  lines.push('// Auto-generated type declarations for WASM host code');
  lines.push('declare const wasmBytes: BufferSource;');
  lines.push('');

  // Generate an interface for the module's exports so generated code can cast to it
  const funcExports = moduleInfo.exports.filter(e => e.kind === 'function');
  if (funcExports.length > 0) {
    lines.push('interface WasmExports {');
    for (const exp of funcExports) {
      if (!/^[a-zA-Z_$][a-zA-Z0-9_$]*$/.test(exp.name)) continue;
      if (!exp.signature) {
        lines.push(`  ${exp.name}: Function;`);
        continue;
      }
      const params = exp.signature.params
        .map((p, i) => `${p.name?.replace('$', '') ?? `p${i}`}: ${watTypeToTs(p.type)}`)
        .join(', ');
      const ret = exp.signature.results.length === 0
        ? 'void'
        : exp.signature.results.length === 1
          ? watTypeToTs(exp.signature.results[0])
          : `[${exp.signature.results.map(watTypeToTs).join(', ')}]`;
      lines.push(`  ${exp.name}(${params}): ${ret};`);
    }
    lines.push('}');
  }

  lines.push('');
  return lines.join('\n');
}
