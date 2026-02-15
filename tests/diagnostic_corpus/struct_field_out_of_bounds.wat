;; Struct field index out of bounds
(module
  (type $point (struct (field $x i32) (field $y i32)))
  (func (param (ref $point))
    local.get 0
    struct.get $point 5
  )
)
