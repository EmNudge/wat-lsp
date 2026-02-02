;; Stack underflow with mixed folded and linear form
;; Expected error: Stack underflow: 'i32.add' requires 2 values but only 1 available on stack

(module
  (func $partial_stack_add (result i32)
    i32.const 42
    (i32.add)  ;; ERROR: needs 2 values, only 1 on stack
  )
)
