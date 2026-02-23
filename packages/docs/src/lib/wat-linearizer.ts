// Folded (S-expression) → Linear (stack) WAT format converter.
// Only transforms instruction bodies inside func forms; module structure is preserved.

// ── Token types ──

const enum TT {
  LPAREN,
  RPAREN,
  STRING,
  LINE_COMMENT,
  BLOCK_COMMENT,
  WORD,
  WHITESPACE,
}

interface Token {
  type: TT;
  value: string;
  offset: number;
}

// ── Tokenizer ──

function tokenize(src: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  while (i < src.length) {
    const ch = src[i];

    // Whitespace
    if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
      const start = i;
      while (
        i < src.length &&
        (src[i] === ' ' || src[i] === '\t' || src[i] === '\n' || src[i] === '\r')
      )
        i++;
      tokens.push({ type: TT.WHITESPACE, value: src.slice(start, i), offset: start });
      continue;
    }

    // Block comment (; ... ;)
    if (ch === '(' && src[i + 1] === ';') {
      const start = i;
      i += 2;
      let depth = 1;
      while (i < src.length && depth > 0) {
        if (src[i] === '(' && src[i + 1] === ';') {
          depth++;
          i += 2;
        } else if (src[i] === ';' && src[i + 1] === ')') {
          depth--;
          i += 2;
        } else i++;
      }
      tokens.push({ type: TT.BLOCK_COMMENT, value: src.slice(start, i), offset: start });
      continue;
    }

    // Line comment
    if (ch === ';' && src[i + 1] === ';') {
      const start = i;
      while (i < src.length && src[i] !== '\n') i++;
      tokens.push({ type: TT.LINE_COMMENT, value: src.slice(start, i), offset: start });
      continue;
    }

    // Parentheses
    if (ch === '(') {
      tokens.push({ type: TT.LPAREN, value: '(', offset: i });
      i++;
      continue;
    }
    if (ch === ')') {
      tokens.push({ type: TT.RPAREN, value: ')', offset: i });
      i++;
      continue;
    }

    // String
    if (ch === '"') {
      const start = i;
      i++;
      while (i < src.length && src[i] !== '"') {
        if (src[i] === '\\') i++;
        i++;
      }
      i++; // closing quote
      tokens.push({ type: TT.STRING, value: src.slice(start, i), offset: start });
      continue;
    }

    // Word (anything else non-whitespace, non-paren, non-string)
    const start = i;
    while (
      i < src.length &&
      src[i] !== ' ' &&
      src[i] !== '\t' &&
      src[i] !== '\n' &&
      src[i] !== '\r' &&
      src[i] !== '(' &&
      src[i] !== ')' &&
      src[i] !== '"' &&
      !(src[i] === ';' && src[i + 1] === ';') &&
      !(src[i] === '(' && src[i + 1] === ';')
    ) {
      i++;
    }
    if (i > start) {
      tokens.push({ type: TT.WORD, value: src.slice(start, i), offset: start });
    }
  }
  return tokens;
}

// ── S-expression AST ──

export type SExpr =
  | { type: 'list'; children: SExpr[]; head?: string }
  | { type: 'atom'; value: string }
  | { type: 'string'; value: string }
  | { type: 'comment'; value: string; block: boolean };

function parseSExprs(tokens: Token[]): SExpr[] {
  let pos = 0;

  function parseOne(): SExpr | null {
    while (pos < tokens.length) {
      const tok = tokens[pos];
      if (tok.type === TT.WHITESPACE) {
        pos++;
        continue;
      }

      if (tok.type === TT.LINE_COMMENT) {
        pos++;
        return { type: 'comment', value: tok.value, block: false };
      }
      if (tok.type === TT.BLOCK_COMMENT) {
        pos++;
        return { type: 'comment', value: tok.value, block: true };
      }
      if (tok.type === TT.STRING) {
        pos++;
        return { type: 'string', value: tok.value };
      }
      if (tok.type === TT.WORD) {
        pos++;
        return { type: 'atom', value: tok.value };
      }
      if (tok.type === TT.RPAREN) {
        return null; // end of list
      }
      if (tok.type === TT.LPAREN) {
        pos++; // consume '('
        const children: SExpr[] = [];
        let head: string | undefined;
        while (pos < tokens.length) {
          const t = tokens[pos];
          if (t.type === TT.WHITESPACE) {
            pos++;
            continue;
          }
          if (t.type === TT.RPAREN) {
            pos++;
            break;
          }
          const child = parseOne();
          if (child === null) break;
          if (head === undefined && child.type === 'atom') {
            head = child.value;
          }
          children.push(child);
        }
        return { type: 'list', children, head };
      }
      pos++;
    }
    return null;
  }

  const results: SExpr[] = [];
  while (pos < tokens.length) {
    const expr = parseOne();
    if (expr) results.push(expr);
  }
  return results;
}

