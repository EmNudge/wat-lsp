;; Legal subtype relations for ref.null results:
;;  - nullref (ref.null none) <: structref, eqref, anyref (nullable)
;;  - a nullable concrete struct ref <: structref/eqref/anyref
(module
  (type $t (struct (field i32)))
  (func (result anyref)
    ref.null none)
  (func (result eqref)
    ref.null none)
  (func (result structref)
    ref.null $t))
