;; Nested Block Comments
;; Demonstrates WAT's support for nested (; ... ;) block comments

(module
  (; This module shows how block comments can nest.
     Nested block comments are useful for commenting out
     code that already contains block comments. ;)

  ;; A simple identity function
  (func $identity (export "identity") (param $x i32) (result i32)
    (; Return the input unchanged ;)
    (local.get $x))

  (; Temporarily disabled — the outer comment hides everything,
     including the inner block comment:
     (; TODO: optimize this function ;)
     (func $unused (param i32) (result i32)
       (local.get 0))
  ;)

  ;; Double and add
  (func $double_add (export "double_add") (param $a i32) (param $b i32) (result i32)
    (; Computes (a * 2) + b ;)
    (i32.add
      (i32.mul
        (local.get $a)
        (i32.const 2))
      (local.get $b)))

  (; Deeply nested comments also work:
     (; level 1
        (; level 2 ;)
     ;)
  ;)

  ;; Negate a number
  (func $negate (export "negate") (param $n i32) (result i32)
    (i32.sub
      (i32.const 0)
      (local.get $n))))
