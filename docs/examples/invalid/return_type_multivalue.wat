;; Return type mismatch with multi-value: wrong types in multi-value return
;; Expected error: Type mismatch at position 1: function returns f32 but declared as i64

(module
  (func $multivalue_wrong_types (result i32 i64)
    i32.const 1   ;; OK: first return is i32
    f32.const 2.0 ;; ERROR: second return is f32 but should be i64
  )
)
