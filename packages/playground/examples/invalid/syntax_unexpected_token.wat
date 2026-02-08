;; Invalid: syntax error - unexpected token
;; "foobar" is not a valid instruction

(module
  (func (export "test") (result i32)
    (foobar 42)))
