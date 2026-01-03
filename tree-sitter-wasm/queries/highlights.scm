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
] @type.builtin

; Control flow keywords
[
  "block"
  "loop"
  "if"
  "then"
  "else"
  "end"
  "br_table"
  "call_indirect"
  "try"
  "catch"
  "catch_all"
  "throw"
  "rethrow"
  "delegate"
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
  "offset"
  "align"
  "item"
  "declare"
  "rec"
  "field"
  "struct"
  "array"
] @keyword

; Instructions - these contain the actual instruction text like "local.get", "i32.add"
; The semantic tokens provider will split these into namespace.action
(op_nullary) @function.instruction
(op_index) @function.instruction
(op_index_opt) @function.instruction
(op_index_opt_offset_opt_align_opt) @function.instruction
(op_const) @function.instruction
(op_select) @function.instruction
(op_simd_const) @function.instruction
(op_simd_lane) @function.instruction
(op_simd_offset_opt_align_opt) @function.instruction
(op_table_copy) @function.instruction
(op_table_init) @function.instruction
(op_func_bind) @function.instruction
(op_let) @function.instruction

; Special parametric instruction keyword
"select" @function.instruction

; GC proposal instructions
[
  "struct.new"
  "struct.new_default"
  "struct.get"
  "struct.get_s"
  "struct.get_u"
  "struct.set"
  "array.new"
  "array.new_default"
  "array.new_fixed"
  "array.new_data"
  "array.new_elem"
  "array.get"
  "array.get_s"
  "array.get_u"
  "array.set"
  "array.len"
  "array.fill"
  "array.copy"
  "array.init_data"
  "array.init_elem"
  "ref.test"
  "ref.cast"
  "ref.cast_null"
  "ref.i31"
  "i31.get_s"
  "i31.get_u"
  "br_on_cast"
  "br_on_cast_fail"
] @function.instruction

; Bulk memory instructions
[
  "memory.init"
  "memory.copy"
  "memory.fill"
  "data.drop"
  "elem.drop"
] @function.instruction

; Reference type instructions
[
  "ref.null"
  "ref.func"
  "ref.extern"
  "table.get"
  "table.set"
  "table.size"
  "table.grow"
  "table.fill"
] @function.instruction

; Exception handling instructions
[
  "throw_ref"
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
