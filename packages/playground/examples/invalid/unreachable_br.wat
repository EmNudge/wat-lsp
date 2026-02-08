;; Invalid: br_if with undefined label
;; Demonstrates branch instruction with bad reference

(module
  (func (export "test") (result i32)
    (block $outer (result i32)
      (br_if $inner (i32.const 1))
      (i32.const 0))))
