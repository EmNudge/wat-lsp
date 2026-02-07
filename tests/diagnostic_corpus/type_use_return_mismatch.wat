;; Function using (type $t) should still validate return types
(module
  (type $t (func (result i64)))
  (func $f (type $t)
    (i32.const 42)))
