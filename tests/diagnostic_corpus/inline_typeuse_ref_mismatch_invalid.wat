;; Genuine inline-vs-type-reference reference mismatches must still be flagged.
;; `$f` declares `(ref $b)` inline but the type reference resolves to `(ref $a)`;
;; `$g` declares a non-nullable `(ref $a)` inline against a nullable
;; `(ref null $a)` type reference. Both are rejected by
;; `wasm-tools validate --features all` ("inline function type doesn't match
;; type reference") and must remain flagged now that reference forms are resolved
;; instead of masked by `Unknown`.
(module
  (type $a (struct))
  (type $b (struct (field i32)))
  (type $ta (func (param (ref $a)) (result i32)))
  (func $f (type $ta) (param (ref $b)) (result i32)
    i32.const 0
  )
  (type $tn (func (param (ref null $a)) (result i32)))
  (func $g (type $tn) (param (ref $a)) (result i32)
    i32.const 0
  )
)
