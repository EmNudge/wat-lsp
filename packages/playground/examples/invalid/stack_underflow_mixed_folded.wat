;; Issue #93: Stack underflow with mixed folded and linear form
;; The folded expression (i32.add) needs 2 operands from the stack,
;; but only 1 is available from the preceding i32.const instruction.
(module
  (func $partial_stack_add (result i32)
    i32.const 42
    (i32.add)  ;; ERROR: needs 2 values, only 1 on stack
  )
)
