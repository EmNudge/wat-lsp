; Comments
(comment_line) @comment
(comment_block) @comment

; Strings
(string) @string

; Numbers
(nat) @number
(int) @number
(float) @number
(dec_nat) @number
(hex_nat) @number
(dec_float) @number
(hex_float) @number
(align_offset_value) @number

; Types - numeric types (i32, i64, f32, f64, v128)
(num_type_i32) @type.builtin
(num_type_i64) @type.builtin
(num_type_f32) @type.builtin
(num_type_f64) @type.builtin
(num_type_v128) @type.builtin

; Types - reference types
(ref_type_funcref) @type.builtin
(ref_type_externref) @type.builtin
[
  "anyref"
  "eqref"
  "i31ref"
  "structref"
  "arrayref"
  "nullref"
  "nullfuncref"
  "nullexternref"
  "exnref"
] @type.builtin

; Control flow keywords
[
  "block"
  "loop"
  "if"
  "then"
  "else"
  "end"
  "br"
  "br_if"
  "br_table"
  "return"
  "call"
  "call_indirect"
  "try"
  "catch"
  "catch_all"
  "throw"
  "rethrow"
  "delegate"
  "return_call"
  "return_call_indirect"
] @keyword.control

; Module structure keywords
[
  "module"
  "import"
  "export"
  "memory"
  "data"
  "table"
  "elem"
  "start"
  "func"
  "type"
  "param"
  "result"
  "global"
  "local"
  "mut"
  "shared"
  "offset"
  "align"
  "item"
  "declare"
  "rec"
  "field"
  "struct"
  "array"
  "sub"
  "final"
] @keyword

; Instructions - these contain the actual instruction text like "local.get", "i32.add"
; The semantic tokens provider will split these into namespace.action
(op_nullary) @function.instruction
(op_index) @function.instruction
(op_const) @function.instruction
(op_select) @function.instruction
(op_simd_const) @function.instruction
(op_simd_lane) @function.instruction
(op_table_copy) @function.instruction
(op_table_init) @function.instruction
(op_br_table) @function.instruction
(op_call_indirect) @function.instruction
(op_memory) @function.instruction
(op_let) @function.instruction

; Special parametric instructions
[
  "drop"
  "select"
  "unreachable"
  "nop"
] @function.instruction

; Function identifiers (after func keyword)
(module_field_func
  identifier: (identifier) @function.definition)

; Type identifiers (after type keyword)
(module_field_type
  identifier: (identifier) @type.definition)

; Global identifiers
(module_field_global
  identifier: (identifier) @variable.definition)

; Memory identifiers
(module_field_memory
  identifier: (identifier) @variable.definition)

; Table identifiers
(module_field_table
  identifier: (identifier) @variable.definition)

; Parameter and local variable names
(func_type_params_one
  (identifier) @variable.parameter)
(func_locals_one
  (identifier) @variable.local)

; Generic identifiers (variables, labels)
(identifier) @variable

; Index references
(index
  (nat) @number)
(index
  (identifier) @variable)

; Brackets - let TextMate handle these
; ["(" ")"] @punctuation.bracket
