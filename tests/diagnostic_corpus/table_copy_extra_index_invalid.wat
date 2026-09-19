;; Three table indices are invalid for table.copy: it takes zero or two. The
;; full pipeline must reject the surplus immediate.
(module
  (table 1 funcref)
  (func
    i32.const 0
    i32.const 0
    i32.const 0
    table.copy 0 0 0))
