;; Chained Indirect Calls
;; Demonstrates chaining multiple linear call_indirect instructions,
;; where the result of one feeds into the next (pipeline pattern).
;; Related: issue #132

(module
  ;; Function type signatures
  (type $unary (func (param i32) (result i32)))
  (type $binary (func (param i32 i32) (result i32)))

  ;; Function tables
  (table 5 funcref)
  (elem (i32.const 0) func $double $square $negate $inc $add)

  ;; Target functions
  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func $square (param $x i32) (result i32)
    (i32.mul (local.get $x) (local.get $x)))

  (func $negate (param $x i32) (result i32)
    (i32.sub (i32.const 0) (local.get $x)))

  (func $inc (param $x i32) (result i32)
    (i32.add (local.get $x) (i32.const 1)))

  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  ;; --- Chained linear call_indirect ---

  ;; Two chained unary calls: double then square
  ;; Stack: $x -> [i32] -> [i32, i32] -> call_indirect -> [i32] -> [i32, i32] -> call_indirect -> [i32]
  (func (export "double_then_square") (param $x i32) (result i32)
    local.get $x
    i32.const 0                        ;; table index -> $double
    call_indirect (type $unary)
    i32.const 1                        ;; table index -> $square
    call_indirect (type $unary))

  ;; Three chained unary calls: double, negate, increment
  (func (export "pipeline3") (param $x i32) (result i32)
    local.get $x
    i32.const 0                        ;; table index -> $double
    call_indirect (type $unary)
    i32.const 2                        ;; table index -> $negate
    call_indirect (type $unary)
    i32.const 3                        ;; table index -> $inc
    call_indirect (type $unary))

  ;; Chain with a binary call at the end: double two values, then add them
  (func (export "double_both_and_add") (param $a i32) (param $b i32) (result i32)
    local.get $a
    i32.const 0                        ;; table index -> $double
    call_indirect (type $unary)
    local.get $b
    i32.const 0                        ;; table index -> $double
    call_indirect (type $unary)
    i32.const 4                        ;; table index -> $add
    call_indirect (type $binary))

  ;; --- Mixed folded and linear chaining ---

  ;; Folded outer, linear inner: square(double(x))
  (func (export "mixed_chain") (param $x i32) (result i32)
    (call_indirect (type $unary)
      (call_indirect (type $unary)
        (local.get $x)
        (i32.const 0))                 ;; inner: $double
      (i32.const 1)))                  ;; outer: $square

  ;; Dynamic pipeline: apply whichever operation the caller picks, twice
  (func (export "apply_twice") (param $x i32) (param $op i32) (result i32)
    local.get $x
    local.get $op
    call_indirect (type $unary)
    local.get $op
    call_indirect (type $unary))
)
