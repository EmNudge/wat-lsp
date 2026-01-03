;; Exception Handling Proposal
;; Demonstrates try/catch/throw/rethrow

(module
  ;; Define exception tags
  (tag $div_by_zero (param i32))
  (tag $out_of_bounds (param i32 i32))  ;; index, length
  (tag $invalid_input)
  (tag $custom_error (param i32 i32 i32))

  (memory 1)

  ;; Safe division that throws on divide by zero
  (func $safe_divide (param $a i32) (param $b i32) (result i32)
    (if (i32.eqz (local.get $b))
      (then
        (throw $div_by_zero (local.get $a))))
    (i32.div_s (local.get $a) (local.get $b)))

  ;; Bounds-checked array access
  (func $checked_load (param $idx i32) (param $len i32) (result i32)
    (if (i32.ge_u (local.get $idx) (local.get $len))
      (then
        (throw $out_of_bounds (local.get $idx) (local.get $len))))
    (i32.load (i32.shl (local.get $idx) (i32.const 2))))

  ;; Function that catches division errors
  (func $divide_with_default (param $a i32) (param $b i32) (param $default i32) (result i32)
    (try (result i32)
      (do
        (call $safe_divide (local.get $a) (local.get $b)))
      (catch $div_by_zero
        (drop)  ;; drop the exception payload
        (local.get $default))))

  ;; Function demonstrating catch_all
  (func $try_operation (param $op i32) (result i32)
    (try (result i32)
      (do
        (if (result i32) (i32.eq (local.get $op) (i32.const 0))
          (then
            (throw $invalid_input)
            (unreachable))
          (else
            (if (result i32) (i32.eq (local.get $op) (i32.const 1))
              (then
                (throw $div_by_zero (i32.const 42))
                (unreachable))
              (else
                (i32.const 100))))))
      (catch $invalid_input
        (i32.const -1))
      (catch $div_by_zero
        (drop)
        (i32.const -2))
      (catch_all
        (i32.const -999))))

  ;; Nested try blocks
  (func $nested_try (param $x i32) (result i32)
    (try (result i32)
      (do
        (try (result i32)
          (do
            (if (i32.lt_s (local.get $x) (i32.const 0))
              (then
                (throw $invalid_input)))
            (call $safe_divide (i32.const 100) (local.get $x)))
          (catch $div_by_zero
            (drop)
            (i32.const 0))))
      (catch $invalid_input
        (i32.const -1))))

  ;; Function that rethrows an exception
  (func $log_and_rethrow (param $a i32) (param $b i32) (result i32)
    (try (result i32)
      (do
        (call $safe_divide (local.get $a) (local.get $b)))
      (catch $div_by_zero
        ;; Log the error (store to memory)
        (i32.store (i32.const 0) (i32.const 1))  ;; error flag
        ;; Rethrow the exception
        (rethrow 0))))

  ;; Function using try with delegate
  (func $delegating_handler (param $x i32) (result i32)
    (try (result i32)
      (do
        (try (result i32)
          (do
            (if (i32.eqz (local.get $x))
              (then
                (throw $div_by_zero (local.get $x))))
            (local.get $x))
          (delegate 0)))  ;; delegate to outer try
      (catch $div_by_zero
        (drop)
        (i32.const -1))))

  ;; Multi-value exception payload
  (func $throw_custom (param $a i32) (param $b i32) (param $c i32)
    (throw $custom_error (local.get $a) (local.get $b) (local.get $c)))

  ;; Catch multi-value exception
  (func $catch_custom (result i32)
    (try (result i32)
      (do
        (call $throw_custom (i32.const 10) (i32.const 20) (i32.const 30))
        (i32.const 0))
      (catch $custom_error
        ;; Stack has: a b c (top)
        ;; Sum them all
        (i32.add (i32.add)))))

  ;; Example: Validate input and process
  (func $validate_and_process (param $input i32) (result i32)
    (local $result i32)
    (try (result i32)
      (do
        ;; Validate: must be positive
        (if (i32.le_s (local.get $input) (i32.const 0))
          (then
            (throw $invalid_input)))
        ;; Validate: must be less than 1000
        (if (i32.ge_s (local.get $input) (i32.const 1000))
          (then
            (throw $out_of_bounds (local.get $input) (i32.const 1000))))
        ;; Process: double and add 1
        (i32.add
          (i32.mul (local.get $input) (i32.const 2))
          (i32.const 1)))
      (catch $invalid_input
        (i32.const -1))
      (catch $out_of_bounds
        (drop)
        (drop)
        (i32.const -2))
      (catch_all
        (i32.const -999))))

  ;; Exports
  (export "safe_divide" (func $safe_divide))
  (export "divide_with_default" (func $divide_with_default))
  (export "try_operation" (func $try_operation))
  (export "nested_try" (func $nested_try))
  (export "validate_and_process" (func $validate_and_process)))
