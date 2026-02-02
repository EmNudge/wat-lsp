;; Invalid: undefined memory reference in load instruction
;; The memory $missing is never declared (multi-memory proposal)

(module
  (memory $other 1)
  (func (export "test") (result i32)
    (i32.load $missing (i32.const 0))))
