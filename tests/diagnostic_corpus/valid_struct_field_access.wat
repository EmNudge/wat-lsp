;; Valid struct field access - should produce no field errors
(module
  (type $point (struct (field $x i32) (field $y i32)))
  (func (param (ref $point)) (result i32)
    local.get 0
    struct.get $point 0
  )
)
