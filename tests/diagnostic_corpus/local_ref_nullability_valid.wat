;; A non-nullable concrete reference `(ref $s)` is a subtype of the nullable
;; `(ref null $s)`, so storing the parameter into the nullable local is valid.
;; This pins that parameters and locals keep their resolved concrete-reference
;; identity AND nullability through the value stack (confirmed valid by
;; `wasm-tools validate --features all`). Must produce NO diagnostics.
(module
  (type $s (struct))
  (func (param (ref $s))
    (local (ref null $s))
    local.get 0
    local.set 1
  )
)
