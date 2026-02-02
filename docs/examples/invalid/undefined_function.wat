;; Invalid: undefined function reference
;; The function $nonexistent is never defined

(module
  (func (export "caller") (result i32)
    (call $nonexistent)))
