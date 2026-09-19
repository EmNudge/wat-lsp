;; br_on_cast between unrelated reference hierarchies is invalid: funcref and
;; (ref extern) are not in the same hierarchy, so a funcref value cannot be cast
;; to (ref extern) nor carried to a label expecting it.
;; (Confirmed invalid by wasm-tools validate --features all.)
(module
  (func $f (param $x funcref) (result funcref)
    (block $b (result (ref extern))
      local.get $x
      br_on_cast $b funcref (ref extern)
    )
    drop
    local.get $x
  )
)
