;; br_table labels whose result types are related by reference subtyping share a
;; common subtype and are valid. Here the branched value is `structref`, which is
;; a subtype of both the `structref` label and the `eqref` label, so no
;; "inconsistent types" diagnostic should be produced. (Confirmed valid by
;; wasm-tools validate --features all.)
(module
  (func $f (param $r structref)
    (block $a (result eqref)
      (block $b (result structref)
        local.get $r
        i32.const 0
        br_table $b $a
      )
      drop
      unreachable
    )
    drop
  )
  ;; reverse ordering: default label is the wider `eqref`, target is `structref`
  (func $g (param $r structref)
    (block $a (result structref)
      (block $b (result eqref)
        local.get $r
        i32.const 0
        br_table $a $b
      )
      drop
      unreachable
    )
    drop
  )
)
