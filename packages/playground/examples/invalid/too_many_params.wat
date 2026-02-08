;; Invalid: too many parameters to i32.add
;; i32.add takes exactly 2 operands, not 3

(module
  (func (export "test") (result i32)
    (i32.add
      (i32.const 1)
      (i32.const 2)
      (i32.const 3))))
