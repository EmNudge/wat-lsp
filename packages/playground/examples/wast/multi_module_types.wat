;; Multi-Module Type Independence
;; Demonstrates that each module has its own type, function, and index spaces

;; Module A: Uses $sig for an i32 → i32 function type
(module
  (type $sig (func (param i32) (result i32)))

  (func $apply (export "double") (type $sig)
    (i32.mul (local.get 0) (i32.const 2)))

  (func $apply_twice (export "quadruple") (param $x i32) (result i32)
    (call $apply (call $apply (local.get $x))))

  (table $fns 1 funcref)
  (elem (i32.const 0) func $apply)

  (func $call_table (export "call_table") (param $x i32) (result i32)
    (call_indirect $fns (type $sig)
      (local.get $x)
      (i32.const 0)))
)

;; Module B: Reuses $sig for a completely different signature (f64 → f64)
;; No conflict with Module A's $sig — independent type spaces
(module
  (type $sig (func (param f64) (result f64)))

  (func $apply (export "negate") (type $sig)
    (f64.neg (local.get 0)))

  (func $apply_twice (export "double_negate") (param $x f64) (result f64)
    (call $apply (call $apply (local.get $x))))

  (global $pi f64 (f64.const 3.141592653589793))

  (func $area (export "circle_area") (param $r f64) (result f64)
    (f64.mul (global.get $pi) (f64.mul (local.get $r) (local.get $r))))
)

;; Module C: Complex types with the same names as A and B
(module
  (type $sig (func (param i64) (result i64)))

  (func $apply (export "factorial") (type $sig)
    (if (result i64) (i64.le_u (local.get 0) (i64.const 1))
      (then (i64.const 1))
      (else
        (i64.mul
          (local.get 0)
          (call $apply (i64.sub (local.get 0) (i64.const 1)))))))

  (func $fib (export "fibonacci") (param $n i64) (result i64)
    (if (result i64) (i64.le_u (local.get $n) (i64.const 1))
      (then (local.get $n))
      (else
        (i64.add
          (call $fib (i64.sub (local.get $n) (i64.const 1)))
          (call $fib (i64.sub (local.get $n) (i64.const 2)))))))
)
