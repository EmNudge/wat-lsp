;; Invalid: syntax error - missing closing parenthesis
;; The module is missing its closing paren

(module
  (func (export "test") (result i32)
    (i32.const 42))
