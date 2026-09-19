;; br_table targets with unrelated result types have no common subtype and are
;; invalid. `funcref` and `externref` live in different reference hierarchies, so
;; a value cannot be a subtype of both. (Confirmed invalid by
;; wasm-tools validate --features all.)
(module
  (func $f
    (block $a (result externref)
      (block $b (result funcref)
        ref.null func
        i32.const 0
        br_table $b $a
      )
      drop
      unreachable
    )
    drop
  )
)
