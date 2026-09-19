;; memory.copy takes a jointly-optional pair of memory indices: supply zero
;; (both default to 0) or two. Regression: the zero-immediate form is the
;; canonical spelling and must not be flagged.
(module
  (memory 1)
  (func $copy_default
    i32.const 0
    i32.const 0
    i32.const 0
    memory.copy)
  (func $copy_explicit
    i32.const 0
    i32.const 0
    i32.const 0
    memory.copy 0 0))
