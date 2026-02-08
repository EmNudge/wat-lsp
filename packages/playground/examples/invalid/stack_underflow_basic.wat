;; Stack underflow: binary operation with empty stack
;; Expected error: Stack underflow: 'i32.add' requires 2 values but only 0 available on stack

(module
  (func $empty_stack_add
    i32.add  ;; ERROR: needs 2 values, stack is empty
  )
)
