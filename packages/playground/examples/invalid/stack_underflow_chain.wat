;; Stack underflow: chain of operations with underflow in the middle
;; Expected error: Stack underflow: 'i32.mul' requires 2 values but only 1 available on stack

(module
  (func $chain_underflow (result i32)
    i32.const 1
    i32.const 2
    i32.add     ;; OK: consumes 2, produces 1 (stack: 1)
    i32.mul     ;; ERROR: needs 2, only 1 on stack
  )
)
