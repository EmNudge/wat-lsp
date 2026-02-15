;; Invalid: bulk operation with out-of-range memory/table index
;; memory.fill and table.fill reference non-existent indices

(module
  (memory 1)
  (table 1 funcref)

  (func
    ;; Error: unknown memory 5
    (memory.fill 5 (i32.const 0) (i32.const 0) (i32.const 0))

    ;; Error: unknown table 3
    (table.fill 3 (i32.const 0) (ref.null func) (i32.const 0))))
