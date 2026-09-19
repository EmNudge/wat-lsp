;; br_on_cast where the target ref type (rt2) is nullable while the source (rt1)
;; is non-nullable is valid. The meaningful nullability constraint is that the
;; value carried to the branch label matches the label's result type (it does
;; here); rejecting purely because rt2 is nullable and rt1 is not was a false
;; positive. (Both funcs confirmed valid by wasm-tools validate --features all.)
(module
  ;; abstract: rt1 = (ref any) non-null, rt2 = anyref nullable
  (func $abstract (param $x (ref any)) (result (ref any))
    (block $b (result anyref)
      local.get $x
      br_on_cast $b (ref any) anyref
      unreachable
    )
    unreachable
  )
  ;; concrete: rt1 = (ref $s) non-null, rt2 = (ref null $s) nullable
  (type $s (struct))
  (func $concrete (param $x (ref $s)) (result (ref $s))
    (block $b (result (ref null $s))
      local.get $x
      br_on_cast $b (ref $s) (ref null $s)
      unreachable
    )
    unreachable
  )
)
