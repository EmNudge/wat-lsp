;; Invalid: multiple errors in one file
;; This file has both undefined references and wrong parameter counts

(module
  (func (export "bad") (result i32)
    ;; Error 1: undefined local
    (local.get $x)
    ;; Error 2: too many params to i32.const (expects 0 nested operands)
    (i32.const 1 (i32.const 2))
    ;; Error 3: undefined function
    (call $ghost)))
