;; Inline function signatures that repeat a reference type exactly must match the
;; referenced type definition. Previously every reference form collapsed to the
;; recovery placeholder `Unknown`, so a valid inline signature that repeated a
;; concrete `(ref $s)` or a non-nullable `(ref func)` was falsely reported as
;; "Inline function type does not match the type reference". Both functions below
;; are valid (confirmed by `wasm-tools validate --features all`) and must produce
;; NO diagnostics.
(module
  (type $s (struct))
  (type $t1 (func (param (ref $s)) (result (ref null $s))))
  (func (type $t1) (param (ref $s)) (result (ref null $s))
    ref.null $s
  )
  (type $t2 (func (param (ref func)) (result funcref)))
  (func (type $t2) (param (ref func)) (result funcref)
    local.get 0
  )
)
