/**
 * Generates JavaScript host code for a compiled WASM module.
 * The generated code assumes `wasmBytes` is a global (injected by the worker).
 */

import type { WatModuleInfo, WatImport, WatExport, WatValueType } from './watTypeExtractor';

/** Check if a string is a valid JS identifier */
function isValidJsIdentifier(name: string): boolean {
  return /^[a-zA-Z_$][a-zA-Z0-9_$]*$/.test(name);
}

/** Get a safe accessor for a name (dot notation or bracket notation) */
function propAccess(name: string): string {
  return isValidJsIdentifier(name) ? `.${name}` : `["${name}"]`;
}

/** Get default value for a WAT type */
function defaultValue(type: WatValueType): string {
  switch (type) {
    case 'i32': return '0';
    case 'i64': return '0n';
    case 'f32': return '0.0';
    case 'f64': return '0.0';
    case 'v128': return '0';
    case 'funcref': return 'null';
    case 'externref': return 'null';
    default: return '0';
  }
}

/** Get placeholder argument for an example function call */
function placeholderArg(type: WatValueType, index: number): string {
  switch (type) {
    case 'i32': return String(index + 1);
    case 'i64': return `${index + 1}n`;
    case 'f32': return `${(index + 1)}.0`;
    case 'f64': return `${(index + 1)}.0`;
    default: return String(index);
  }
}

/** Generate a function stub for an imported function */
function generateFuncStub(imp: WatImport): string {
  const sig = imp.signature;
  if (!sig) return '() => {}';

  const paramNames = sig.params.map((p, i) => p.name?.replace('$', '') ?? `p${i}`);
  const paramList = paramNames.join(', ');

  // Smart defaults
  if (sig.results.length === 0) {
    if (paramNames.length > 0) {
      return `(${paramList}) => console.log(${paramNames.join(', ')})`;
    }
    return '() => {}';
  }

  if (paramNames.length === 0 && sig.results.length > 0) {
    return `() => ${defaultValue(sig.results[0])}`;
  }

  return `(${paramList}) => ${defaultValue(sig.results[0])}`;
}

/** Generate import object entries grouped by module */
function generateImportObject(imports: WatImport[]): string {
  if (imports.length === 0) return '';

  // Group by module
  const modules = new Map<string, WatImport[]>();
  for (const imp of imports) {
    const list = modules.get(imp.module) ?? [];
    list.push(imp);
    modules.set(imp.module, list);
  }

  const lines: string[] = ['const importObject = {'];
  const moduleEntries = [...modules.entries()];
  for (let mi = 0; mi < moduleEntries.length; mi++) {
    const [mod, imps] = moduleEntries[mi];
    const modAccessor = isValidJsIdentifier(mod) ? mod : `"${mod}"`;
    lines.push(`  ${modAccessor}: {`);

    for (let ii = 0; ii < imps.length; ii++) {
      const imp = imps[ii];
      const nameAccessor = isValidJsIdentifier(imp.name) ? imp.name : `"${imp.name}"`;
      let value: string;

      switch (imp.kind) {
        case 'function':
          value = generateFuncStub(imp);
          break;
        case 'memory': {
          const parts = [`initial: ${imp.memoryInitial ?? 1}`];
          if (imp.memoryMaximum !== undefined) parts.push(`maximum: ${imp.memoryMaximum}`);
          value = `new WebAssembly.Memory({ ${parts.join(', ')} })`;
          break;
        }
        case 'global': {
          const valType = imp.globalType ?? 'i32';
          const mut = imp.globalMutable ?? false;
          value = `new WebAssembly.Global({ value: "${valType}", mutable: ${mut} }, ${defaultValue(valType)})`;
          break;
        }
        case 'table': {
          const parts = [`element: "${imp.tableElement ?? 'anyfunc'}"`, `initial: ${imp.tableInitial ?? 0}`];
          if (imp.tableMaximum !== undefined) parts.push(`maximum: ${imp.tableMaximum}`);
          value = `new WebAssembly.Table({ ${parts.join(', ')} })`;
          break;
        }
        case 'tag': {
          const params = imp.tagParams ?? [];
          const paramStrs = params.map(t => `"${t}"`);
          value = `new WebAssembly.Tag({ parameters: [${paramStrs.join(', ')}] })`;
          break;
        }
        default:
          value = 'undefined';
      }

      const comma = ii < imps.length - 1 ? ',' : ',';
      lines.push(`    ${nameAccessor}: ${value}${comma}`);
    }

    const modComma = mi < moduleEntries.length - 1 ? ',' : ',';
    lines.push(`  }${modComma}`);
  }

  lines.push('};');
  return lines.join('\n');
}

