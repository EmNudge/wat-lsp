;; Stack underflow: select with insufficient values
;; Expected error: Stack underflow: 'select' requires 3 values but only 2 available on stack

(module
  (func $partial_select (result i32)
    i32.const 10
    i32.const 20
    select  ;; ERROR: needs 3 values (val1, val2, condition), only 2 on stack
  )
)
