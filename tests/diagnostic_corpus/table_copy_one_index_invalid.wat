;; A single table index is invalid for table.copy: it takes zero or two, never
;; one. The full pipeline must reject the one-immediate form.
(module
  (table 1 funcref)
  (func
    i32.const 0
    i32.const 0
    i32.const 0
    table.copy 0))
