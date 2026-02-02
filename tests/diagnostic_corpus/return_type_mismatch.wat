;; Return type mismatch: function declares i64 but returns i32
(module
  (func $returns_wrong (result i64)
    (i32.const 42)))
