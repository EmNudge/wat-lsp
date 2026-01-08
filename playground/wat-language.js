// WAT Language Definition for Monaco Editor
export const watLanguage = {
  id: 'wat',
  extensions: ['.wat', '.wast'],
  aliases: ['WebAssembly Text', 'WAT', 'WAST'],

  // Monarch tokenizer for syntax highlighting
  monarchTokensProvider: {
    defaultToken: '',
    tokenPostfix: '.wat',

    keywords: [
      'module', 'import', 'export', 'func', 'param', 'result', 'local',
      'global', 'memory', 'data', 'table', 'elem', 'type', 'start',
      'offset', 'mut', 'if', 'then', 'else', 'end', 'block', 'loop',
      'br', 'br_if', 'br_table', 'call', 'call_indirect', 'return',
      'drop', 'select', 'unreachable', 'nop', 'ref', 'struct', 'array',
      'field', 'tag', 'try', 'catch', 'catch_all', 'throw', 'rethrow',
      'delegate', 'rec'
    ],

    typeKeywords: [
      'i32', 'i64', 'f32', 'f64', 'v128', 'funcref', 'externref',
      'anyref', 'eqref', 'i31ref', 'structref', 'arrayref', 'nullref',
      'nullfuncref', 'nullexternref'
    ],

    operators: [
      // i32 operations
      'i32.const', 'i32.add', 'i32.sub', 'i32.mul', 'i32.div_s', 'i32.div_u',
      'i32.rem_s', 'i32.rem_u', 'i32.and', 'i32.or', 'i32.xor', 'i32.shl',
      'i32.shr_s', 'i32.shr_u', 'i32.rotl', 'i32.rotr', 'i32.clz', 'i32.ctz',
      'i32.popcnt', 'i32.eqz', 'i32.eq', 'i32.ne', 'i32.lt_s', 'i32.lt_u',
      'i32.gt_s', 'i32.gt_u', 'i32.le_s', 'i32.le_u', 'i32.ge_s', 'i32.ge_u',
      'i32.load', 'i32.load8_s', 'i32.load8_u', 'i32.load16_s', 'i32.load16_u',
      'i32.store', 'i32.store8', 'i32.store16', 'i32.wrap_i64',
      'i32.trunc_f32_s', 'i32.trunc_f32_u', 'i32.trunc_f64_s', 'i32.trunc_f64_u',
      'i32.reinterpret_f32', 'i32.extend8_s', 'i32.extend16_s',

      // i64 operations
      'i64.const', 'i64.add', 'i64.sub', 'i64.mul', 'i64.div_s', 'i64.div_u',
      'i64.rem_s', 'i64.rem_u', 'i64.and', 'i64.or', 'i64.xor', 'i64.shl',
      'i64.shr_s', 'i64.shr_u', 'i64.rotl', 'i64.rotr', 'i64.clz', 'i64.ctz',
      'i64.popcnt', 'i64.eqz', 'i64.eq', 'i64.ne', 'i64.lt_s', 'i64.lt_u',
      'i64.gt_s', 'i64.gt_u', 'i64.le_s', 'i64.le_u', 'i64.ge_s', 'i64.ge_u',
      'i64.load', 'i64.load8_s', 'i64.load8_u', 'i64.load16_s', 'i64.load16_u',
      'i64.load32_s', 'i64.load32_u', 'i64.store', 'i64.store8', 'i64.store16',
      'i64.store32', 'i64.extend_i32_s', 'i64.extend_i32_u',
      'i64.trunc_f32_s', 'i64.trunc_f32_u', 'i64.trunc_f64_s', 'i64.trunc_f64_u',
      'i64.reinterpret_f64', 'i64.extend8_s', 'i64.extend16_s', 'i64.extend32_s',

      // f32 operations
      'f32.const', 'f32.add', 'f32.sub', 'f32.mul', 'f32.div', 'f32.abs',
      'f32.neg', 'f32.ceil', 'f32.floor', 'f32.trunc', 'f32.nearest',
      'f32.sqrt', 'f32.min', 'f32.max', 'f32.copysign', 'f32.eq', 'f32.ne',
      'f32.lt', 'f32.gt', 'f32.le', 'f32.ge', 'f32.load', 'f32.store',
      'f32.convert_i32_s', 'f32.convert_i32_u', 'f32.convert_i64_s',
      'f32.convert_i64_u', 'f32.demote_f64', 'f32.reinterpret_i32',

      // f64 operations
      'f64.const', 'f64.add', 'f64.sub', 'f64.mul', 'f64.div', 'f64.abs',
      'f64.neg', 'f64.ceil', 'f64.floor', 'f64.trunc', 'f64.nearest',
      'f64.sqrt', 'f64.min', 'f64.max', 'f64.copysign', 'f64.eq', 'f64.ne',
      'f64.lt', 'f64.gt', 'f64.le', 'f64.ge', 'f64.load', 'f64.store',
      'f64.convert_i32_s', 'f64.convert_i32_u', 'f64.convert_i64_s',
      'f64.convert_i64_u', 'f64.promote_f32', 'f64.reinterpret_i64',

      // Local/global operations
      'local.get', 'local.set', 'local.tee', 'global.get', 'global.set',

      // Memory operations
      'memory.size', 'memory.grow', 'memory.init', 'memory.copy', 'memory.fill',

      // Table operations
      'table.get', 'table.set', 'table.size', 'table.grow', 'table.fill',
      'table.copy', 'table.init',

      // Reference operations
      'ref.null', 'ref.is_null', 'ref.func', 'ref.eq', 'ref.as_non_null',
      'ref.cast', 'ref.test',

      // Control flow
      'return_call', 'return_call_indirect'
    ],

    brackets: [
      ['(', ')', 'delimiter.parenthesis']
    ],

    tokenizer: {
      root: [
        // Comments
        [/;;.*$/, 'comment'],
        [/\(;/, 'comment', '@blockComment'],

        // Strings
        [/"([^"\\]|\\.)*$/, 'string.invalid'],
        [/"/, 'string', '@string'],

        // Numbers
        [/[+-]?0x[0-9a-fA-F_]+/, 'number.hex'],
        [/[+-]?\d+\.\d*([eE][+-]?\d+)?/, 'number.float'],
        [/[+-]?\d+/, 'number'],
        [/nan(:0x[0-9a-fA-F]+)?/, 'number'],
        [/[+-]?inf/, 'number'],

        // Identifiers
        [/\$[a-zA-Z_][a-zA-Z0-9_!#$%&'*+\-./:<=>?@\\^`|~]*/, 'variable'],

        // Type-prefixed operators (before keywords)
        [/[if](32|64)\.[a-z_]+/, 'keyword.operator'],
        [/v128\.[a-z_]+/, 'keyword.operator'],
        [/(local|global)\.(get|set|tee)/, 'keyword.operator'],
        [/(memory|table)\.[a-z_]+/, 'keyword.operator'],
        [/ref\.[a-z_]+/, 'keyword.operator'],

        // Keywords
        [/[a-z_][a-z0-9_]*/, {
          cases: {
            '@typeKeywords': 'type',
            '@keywords': 'keyword',
            '@default': 'identifier'
          }
        }],

        // Brackets
        [/[()]/, '@brackets'],

        // Whitespace
        [/[ \t\r\n]+/, 'white']
      ],

      blockComment: [
        [/[^(;]+/, 'comment'],
        [/;\)/, 'comment', '@pop'],
        [/[(;]/, 'comment']
      ],

      string: [
        [/[^\\"]+/, 'string'],
        [/\\./, 'string.escape'],
        [/"/, 'string', '@pop']
      ]
    }
  },

  // Language configuration for brackets, comments, etc.
  languageConfiguration: {
    comments: {
      lineComment: ';;',
      blockComment: ['(;', ';)']
    },
    brackets: [
      ['(', ')']
    ],
    autoClosingPairs: [
      { open: '(', close: ')' },
      { open: '"', close: '"', notIn: ['string'] }
    ],
    surroundingPairs: [
      { open: '(', close: ')' },
      { open: '"', close: '"' }
    ]
  },

  // Completion items
  completionItems: [
    // Module structure
    { label: 'module', kind: 'Keyword', insertText: '(module\n  $0\n)', insertTextRules: 4, documentation: 'Define a WebAssembly module' },
    { label: 'func', kind: 'Keyword', insertText: '(func $$1 (param $2) (result $3)\n  $0)', insertTextRules: 4, documentation: 'Define a function' },
    { label: 'param', kind: 'Keyword', insertText: '(param $$1 $2)', insertTextRules: 4, documentation: 'Function parameter' },
    { label: 'result', kind: 'Keyword', insertText: '(result $1)', insertTextRules: 4, documentation: 'Function result type' },
    { label: 'local', kind: 'Keyword', insertText: '(local $$1 $2)', insertTextRules: 4, documentation: 'Local variable' },
    { label: 'global', kind: 'Keyword', insertText: '(global $$1 $2 ($3))', insertTextRules: 4, documentation: 'Global variable' },
    { label: 'memory', kind: 'Keyword', insertText: '(memory $1)', insertTextRules: 4, documentation: 'Linear memory' },
    { label: 'table', kind: 'Keyword', insertText: '(table $1 funcref)', insertTextRules: 4, documentation: 'Function table' },
    { label: 'import', kind: 'Keyword', insertText: '(import "$1" "$2" ($3))', insertTextRules: 4, documentation: 'Import from host' },
    { label: 'export', kind: 'Keyword', insertText: '(export "$1" ($2))', insertTextRules: 4, documentation: 'Export to host' },
    { label: 'type', kind: 'Keyword', insertText: '(type $$1 (func (param $2) (result $3)))', insertTextRules: 4, documentation: 'Type definition' },

    // Types
    { label: 'i32', kind: 'Type', documentation: '32-bit integer' },
    { label: 'i64', kind: 'Type', documentation: '64-bit integer' },
    { label: 'f32', kind: 'Type', documentation: '32-bit float' },
    { label: 'f64', kind: 'Type', documentation: '64-bit float' },
    { label: 'funcref', kind: 'Type', documentation: 'Function reference' },
    { label: 'externref', kind: 'Type', documentation: 'External reference' },

    // Control flow
    { label: 'block', kind: 'Keyword', insertText: '(block $$1\n  $0\n)', insertTextRules: 4, documentation: 'Block construct' },
    { label: 'loop', kind: 'Keyword', insertText: '(loop $$1\n  $0\n)', insertTextRules: 4, documentation: 'Loop construct' },
    { label: 'if', kind: 'Keyword', insertText: '(if (then\n  $0\n))', insertTextRules: 4, documentation: 'Conditional' },
    { label: 'br', kind: 'Keyword', insertText: '(br $$1)', insertTextRules: 4, documentation: 'Branch' },
    { label: 'br_if', kind: 'Keyword', insertText: '(br_if $$1)', insertTextRules: 4, documentation: 'Conditional branch' },
    { label: 'call', kind: 'Keyword', insertText: '(call $$1)', insertTextRules: 4, documentation: 'Call function' },
    { label: 'return', kind: 'Keyword', documentation: 'Return from function' },

    // i32 operations
    { label: 'i32.const', kind: 'Function', insertText: '(i32.const $1)', insertTextRules: 4, documentation: 'i32 constant' },
    { label: 'i32.add', kind: 'Function', documentation: 'i32 addition' },
    { label: 'i32.sub', kind: 'Function', documentation: 'i32 subtraction' },
    { label: 'i32.mul', kind: 'Function', documentation: 'i32 multiplication' },
    { label: 'i32.div_s', kind: 'Function', documentation: 'i32 signed division' },
    { label: 'i32.div_u', kind: 'Function', documentation: 'i32 unsigned division' },
    { label: 'i32.load', kind: 'Function', documentation: 'Load i32 from memory' },
    { label: 'i32.store', kind: 'Function', documentation: 'Store i32 to memory' },
    { label: 'i32.eq', kind: 'Function', documentation: 'i32 equality' },
    { label: 'i32.eqz', kind: 'Function', documentation: 'i32 equals zero' },
    { label: 'i32.lt_s', kind: 'Function', documentation: 'i32 signed less than' },
    { label: 'i32.gt_s', kind: 'Function', documentation: 'i32 signed greater than' },

    // Local/global
    { label: 'local.get', kind: 'Function', insertText: '(local.get $$1)', insertTextRules: 4, documentation: 'Get local variable' },
    { label: 'local.set', kind: 'Function', insertText: '(local.set $$1)', insertTextRules: 4, documentation: 'Set local variable' },
    { label: 'local.tee', kind: 'Function', insertText: '(local.tee $$1)', insertTextRules: 4, documentation: 'Tee local variable' },
    { label: 'global.get', kind: 'Function', insertText: '(global.get $$1)', insertTextRules: 4, documentation: 'Get global variable' },
    { label: 'global.set', kind: 'Function', insertText: '(global.set $$1)', insertTextRules: 4, documentation: 'Set global variable' },

    // Memory
    { label: 'memory.size', kind: 'Function', documentation: 'Get memory size in pages' },
    { label: 'memory.grow', kind: 'Function', documentation: 'Grow memory by pages' }
  ]
};
