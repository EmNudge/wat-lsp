// WAT Language Definition for Monaco Editor
// Note: Syntax highlighting is now provided by tree-sitter via the LSP's
// semantic tokens provider, not by Monarch tokenizer.
export const watLanguage = {
  id: 'wat',
  extensions: ['.wat', '.wast'],
  aliases: ['WebAssembly Text', 'WAT', 'WAST'],

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
