(module
  (type $parent (sub (struct (field (mut i32)))))
  (type $child (sub $parent (struct (field (mut i64)))))
)
