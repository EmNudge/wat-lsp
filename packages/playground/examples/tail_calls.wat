;; Tail Call Proposal
;; Demonstrates return_call and return_call_indirect for tail recursion

(module
  ;; Type definitions for indirect tail calls
  (type $unary_i32 (func (param i32) (result i32)))
  (type $binary_i32 (func (param i32 i32) (result i32)))
  (type $ternary_i32 (func (param i32 i32 i32) (result i32)))

  ;; Function table for indirect tail calls
  (table $ops 4 funcref)
  (elem (table $ops) (i32.const 0) func $add $sub $mul $identity)

  ;; Helper functions
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  (func $sub (param $a i32) (param $b i32) (result i32)
    (i32.sub (local.get $a) (local.get $b)))

  (func $mul (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b)))

  (func $identity (param $a i32) (param $b i32) (result i32)
    (local.get $a))

  ;; Tail-recursive factorial with accumulator
  (func $factorial_acc (param $n i32) (param $acc i32) (result i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then (local.get $acc))
      (else
        (return_call $factorial_acc
          (i32.sub (local.get $n) (i32.const 1))
          (i32.mul (local.get $acc) (local.get $n))))))

  ;; Public factorial function
  (func $factorial (param $n i32) (result i32)
    (return_call $factorial_acc (local.get $n) (i32.const 1)))

  ;; Tail-recursive Fibonacci with accumulators
  (func $fib_acc (param $n i32) (param $a i32) (param $b i32) (result i32)
    (if (result i32) (i32.eqz (local.get $n))
      (then (local.get $a))
      (else
        (return_call $fib_acc
          (i32.sub (local.get $n) (i32.const 1))
          (local.get $b)
          (i32.add (local.get $a) (local.get $b))))))

  ;; Public Fibonacci function
  (func $fibonacci (param $n i32) (result i32)
    (return_call $fib_acc (local.get $n) (i32.const 0) (i32.const 1)))

  ;; Tail-recursive GCD (Euclidean algorithm)
  (func $gcd (param $a i32) (param $b i32) (result i32)
    (if (result i32) (i32.eqz (local.get $b))
      (then (local.get $a))
      (else
        (return_call $gcd
          (local.get $b)
          (i32.rem_u (local.get $a) (local.get $b))))))

  ;; Tail-recursive sum from 1 to n
  (func $sum_to_n_acc (param $n i32) (param $acc i32) (result i32)
    (if (result i32) (i32.eqz (local.get $n))
      (then (local.get $acc))
      (else
        (return_call $sum_to_n_acc
          (i32.sub (local.get $n) (i32.const 1))
          (i32.add (local.get $acc) (local.get $n))))))

  (func $sum_to_n (param $n i32) (result i32)
    (return_call $sum_to_n_acc (local.get $n) (i32.const 0)))

  ;; Tail-recursive power function
  (func $pow_acc (param $base i32) (param $exp i32) (param $acc i32) (result i32)
    (if (result i32) (i32.eqz (local.get $exp))
      (then (local.get $acc))
      (else
        (if (result i32) (i32.and (local.get $exp) (i32.const 1))
          ;; Odd exponent: acc = acc * base, exp = exp - 1
          (then
            (return_call $pow_acc
              (local.get $base)
              (i32.sub (local.get $exp) (i32.const 1))
              (i32.mul (local.get $acc) (local.get $base))))
          ;; Even exponent: base = base * base, exp = exp / 2
          (else
            (return_call $pow_acc
              (i32.mul (local.get $base) (local.get $base))
              (i32.shr_u (local.get $exp) (i32.const 1))
              (local.get $acc)))))))

  (func $pow (param $base i32) (param $exp i32) (result i32)
    (return_call $pow_acc (local.get $base) (local.get $exp) (i32.const 1)))

  ;; Indirect tail call: apply operation by index
  (func $apply_op (param $op_idx i32) (param $a i32) (param $b i32) (result i32)
    (return_call_indirect $ops (type $binary_i32)
      (local.get $a)
      (local.get $b)
      (local.get $op_idx)))

  ;; Repeated application via tail calls
  (func $repeat_op_acc (param $op_idx i32) (param $acc i32) (param $val i32) (param $times i32) (result i32)
    (if (result i32) (i32.eqz (local.get $times))
      (then (local.get $acc))
      (else
        (return_call $repeat_op_acc
          (local.get $op_idx)
          (call $apply_op (local.get $op_idx) (local.get $acc) (local.get $val))
          (local.get $val)
          (i32.sub (local.get $times) (i32.const 1))))))

  (func $repeat_op (param $op_idx i32) (param $init i32) (param $val i32) (param $times i32) (result i32)
    (return_call $repeat_op_acc
      (local.get $op_idx)
      (local.get $init)
      (local.get $val)
      (local.get $times)))

  ;; Mutual recursion with tail calls: even/odd check
  (func $is_even (param $n i32) (result i32)
    (if (result i32) (i32.eqz (local.get $n))
      (then (i32.const 1))
      (else (return_call $is_odd (i32.sub (local.get $n) (i32.const 1))))))

  (func $is_odd (param $n i32) (result i32)
    (if (result i32) (i32.eqz (local.get $n))
      (then (i32.const 0))
      (else (return_call $is_even (i32.sub (local.get $n) (i32.const 1))))))

  ;; Ackermann function with tail call (pseudo-CPS style)
  ;; Note: True Ackermann cannot be fully tail-recursive without CPS
  ;; This is a simplified version
  (func $ackermann_iter (param $m i32) (param $n i32) (result i32)
    (if (result i32) (i32.eqz (local.get $m))
      (then (i32.add (local.get $n) (i32.const 1)))
      (else
        (if (result i32) (i32.eqz (local.get $n))
          (then
            (return_call $ackermann_iter
              (i32.sub (local.get $m) (i32.const 1))
              (i32.const 1)))
          (else
            ;; A(m, n) = A(m-1, A(m, n-1))
            ;; Cannot be fully tail-recursive without CPS
            (call $ackermann_iter
              (i32.sub (local.get $m) (i32.const 1))
              (call $ackermann_iter
                (local.get $m)
                (i32.sub (local.get $n) (i32.const 1)))))))))

  ;; Count down to zero (simple tail recursion demo)
  (func $countdown (param $n i32) (result i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 0))
      (then (i32.const 0))
      (else
        (return_call $countdown (i32.sub (local.get $n) (i32.const 1))))))

  ;; Collatz sequence length (tail recursive)
  (func $collatz_len_acc (param $n i32) (param $len i32) (result i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then (local.get $len))
      (else
        (if (result i32) (i32.and (local.get $n) (i32.const 1))
          ;; Odd: 3n + 1
          (then
            (return_call $collatz_len_acc
              (i32.add
                (i32.mul (local.get $n) (i32.const 3))
                (i32.const 1))
              (i32.add (local.get $len) (i32.const 1))))
          ;; Even: n / 2
          (else
            (return_call $collatz_len_acc
              (i32.shr_u (local.get $n) (i32.const 1))
              (i32.add (local.get $len) (i32.const 1))))))))

  (func $collatz_length (param $n i32) (result i32)
    (return_call $collatz_len_acc (local.get $n) (i32.const 0)))

  ;; Exports
  (export "factorial" (func $factorial))
  (export "fibonacci" (func $fibonacci))
  (export "gcd" (func $gcd))
  (export "sum_to_n" (func $sum_to_n))
  (export "pow" (func $pow))
  (export "apply_op" (func $apply_op))
  (export "repeat_op" (func $repeat_op))
  (export "is_even" (func $is_even))
  (export "is_odd" (func $is_odd))
  (export "countdown" (func $countdown))
  (export "collatz_length" (func $collatz_length)))
