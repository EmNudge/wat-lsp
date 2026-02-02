;; Return type mismatch in folded expression
;; Expected error: Type mismatch: function returns i32 but declared as f64

(module
  (func $folded_wrong_type (param $a i32) (param $b i32) (result f64)
    (i32.add      ;; ERROR: i32.add returns i32 but function declares f64
      (local.get $a)
      (local.get $b))
  )
)
