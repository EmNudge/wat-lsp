;; Invalid: active element segment without a table
;; An active element segment requires at least one table to be defined

(module
  (func $f)

  ;; Error: unknown table 0
  (elem (offset (i32.const 0)) $f))
