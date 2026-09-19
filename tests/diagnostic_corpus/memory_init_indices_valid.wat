;; memory.init requires the data index; the memory index is optional and
;; defaults to 0. One or two immediates are both valid. Regression: the
;; one-immediate form (data index only) must not be flagged.
(module
  (memory 1)
  (data $d "abc")
  (func $init_default
    i32.const 0
    i32.const 0
    i32.const 0
    memory.init $d)
  (func $init_explicit
    i32.const 0
    i32.const 0
    i32.const 0
    memory.init 0 $d))
