;; Invalid: undefined type reference
;; The type $missing_type is never declared

(module
  (table 1 funcref)
  (func (export "test") (result i32)
    (call_indirect (type $missing_type) (i32.const 0))))