// ── Instruction classification ──

const DECLARATION_HEADS = new Set([
  'module',
  'func',
  'param',
  'result',
  'local',
  'global',
  'table',
  'memory',
  'type',
  'import',
  'export',
  'data',
  'elem',
  'start',
  'then',
  'else',
  'mut',
  'offset',
  'item',
  'declare',
  'field',
  'rec',
  'sub',
  'final',
  'tag',
]);

const BARE_INSTRUCTION_KEYWORDS = new Set([
  'block',
  'loop',
  'if',
  'br',
  'br_if',
  'br_table',
  'call',
  'call_indirect',
  'return',
  'drop',
  'select',
  'unreachable',
  'nop',
  'return_call',
  'return_call_indirect',
  'return_call_ref',
  'call_ref',
  'throw',
  'rethrow',
  'try_table',
  'throw_ref',
]);

function isInstruction(head: string): boolean {
  if (DECLARATION_HEADS.has(head)) return false;
  if (BARE_INSTRUCTION_KEYWORDS.has(head)) return true;
  // Dot-prefixed instructions: i32.add, local.get, memory.grow, etc.
  if (head.includes('.')) return true;
  return false;
}

// ── Linearizer ──

function exprToString(expr: SExpr): string {
  if (expr.type === 'atom') return expr.value;
  if (expr.type === 'string') return expr.value;
  if (expr.type === 'comment') return expr.value;
  if (expr.type === 'list') {
    return '(' + expr.children.map(exprToString).join(' ') + ')';
  }
  return '';
}

interface LinearLine {
  text: string;
  indent: number;
}

function linearizeBody(exprs: SExpr[], baseIndent: number): LinearLine[] {
  const lines: LinearLine[] = [];

  for (const expr of exprs) {
    if (expr.type === 'comment') {
      lines.push({ text: expr.value, indent: baseIndent });
      continue;
    }

    if (expr.type === 'atom' || expr.type === 'string') {
      // Bare atom in instruction body — pass through
      lines.push({ text: expr.value, indent: baseIndent });
      continue;
    }

    if (expr.type !== 'list' || !expr.head) {
      lines.push({ text: exprToString(expr), indent: baseIndent });
      continue;
    }

    const head = expr.head;

    // Non-instruction list — emit verbatim (e.g., (param ...), (result ...), (local ...))
    if (!isInstruction(head)) {
      lines.push({ text: exprToString(expr), indent: baseIndent });
      continue;
    }

    // Control: block / loop
    if (head === 'block' || head === 'loop') {
      linearizeBlock(expr, head, baseIndent, lines);
      continue;
    }

    // Control: if
    if (head === 'if') {
      linearizeIf(expr, baseIndent, lines);
      continue;
    }

    // Regular instruction: linearize nested args first, then emit instruction
    linearizeInstruction(expr, baseIndent, lines);
  }

  return lines;
}

function linearizeBlock(
  expr: SExpr & { type: 'list' },
  kind: string,
  indent: number,
  lines: LinearLine[],
) {
  const children = expr.children;
  // children[0] is the head atom (block/loop)
  // Collect header parts: label, type annotations
  const headerParts: string[] = [kind];
  let bodyStart = 1;
  let label: string | undefined;

  for (let i = 1; i < children.length; i++) {
    const c = children[i];
    if (c.type === 'atom' && c.value.startsWith('$')) {
      headerParts.push(c.value);
      label = c.value;
      bodyStart = i + 1;
    } else if (
      c.type === 'list' &&
      (c.head === 'result' || c.head === 'type' || c.head === 'param')
    ) {
      headerParts.push(exprToString(c));
      bodyStart = i + 1;
    } else {
      bodyStart = i;
      break;
    }
  }

  lines.push({ text: headerParts.join(' '), indent });

  const bodyExprs = children.slice(bodyStart);
  const bodyLines = linearizeBody(bodyExprs, indent + 2);
  lines.push(...bodyLines);

  lines.push({ text: label ? `end ${label}` : 'end', indent });
}

