;; Stack underflow: call with insufficient arguments
;; Expected error: Stack underflow: 'call' requires 2 values but only 1 available on stack

(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )

  (func $main (result i32)
    i32.const 5
    call $add  ;; ERROR: $add needs 2 params, only 1 value on stack
  )
)
