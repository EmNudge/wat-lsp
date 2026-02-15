;; struct.get_s on non-packed field type
(module
  (type $s (struct (field $f i32)))
  (func (param (ref $s))
    local.get 0
    struct.get_s $s 0
  )
)
