;; Three memory indices are invalid for memory.copy: it takes zero or two. The
;; full pipeline must reject the surplus immediate.
(module
  (memory 1)
  (func
    i32.const 0
    i32.const 0
    i32.const 0
    memory.copy 0 0 0))
