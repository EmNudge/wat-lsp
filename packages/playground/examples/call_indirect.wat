;; Indirect Calls (call_indirect)
;; Demonstrates call_indirect in both linear and folded style,
;; with varying parameter counts.

(module
  ;; Function type signatures
  (type $void_to_i32 (func (result i32)))
  (type $unary (func (param i32) (result i32)))
  (type $binary (func (param i32 i32) (result i32)))

  ;; A table of functions
  (table 4 funcref)

  ;; Populate the table
  (elem (i32.const 0) func $forty_two $double $add $square)

  ;; Target functions
  (func $forty_two (result i32)
    i32.const 42)

  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  (func $square (param $x i32) (result i32)
    (i32.mul (local.get $x) (local.get $x)))

  ;; --- Linear (unfolded) style ---

  ;; No-param call: just the table index on the stack
  (func $linear_nullary (export "linear_nullary") (result i32)
    i32.const 0                        ;; table index -> $forty_two
    call_indirect (type $void_to_i32))

  ;; One-param call: arg + table index
  (func $linear_unary (export "linear_unary") (param $x i32) (result i32)
    local.get $x                       ;; argument
    i32.const 1                        ;; table index -> $double
    call_indirect (type $unary))

  ;; Two-param call: arg, arg, table index
  (func $linear_binary (export "linear_binary") (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.const 2                        ;; table index -> $add
    call_indirect (type $binary))

  ;; --- Folded (S-expression) style ---

  (func $folded_nullary (export "folded_nullary") (result i32)
    (call_indirect (type $void_to_i32)
      (i32.const 0)))                  ;; table index -> $forty_two

  (func $folded_unary (export "folded_unary") (param $x i32) (result i32)
    (call_indirect (type $unary)
      (local.get $x)                   ;; argument
      (i32.const 3)))                  ;; table index -> $square

  (func $folded_binary (export "folded_binary") (param $a i32) (param $b i32) (result i32)
    (call_indirect (type $binary)
      (local.get $a)
      (local.get $b)
      (i32.const 2)))                  ;; table index -> $add

)
