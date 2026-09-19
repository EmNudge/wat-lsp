;; memory.init requires at least the data index; the zero-immediate form is
;; invalid and the pipeline must reject the missing required index.
(module
  (memory 1)
  (data $d "abc")
  (func
    i32.const 0
    i32.const 0
    i32.const 0
    memory.init))
