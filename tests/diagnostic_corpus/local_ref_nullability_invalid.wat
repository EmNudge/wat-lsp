;; A nullable concrete reference `(ref null $s)` is NOT a subtype of the
;; non-nullable `(ref $s)`, so storing the nullable parameter into the
;; non-nullable local must be flagged. Rejected by
;; `wasm-tools validate --features all`; pins that nullability is preserved
;; through the value stack rather than collapsed by a placeholder.
(module
  (type $s (struct))
  (func (param (ref null $s))
    (local (ref $s))
    local.get 0
    local.set 1
  )
)
