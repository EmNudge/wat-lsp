;; Return arity mismatch: function declares return type but body is empty
;; Expected error: Type mismatch: function body leaves 0 values on stack but signature requires 1

(module
  (func $missing_return_value (result i32)
    ;; ERROR: no value on stack but function declares (result i32)
  )
)
