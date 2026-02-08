;; Return type mismatch: function declares i64 but returns i32
;; Expected error: Type mismatch: function returns i32 but declared as i64

(module
  (func $returns_wrong_type (result i64)
    i32.const 42  ;; ERROR: returns i32 but function declares i64
  )
)
