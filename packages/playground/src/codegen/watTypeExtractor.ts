/**
 * Lightweight regex-based WAT type extractor.
 * Extracts imports, exports, and function signatures from well-formed WAT source.
 */

export type WatValueType = 'i32' | 'i64' | 'f32' | 'f64' | 'v128' | 'funcref' | 'externref';

export interface WatParam {
  name?: string;
  type: WatValueType;
}

export interface WatFuncSignature {
  params: WatParam[];
  results: WatValueType[];
}

export interface WatImport {
  module: string;
  name: string;
  kind: 'function' | 'memory' | 'global' | 'table' | 'tag';
  /** Only set for function imports */
  signature?: WatFuncSignature;
  /** Memory: initial pages */
  memoryInitial?: number;
  /** Memory: maximum pages */
  memoryMaximum?: number;
  /** Global: value type */
  globalType?: WatValueType;
  /** Global: is mutable */
  globalMutable?: boolean;
  /** Table: element type */
  tableElement?: string;
  /** Table: initial size */
  tableInitial?: number;
  /** Table: maximum size */
  tableMaximum?: number;
  /** Tag: parameter types */
  tagParams?: WatValueType[];
}

export interface WatExport {
  name: string;
  kind: 'function' | 'memory' | 'global' | 'table' | 'tag';
  /** Function signature, resolved from inline or standalone export */
  signature?: WatFuncSignature;
}

export interface WatModuleInfo {
  imports: WatImport[];
  exports: WatExport[];
}

const VALUE_TYPES = new Set(['i32', 'i64', 'f32', 'f64', 'v128', 'funcref', 'externref']);

function isValueType(s: string): s is WatValueType {
  return VALUE_TYPES.has(s);
}

/** Parse (param ...) clauses into WatParam[] */
function parseParams(text: string): WatParam[] {
  const params: WatParam[] = [];
  // Match (param $name type) or (param type type ...)
  const paramRe = /\(param\s+([^)]*)\)/g;
  let m: RegExpExecArray | null;
  while ((m = paramRe.exec(text)) !== null) {
    const inner = m[1].trim();
    const tokens = inner.split(/\s+/);
    if (tokens.length >= 2 && tokens[0].startsWith('$')) {
      // Named param: (param $name type)
      const name = tokens[0];
      for (let i = 1; i < tokens.length; i++) {
        const t = tokens[i];
        if (isValueType(t)) {
          params.push({ name, type: t });
        }
      }
    } else {
      // Anonymous params: (param type type ...)
      for (const t of tokens) {
        if (isValueType(t)) {
          params.push({ type: t });
        }
      }
    }
  }
  return params;
}

/** Parse (result ...) clauses into WatValueType[] */
function parseResults(text: string): WatValueType[] {
  const results: WatValueType[] = [];
  const resultRe = /\(result\s+([^)]*)\)/g;
  let m: RegExpExecArray | null;
  while ((m = resultRe.exec(text)) !== null) {
    const tokens = m[1].trim().split(/\s+/);
    for (const t of tokens) {
      if (isValueType(t)) {
        results.push(t);
      }
    }
  }
  return results;
}

/** Strip line/block comments from WAT source */
function stripComments(source: string): string {
  // Remove block comments (;  ;) - handle nested
  let result = source;
  let depth = 0;
  let out = '';
  let i = 0;
  while (i < result.length) {
    if (i + 1 < result.length && result[i] === '(' && result[i + 1] === ';') {
      depth++;
      i += 2;
    } else if (i + 1 < result.length && result[i] === ';' && result[i + 1] === ')') {
      depth = Math.max(0, depth - 1);
      i += 2;
    } else if (depth === 0) {
      out += result[i];
      i++;
    } else {
      i++;
    }
  }
  // Remove line comments
  return out.replace(/;;[^\n]*/g, '');
}

/** Find balanced parenthesized expression starting at open paren */
function findBalancedParen(source: string, start: number): string {
  if (source[start] !== '(') return '';
  let depth = 0;
  for (let i = start; i < source.length; i++) {
    if (source[i] === '(') depth++;
    else if (source[i] === ')') {
      depth--;
      if (depth === 0) return source.slice(start, i + 1);
    }
  }
  return source.slice(start);
}

/**
 * Extract type information from WAT source.
 */
