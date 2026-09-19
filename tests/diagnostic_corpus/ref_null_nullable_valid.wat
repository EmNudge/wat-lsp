;; ref.null produces a nullable reference; storing it where a nullable
;; reference is expected is valid.
(module
  (type $t (struct (field i32)))
  (func (result funcref)
    ref.null func)
  (func (result (ref null $t))
    ref.null $t)
  (func (result structref)
    ref.null none))
