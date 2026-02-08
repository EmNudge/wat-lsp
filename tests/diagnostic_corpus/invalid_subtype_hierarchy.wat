(module
  (type $parent (sub (struct (field i32) (field i64))))
  (type $child (sub $parent (struct (field i32))))
)