export function extractWatTypes(source: string): WatModuleInfo {
  const clean = stripComments(source);
  const imports: WatImport[] = [];
  const exports: WatExport[] = [];
  // name→signature map for resolving standalone exports
  const funcSignatures = new Map<string, WatFuncSignature>();

  // ---- Extract imports ----
  const importRe = /\(import\s+"([^"]+)"\s+"([^"]+)"\s+/g;
  let im: RegExpExecArray | null;
  while ((im = importRe.exec(clean)) !== null) {
    const moduleName = im[1];
    const importName = im[2];
    const body = findBalancedParen(clean, im.index);

    if (/\(func\s/.test(body)) {
      const sig: WatFuncSignature = { params: parseParams(body), results: parseResults(body) };
      imports.push({ module: moduleName, name: importName, kind: 'function', signature: sig });
    } else if (/\(memory\s/.test(body)) {
      const nums = body.match(/\(memory\s+(?:\$\S+\s+)?(\d+)(?:\s+(\d+))?\)/);
      imports.push({
        module: moduleName, name: importName, kind: 'memory',
        memoryInitial: nums ? parseInt(nums[1]) : 1,
        memoryMaximum: nums?.[2] ? parseInt(nums[2]) : undefined,
      });
    } else if (/\(global\s/.test(body)) {
      const mutMatch = body.match(/\(mut\s+(\w+)\)/);
      const typeMatch = body.match(/\(global\s+(?:\$\S+\s+)?(\w+)\)/);
      if (mutMatch) {
        const t = mutMatch[1];
        imports.push({
          module: moduleName, name: importName, kind: 'global',
          globalType: isValueType(t) ? t : 'i32', globalMutable: true,
        });
      } else if (typeMatch) {
        const t = typeMatch[1];
        imports.push({
          module: moduleName, name: importName, kind: 'global',
          globalType: isValueType(t) ? t : 'i32', globalMutable: false,
        });
      } else {
        imports.push({ module: moduleName, name: importName, kind: 'global', globalType: 'i32', globalMutable: false });
      }
    } else if (/\(table\s/.test(body)) {
      const tableMatch = body.match(/\(table\s+(?:\$\S+\s+)?(\d+)(?:\s+(\d+))?\s+(\w+)\)/);
      imports.push({
        module: moduleName, name: importName, kind: 'table',
        tableInitial: tableMatch ? parseInt(tableMatch[1]) : 0,
        tableMaximum: tableMatch?.[2] ? parseInt(tableMatch[2]) : undefined,
        tableElement: tableMatch?.[3] ?? 'funcref',
      });
    } else if (/\(tag\s/.test(body)) {
      const tagParams = parseParams(body);
      imports.push({
        module: moduleName, name: importName, kind: 'tag',
        tagParams: tagParams.map(p => p.type),
      });
    }
  }

  // ---- Extract function definitions with signatures ----
  // Match (func with optional $name and optional (export "name")
  const funcRe = /\(func\s+(\$\S+)?/g;
  let fm: RegExpExecArray | null;
  while ((fm = funcRe.exec(clean)) !== null) {
    const funcBody = findBalancedParen(clean, fm.index);
    if (!funcBody) continue;
    // Skip import functions (they contain (import ...))
    if (/\(import\s/.test(funcBody)) continue;
    const funcName = fm[1]; // may be undefined
    const sig: WatFuncSignature = { params: parseParams(funcBody), results: parseResults(funcBody) };

    // Store by $name if present
    if (funcName) {
      funcSignatures.set(funcName, sig);
    }

    // Check for inline exports: (func (export "name") ...) or (func $id (export "name") ...)
    const inlineExportRe = /\(export\s+"([^"]+)"\)/g;
    let iem: RegExpExecArray | null;
    while ((iem = inlineExportRe.exec(funcBody)) !== null) {
      exports.push({ name: iem[1], kind: 'function', signature: sig });
    }
  }

  // ---- Extract memory definitions with inline exports ----
  const memRe = /\(memory\s+(?:\$\S+\s+)?\(export\s+"([^"]+)"\)/g;
  let mm: RegExpExecArray | null;
  while ((mm = memRe.exec(clean)) !== null) {
    exports.push({ name: mm[1], kind: 'memory' });
  }

  // ---- Extract standalone exports ----
  const exportRe = /\(export\s+"([^"]+)"\s+\((\w+)\s+(\$\S+)\)\)/g;
  let em: RegExpExecArray | null;
  while ((em = exportRe.exec(clean)) !== null) {
    const name = em[1];
    const kind = em[2] as 'func' | 'memory' | 'global' | 'table' | 'tag';
    const ref = em[3];

    // Skip if already added as inline export
    if (exports.some(e => e.name === name)) continue;

    const kindMap: Record<string, WatExport['kind']> = {
      func: 'function', memory: 'memory', global: 'global', table: 'table', tag: 'tag',
    };
    const exportKind = kindMap[kind] ?? 'function';

    const exp: WatExport = { name, kind: exportKind };
    if (exportKind === 'function') {
      exp.signature = funcSignatures.get(ref);
    }
    exports.push(exp);
  }

  return { imports, exports };
}
