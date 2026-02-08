;; Stack underflow: local.set with empty stack
;; Expected error: Stack underflow: 'local.set' requires 1 value but only 0 available on stack

(module
  (func $empty_local_set (local $x i32)
    local.set $x  ;; ERROR: needs 1 value to store, stack is empty
  )
)
