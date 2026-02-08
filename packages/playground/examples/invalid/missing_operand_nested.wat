;; Invalid: missing operand in nested folded expression
;; i32.sub expects 2 operands, but only 1 is provided
;; When nested inside another expression, operands cannot come from the stack

(module
  (func (export "test") (result i32)
    (i32.add
      (i32.sub (i32.const 5))  ;; Error: missing second operand
      (i32.const 3))))
