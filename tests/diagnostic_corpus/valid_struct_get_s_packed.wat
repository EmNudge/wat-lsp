;; Valid struct.get_s on packed i8 field
(module
  (type $s (struct (field $f i8)))
  (func (param (ref $s)) (result i32)
    local.get 0
    struct.get_s $s 0
  )
)
