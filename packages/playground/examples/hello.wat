;; Hello World (Add & Multiply)
;; A simple module demonstrating basic arithmetic functions

(module
  ;; Export the add function
  (func (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add
      (local.get $a)
      (local.get $b)))

  ;; Export a multiply function
  (func (export "multiply") (param $a i32) (param $b i32) (result i32)
    (i32.mul
      (local.get $a)
      (local.get $b)))

  ;; Export a function that returns a constant
  (func (export "answer") (result i32)
    (i32.const 42)))