function linearizeIf(expr: SExpr & { type: 'list' }, indent: number, lines: LinearLine[]) {
  const children = expr.children;
  // children[0] = 'if' atom
  // Collect: optional label, optional (result ...), condition, (then ...), optional (else ...)
  const headerParts: string[] = ['if'];
  let idx = 1;
  let label: string | undefined;

  // Label
  const maybeLabel = children[idx];
  if (maybeLabel && maybeLabel.type === 'atom' && maybeLabel.value.startsWith('$')) {
    label = maybeLabel.value;
    headerParts.push(label);
    idx++;
  }

  // Result / type annotations
  while (idx < children.length) {
    const c = children[idx];
    if (c.type === 'list' && (c.head === 'result' || c.head === 'type' || c.head === 'param')) {
      headerParts.push(exprToString(c));
      idx++;
    } else {
      break;
    }
  }

  // Condition: everything before (then ...) that is an instruction
  const condExprs: SExpr[] = [];
  while (idx < children.length) {
    const c = children[idx];
    if (c.type === 'list' && (c.head === 'then' || c.head === 'else')) break;
    condExprs.push(c);
    idx++;
  }

  // Emit condition instructions first
  const condLines = linearizeBody(condExprs, indent);
  lines.push(...condLines);

  // Emit if header
  lines.push({ text: headerParts.join(' '), indent });

  // Then branch
  const maybeThen = children[idx];
  if (maybeThen && maybeThen.type === 'list' && maybeThen.head === 'then') {
    const thenBody = maybeThen.children.slice(1); // skip 'then' atom
    const thenLines = linearizeBody(thenBody, indent + 2);
    lines.push(...thenLines);
    idx++;
  }

  // Else branch
  const maybeElse = children[idx];
  if (maybeElse && maybeElse.type === 'list' && maybeElse.head === 'else') {
    lines.push({ text: 'else', indent });
    const elseBody = maybeElse.children.slice(1); // skip 'else' atom
    const elseLines = linearizeBody(elseBody, indent + 2);
    lines.push(...elseLines);
    idx++;
  }

  lines.push({ text: label ? `end ${label}` : 'end', indent });
}

function linearizeInstruction(expr: SExpr & { type: 'list' }, indent: number, lines: LinearLine[]) {
  const children = expr.children;
  // children[0] = instruction name atom
  const instrParts: string[] = [(children[0] as { type: 'atom'; value: string }).value];
  const nestedArgs: SExpr[] = [];

  for (let i = 1; i < children.length; i++) {
    const c = children[i];
    if (c.type === 'comment') {
      // Comments in arg position: emit before instruction
      nestedArgs.push(c);
    } else if (c.type === 'atom') {
      // Immediate argument (label, offset=N, align=N, numeric literal)
      instrParts.push(c.value);
    } else if (c.type === 'string') {
      instrParts.push(c.value);
    } else if (c.type === 'list' && c.head && isInstruction(c.head)) {
      // Nested instruction — recursively linearize as argument
      nestedArgs.push(c);
    } else if (c.type === 'list' && c.head && !isInstruction(c.head)) {
      // Non-instruction list in arg position (e.g., (type $t))
      instrParts.push(exprToString(c));
    } else {
      instrParts.push(exprToString(c));
    }
  }

  // Emit nested argument instructions first (stack order)
  for (const arg of nestedArgs) {
    if (arg.type === 'comment') {
      lines.push({ text: arg.value, indent });
    } else {
      const argLines = linearizeBody([arg], indent);
      lines.push(...argLines);
    }
  }

  // Emit the instruction itself
  lines.push({ text: instrParts.join(' '), indent });
}

