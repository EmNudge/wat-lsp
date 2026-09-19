;; table.init requires the elem index; the table index is optional and defaults
;; to 0. One or two immediates are both valid. Regression: the one-immediate
;; form (elem index only) must not be flagged.
(module
  (table 1 funcref)
  (func $f)
  (elem $e func $f)
  (func $init_default
    i32.const 0
    i32.const 0
    i32.const 0
    table.init $e)
  (func $init_explicit
    i32.const 0
    i32.const 0
    i32.const 0
    table.init 0 $e))
