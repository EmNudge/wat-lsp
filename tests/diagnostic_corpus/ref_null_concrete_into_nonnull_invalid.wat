;; ref.null $t is a nullable concrete reference (ref null $t);
;; it cannot satisfy a non-nullable (ref $t) result.
(module
  (type $t (struct (field i32)))
  (func (result (ref $t))
    ref.null $t))
