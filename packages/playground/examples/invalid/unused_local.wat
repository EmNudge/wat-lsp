;; Invalid: unused local variable
;; The local $unused is declared but never referenced

(module
  (func (export "test") (result i32)
    (local $unused i32)
    (i32.const 42)))
