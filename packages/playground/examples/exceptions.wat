;; Exception Handling in WebAssembly
;;
;; This file demonstrates both:
;; 1. Legacy exception handling (try/catch/throw) - supported in browsers since 2022
;; 2. Modern exception handling (try_table) - WebAssembly 3.0 standard (2025)
;;
;; The legacy syntax is still supported in browsers but may be deprecated.
;; New code should prefer try_table.

(module
  ;; ============================================================
  ;; Exception Tags
  ;; Tags define the signature of exceptions (what values they carry)
  ;; ============================================================

  (tag $div_by_zero (param i32))           ;; carries the dividend
  (tag $out_of_bounds (param i32 i32))     ;; carries index and length
  (tag $invalid_input)                      ;; no payload
  (tag $custom_error (param i32 i32 i32))  ;; multi-value payload

  (memory 1)

  ;; ============================================================
  ;; PART 1: MODERN SYNTAX (try_table) - WebAssembly 3.0
  ;;
  ;; try_table uses labeled branches for exception handling.
  ;; When an exception is caught, control branches to the label
  ;; with the exception payload (and optionally an exnref).
  ;; ============================================================

  ;; Safe division using try_table
  (func $safe_divide_modern (param $a i32) (param $b i32) (result i32)
    (block $catch (result i32)       ;; catch target - receives the exception payload
      (try_table (result i32) (catch $div_by_zero $catch)
        (if (i32.eqz (local.get $b))
          (then
            (throw $div_by_zero (local.get $a))))
        (i32.div_s (local.get $a) (local.get $b)))))

  ;; Division with default value using try_table
  (func $divide_with_default_modern (param $a i32) (param $b i32) (param $default i32) (result i32)
    (block $on_error (result i32)    ;; receives dividend from exception
      (drop                          ;; drop the exception payload
        (br_on_null $on_error        ;; if somehow null, use default
          (block $catch (result i32 exnref)
            (try_table (result i32) (catch_ref $div_by_zero $catch)
              (call $safe_divide_modern (local.get $a) (local.get $b))
              (return)))))
      (local.get $default)))

  ;; Catch any exception using catch_all
  (func $try_any_modern (param $op i32) (result i32)
    (block $catch_all_handler (result exnref)
      (try_table (result i32) (catch_all_ref $catch_all_handler)
        (if (i32.eqz (local.get $op))
          (then (throw $invalid_input)))
        (i32.const 42)
        (return)))
    (drop)  ;; drop the exnref
    (i32.const -1))

  ;; Nested try_table blocks
  ;; Uses local variable pattern to avoid complex nested block result types
  (func $nested_modern (param $x i32) (result i32)
    (local $result i32)
    (local.set $result (i32.const -1))  ;; default: invalid input
    (block $done
      (block $outer_catch
        (try_table (catch $invalid_input $outer_catch)
          (block $inner_catch
            (try_table (catch $div_by_zero $inner_catch)
              (if (i32.lt_s (local.get $x) (i32.const 0))
                (then (throw $invalid_input)))
              (local.set $result (call $safe_divide_modern (i32.const 100) (local.get $x)))
              (br $done)))
          ;; inner catch: div by zero, return 0
          (local.set $result (i32.const 0))
          (br $done))))
    ;; outer catch falls through with result = -1
    (local.get $result))

  ;; Rethrow using throw_ref
  (func $log_and_rethrow_modern (param $a i32) (param $b i32) (result i32)
    (block $catch (result i32 exnref)
      (try_table (result i32) (catch_ref $div_by_zero $catch)
        (call $safe_divide_modern (local.get $a) (local.get $b))
        (return)))
    ;; Stack: [dividend, exnref]
    (i32.store (i32.const 0) (i32.const 1))  ;; log error flag
    (throw_ref))  ;; rethrow the caught exception

  ;; ============================================================
  ;; PART 2: LEGACY SYNTAX (try/do/catch)
  ;;
  ;; This syntax is still supported in browsers but deprecated.
  ;; The folded form uses (do ...) for the try body.
  ;; ============================================================

  ;; Safe division that throws on divide by zero
  (func $safe_divide_legacy (param $a i32) (param $b i32) (result i32)
    (if (i32.eqz (local.get $b))
      (then
        (throw $div_by_zero (local.get $a))))
    (i32.div_s (local.get $a) (local.get $b)))

  ;; Function that catches division errors (legacy folded syntax)
  (func $divide_with_default_legacy (param $a i32) (param $b i32) (param $default i32) (result i32)
    (try (result i32)
      (do
        (call $safe_divide_legacy (local.get $a) (local.get $b)))
      (catch $div_by_zero
        (drop)  ;; drop the exception payload
        (local.get $default))))

  ;; Multiple catch clauses (legacy)
  (func $try_operation_legacy (param $op i32) (result i32)
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

  ;; Nested try blocks (legacy)
  (func $nested_try_legacy (param $x i32) (result i32)
    (try (result i32)
      (do
        (try (result i32)
          (do
            (if (i32.lt_s (local.get $x) (i32.const 0))
              (then
                (throw $invalid_input)))
            (call $safe_divide_legacy (i32.const 100) (local.get $x)))
          (catch $div_by_zero
            (drop)
            (i32.const 0))))
      (catch $invalid_input
        (i32.const -1))))

  ;; Function that rethrows an exception (legacy)
  (func $log_and_rethrow_legacy (param $a i32) (param $b i32) (result i32)
    (try (result i32)
      (do
        (call $safe_divide_legacy (local.get $a) (local.get $b)))
      (catch $div_by_zero
        ;; Log the error (store to memory)
        (i32.store (i32.const 0) (i32.const 1))  ;; error flag
        ;; Rethrow the exception
        (rethrow 0))))

  ;; Delegate to outer handler (legacy)
  (func $delegating_handler_legacy (param $x i32) (result i32)
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

  ;; ============================================================
  ;; PART 3: UNFOLD SYNTAX (also legacy, but different form)
  ;;
  ;; The unfold form doesn't use (do ...), instructions are inline.
  ;; ============================================================

  ;; Example using folded try syntax (unfold syntax has limited tooling support)
  (func $unfold_example (param $x i32) (result i32)
    (try (result i32)
      (do
        (if (i32.eqz (local.get $x))
          (then
            (throw $div_by_zero (i32.const 42))))
        (local.get $x))
      (catch $div_by_zero
        (drop)
        (i32.const -1))))

  ;; ============================================================
  ;; Exports
  ;; ============================================================

  ;; Modern (try_table)
  (export "safe_divide" (func $safe_divide_modern))
  (export "divide_with_default" (func $divide_with_default_modern))
  (export "nested" (func $nested_modern))

  ;; Legacy (try/do/catch)
  (export "safe_divide_legacy" (func $safe_divide_legacy))
  (export "divide_with_default_legacy" (func $divide_with_default_legacy))
  (export "nested_legacy" (func $nested_try_legacy)))
