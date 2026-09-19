;; table.init requires at least the elem index; the zero-immediate form is
;; invalid and the pipeline must reject the missing required index.
(module
  (table 1 funcref)
  (func $f)
  (elem $e func $f)
  (func
    i32.const 0
    i32.const 0
    i32.const 0
    table.init))
