;; Invalid: undefined global variable reference
;; The global $missing_global is never declared

(module
  (func (export "test") (result i32)
    (global.get $missing_global)))
