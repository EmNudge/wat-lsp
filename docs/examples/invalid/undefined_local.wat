;; Invalid: undefined local variable reference
;; The local $missing is never declared

(module
  (func (export "test") (result i32)
    (local.get $missing)))
