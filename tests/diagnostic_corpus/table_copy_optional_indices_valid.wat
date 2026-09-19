;; table.copy takes a jointly-optional pair of table indices: supply zero (both
;; default to 0) or two. Regression: the zero-immediate form is the canonical
;; spelling and must not be flagged as an arity error.
(module
  (table 1 funcref)
  (func $copy_default
    i32.const 0
    i32.const 0
    i32.const 0
    table.copy)
  (func $copy_explicit
    i32.const 0
    i32.const 0
    i32.const 0
    table.copy 0 0))
