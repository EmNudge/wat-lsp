;; Return arity mismatch: function leaves values on stack but has no return type
;; Expected error: Type mismatch: function body leaves 1 value on stack but signature requires 0

(module
  (func $returns_extra_value
    i32.const 42  ;; ERROR: leaves value on stack but no return type declared
  )
)
