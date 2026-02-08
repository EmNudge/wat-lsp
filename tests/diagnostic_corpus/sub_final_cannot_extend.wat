(module
  (type $sealed (sub final (struct (field i32))))
  (type $child (sub $sealed (struct (field i32) (field i64))))
)