/** Generate example usage code for exports */
function generateExportUsage(exports: WatExport[]): string {
  const lines: string[] = [];
  const funcExports = exports.filter(e => e.kind === 'function');
  const memExports = exports.filter(e => e.kind === 'memory');
  const globalExports = exports.filter(e => e.kind === 'global');
  const tableExports = exports.filter(e => e.kind === 'table');

  // Destructure function exports (cast to any to avoid WebAssembly.ExportValue type errors)
  if (funcExports.length > 0) {
    const validNames = funcExports.filter(e => isValidJsIdentifier(e.name));
    if (validNames.length > 0) {
      lines.push(`/** @type {WasmExports} */`);
      lines.push(`const { ${validNames.map(e => e.name).join(', ')} } = /** @type {any} */ (instance.exports);`);
    }
  }

  // Memory exports
  for (const exp of memExports) {
    const accessor = `instance.exports${propAccess(exp.name)}`;
    lines.push(`const ${isValidJsIdentifier(exp.name) ? exp.name : 'mem'} = ${accessor};`);
    lines.push(`console.log("${exp.name} buffer:", new Uint8Array(${isValidJsIdentifier(exp.name) ? exp.name : 'mem'}.buffer, 0, 32));`);
  }

  // Global exports
  for (const exp of globalExports) {
    const accessor = `instance.exports${propAccess(exp.name)}`;
    lines.push(`console.log("${exp.name} =", ${accessor}.value);`);
  }

  // Table exports
  for (const exp of tableExports) {
    const accessor = `instance.exports${propAccess(exp.name)}`;
    lines.push(`const ${isValidJsIdentifier(exp.name) ? exp.name : 'tbl'} = ${accessor};`);
  }

  if (lines.length > 0) lines.push('');

  // Generate example function calls
  for (const exp of funcExports) {
    if (!exp.signature) {
      // No signature info — just log the export
      const accessor = isValidJsIdentifier(exp.name) ? exp.name : `instance.exports${propAccess(exp.name)}`;
      lines.push(`console.log("${exp.name}() =", ${accessor}());`);
      continue;
    }

    const sig = exp.signature;
    const args = sig.params.map((p, i) => placeholderArg(p.type, i));
    const accessor = isValidJsIdentifier(exp.name) ? exp.name : `instance.exports${propAccess(exp.name)}`;

    if (sig.results.length > 0) {
      const argsStr = args.length > 0 ? args.join(', ') : '';
      lines.push(`console.log("${exp.name}(${args.join(', ')}) =", ${accessor}(${argsStr}));`);
    } else {
      const argsStr = args.length > 0 ? args.join(', ') : '';
      lines.push(`${accessor}(${argsStr});`);
      lines.push(`console.log("${exp.name} called");`);
    }
  }

  return lines.join('\n');
}

/**
 * Generate complete JavaScript host code for a WASM module.
 */
export function generateHostCode(moduleInfo: WatModuleInfo, filename: string): string {
  const lines: string[] = [];

  lines.push(`// Host code for ${filename}`);
  lines.push(`// wasmBytes is provided by the runtime`);
  lines.push('');

  // Import object
  const importObj = generateImportObject(moduleInfo.imports);
  if (importObj) {
    lines.push(importObj);
    lines.push('');
  }

  // Instantiation
  lines.push('const module = await WebAssembly.compile(wasmBytes);');
  if (moduleInfo.imports.length > 0) {
    lines.push('const instance = await WebAssembly.instantiate(module, importObject);');
  } else {
    lines.push('const instance = await WebAssembly.instantiate(module);');
  }
  lines.push('');

  // Export usage
  const usage = generateExportUsage(moduleInfo.exports);
  if (usage) {
    lines.push(usage);
  } else {
    lines.push('console.log("Module instantiated successfully");');
    lines.push('console.log("Exports:", Object.keys(instance.exports));');
  }

  return lines.join('\n') + '\n';
}
