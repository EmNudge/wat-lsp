;; Invalid: undefined table in table.init
;; The table $missing is never declared

(module
  (table $other 1 funcref)
  (func (export "test")
    (table.init $missing (i32.const 0) (i32.const 0) (i32.const 0))))
