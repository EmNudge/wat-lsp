;; Stack underflow: drop with empty stack
;; Expected error: Stack underflow: 'drop' requires 1 value but only 0 available on stack

(module
  (func $empty_drop
    drop  ;; ERROR: needs 1 value, stack is empty
  )
)
