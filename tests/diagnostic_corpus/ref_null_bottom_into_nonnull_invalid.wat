;; ref.null none is the nullable bottom type (nullref); it cannot satisfy
;; a non-nullable (ref struct) result even though it is a struct subtype.
(module
  (func (result (ref struct))
    ref.null none))
