;; ref.null yields a NULLABLE reference, which cannot satisfy a
;; non-nullable result type (ref func).
(module
  (func (result (ref func))
    ref.null func))