function linearizeFunc(funcExpr: SExpr & { type: 'list' }): string {
  const children = funcExpr.children;
  // children[0] = 'func' atom
  // Collect header: func name, (export ...), (import ...), (type ...), (param ...), (result ...), (local ...)
  const headerParts: string[] = [];
  const localDecls: string[] = [];
  let bodyStart = 1;

  for (let i = 1; i < children.length; i++) {
    const c = children[i];
    if (c.type === 'atom' && c.value.startsWith('$') && headerParts.length === 0) {
      // Function name
      headerParts.push(c.value);
      bodyStart = i + 1;
    } else if (
      c.type === 'list' &&
      c.head &&
      (c.head === 'export' ||
        c.head === 'import' ||
        c.head === 'type' ||
        c.head === 'param' ||
        c.head === 'result')
    ) {
      headerParts.push(exprToString(c));
      bodyStart = i + 1;
    } else if (c.type === 'list' && c.head === 'local') {
      localDecls.push(exprToString(c));
      bodyStart = i + 1;
    } else {
      bodyStart = i;
      break;
    }
  }

  // Detect indent from original source structure — use 4 spaces for func body
  const funcHeader = '(func' + (headerParts.length > 0 ? ' ' + headerParts.join(' ') : '');
  const indent = 4;

  const resultLines: string[] = [];
  resultLines.push('  ' + funcHeader);

  // Local declarations
  for (const ld of localDecls) {
    resultLines.push(' '.repeat(indent) + ld);
  }

  // Body instructions
  const bodyExprs = children.slice(bodyStart);
  const bodyLines = linearizeBody(bodyExprs, indent);
  for (const bl of bodyLines) {
    resultLines.push(' '.repeat(bl.indent) + bl.text);
  }

  resultLines.push('  )');

  return resultLines.join('\n');
}

function hasFoldedInstructions(funcExpr: SExpr & { type: 'list' }): boolean {
  const children = funcExpr.children;
  for (let i = 1; i < children.length; i++) {
    const c = children[i];
    if (c.type === 'list' && c.head && isInstruction(c.head)) {
      // Found an instruction form — this is already folded style
      return true;
    }
  }
  return false;
}

/**
 * Convert folded (S-expression) WAT to linear (stack) WAT format.
 * Only transforms instruction bodies inside `(func ...)` forms.
 * Module structure, types, imports, exports, etc. are preserved.
 */
export function foldedToLinear(source: string): string {
  const tokens = tokenize(source);
  const exprs = parseSExprs(tokens);

  if (exprs.length === 0) return source;

  // Process the top-level expression (usually a module)
  return processTopLevel(exprs, source);
}

function processTopLevel(exprs: SExpr[], _source: string): string {
  const lines: string[] = [];

  for (const expr of exprs) {
    if (expr.type === 'comment') {
      lines.push(expr.value);
      continue;
    }
    if (expr.type !== 'list') {
      lines.push(exprToString(expr));
      continue;
    }

    if (expr.head === 'module') {
      lines.push(linearizeModule(expr));
    } else if (expr.head === 'func') {
      if (hasFoldedInstructions(expr)) {
        lines.push(linearizeFunc(expr));
      } else {
        lines.push(exprToString(expr));
      }
    } else {
      lines.push(exprToString(expr));
    }
  }

  return lines.join('\n');
}

function linearizeModule(moduleExpr: SExpr & { type: 'list' }): string {
  const children = moduleExpr.children;
  const resultLines: string[] = [];

  // Module header: (module or (module $name
  let moduleHeader = '(module';
  if (children.length > 1 && children[1].type === 'atom' && children[1].value.startsWith('$')) {
    moduleHeader += ' ' + children[1].value;
  }
  resultLines.push(moduleHeader);

  const startIdx = children[1]?.type === 'atom' && children[1].value.startsWith('$') ? 2 : 1;

  for (let i = startIdx; i < children.length; i++) {
    const child = children[i];

    if (child.type === 'comment') {
      resultLines.push('  ' + child.value);
      continue;
    }

    if (child.type === 'list' && child.head === 'func' && hasFoldedInstructions(child)) {
      resultLines.push(linearizeFunc(child));
      continue;
    }

    // Non-func or non-folded: preserve as-is with 2-space indent
    resultLines.push('  ' + exprToString(child));
  }

  resultLines.push(')');
  return resultLines.join('\n');
}
