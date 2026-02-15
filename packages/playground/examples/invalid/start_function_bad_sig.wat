;; Invalid: start function with wrong signature
;; The start function must have type [] -> [] (no params, no results)

(module
  (func $bad_start (param i32) (result i32)
    (local.get 0))

  (start $bad_start))
