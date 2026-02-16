;; Exception Handling in WebAssembly (try_table)
;;
;; WebAssembly exception handling uses try_table with labeled branches.
;; When an exception is caught, control branches to the target label
;; with the exception payload (and optionally an exnref).

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
  ;; Basic Exception Handling
  ;; ============================================================

  ;; Safe division: throws on divide by zero
  (func $safe_divide (param $a i32) (param $b i32) (result i32)
    (block $catch (result i32)       ;; catch target - receives the exception payload
      (try_table (result i32) (catch $div_by_zero $catch)
        (if (i32.eqz (local.get $b))
          (then
            (throw $div_by_zero (local.get $a))))
        (i32.div_s (local.get $a) (local.get $b)))))

  ;; Division with default value: catch and return default on error
  (func $divide_with_default (param $a i32) (param $b i32) (param $default i32) (result i32)
    (block $catch (result i32)
      (try_table (result i32) (catch $div_by_zero $catch)
        (call $safe_divide (local.get $a) (local.get $b))
        (return)))
    ;; Caught div_by_zero — stack has [dividend]; drop it and return default
    (drop)
    (local.get $default))

  ;; ============================================================
  ;; Multiple Catch Clauses
  ;; ============================================================

  ;; Catch any exception using catch_all
  (func $try_any (param $op i32) (result i32)
    (block $catch_all_handler
      (try_table (catch_all $catch_all_handler)
        (if (i32.eqz (local.get $op))
          (then (throw $invalid_input)))
        (return (i32.const 42))))
    ;; catch_all handler — return sentinel
    (i32.const -1))

  ;; Multiple catch clauses with try_table
  ;; Each catch branches to a different labeled block
  (func $try_operation (param $op i32) (result i32)
    (local $result i32)
    (block $catch_all_block
      (block $catch_div_zero (result i32)
        (block $catch_invalid
          (try_table
            (catch $invalid_input $catch_invalid)
            (catch $div_by_zero $catch_div_zero)
            (catch_all $catch_all_block)
            ;; Try body
            (if (i32.eq (local.get $op) (i32.const 0))
              (then (throw $invalid_input)))
            (if (i32.eq (local.get $op) (i32.const 1))
              (then (throw $div_by_zero (i32.const 42))))
            (return (i32.const 100))))
        ;; catch $invalid_input handler
        (return (i32.const -1)))
      ;; catch $div_by_zero handler (payload on stack)
      (drop)
      (return (i32.const -2)))
    ;; catch_all handler
    (i32.const -999))

  ;; ============================================================
  ;; Nested Exception Handling
  ;; ============================================================

  ;; Nested try_table blocks
  ;; Uses local variable pattern to avoid complex nested block result types
  (func $nested (param $x i32) (result i32)
    (local $result i32)
    (local.set $result (i32.const -1))  ;; default: invalid input
    (block $done
      (block $outer_catch
        (try_table (catch $invalid_input $outer_catch)
          (block $inner_catch (result i32)
            (try_table (catch $div_by_zero $inner_catch)
              (if (i32.lt_s (local.get $x) (i32.const 0))
                (then (throw $invalid_input)))
              (local.set $result (call $safe_divide (i32.const 100) (local.get $x)))
              (br $done))
            (unreachable))
          ;; inner catch: div by zero — drop dividend payload, set result to 0
          (drop)
          (local.set $result (i32.const 0))
          (br $done))))
    ;; outer catch falls through with result = -1
    (local.get $result))

  ;; ============================================================
  ;; Rethrow and Propagation
  ;; ============================================================

  ;; Rethrow using throw_ref
  ;; Catch an exception, perform a side effect, and rethrow it
  (func $log_and_rethrow (param $a i32) (param $b i32) (result i32)
    (block $catch (result i32 exnref)
      (try_table (result i32) (catch_ref $div_by_zero $catch)
        (call $safe_divide (local.get $a) (local.get $b))
        (return))
      (unreachable))
    ;; Stack: [dividend, exnref]
    (i32.store (i32.const 0) (i32.const 1))  ;; log error flag
    (throw_ref))  ;; rethrow the caught exception

  ;; Catch-and-rethrow pattern (replaces legacy "delegate")
  ;; Inner handler catches specific exceptions, outer catches everything else
  (func $delegating_handler (param $x i32) (result i32)
    (block $outer_catch (result i32)
      (try_table (result i32) (catch $div_by_zero $outer_catch)
        (block $inner_catch (result exnref)
          (try_table (catch_all_ref $inner_catch)
            (if (i32.eqz (local.get $x))
              (then
                (throw $div_by_zero (local.get $x))))
            (return (local.get $x)))
          (unreachable))
        ;; inner catch_all: rethrow to outer handler
        (throw_ref)
        (unreachable)))
    ;; outer catch: div_by_zero handler
    (drop)
    (i32.const -1))

  ;; ============================================================
  ;; Practical Patterns
  ;; ============================================================

  ;; Safe array bounds checking with exceptions
  (func $safe_array_access (param $base i32) (param $index i32) (param $length i32) (result i32)
    (if (i32.ge_u (local.get $index) (local.get $length))
      (then
        (throw $out_of_bounds (local.get $index) (local.get $length))))
    (i32.load (i32.add (local.get $base) (i32.shl (local.get $index) (i32.const 2)))))

  ;; Multi-value exception: throw and catch with 3 values
  (func $throw_custom (param $a i32) (param $b i32) (param $c i32)
    (throw $custom_error (local.get $a) (local.get $b) (local.get $c)))

  (func $catch_custom (result i32)
    (block $catch (result i32 i32 i32)
      (try_table (catch $custom_error $catch)
        (call $throw_custom (i32.const 10) (i32.const 20) (i32.const 30)))
      (return (i32.const 0)))
    ;; Stack: [a, b, c] — sum them
    (i32.add)
    (i32.add))

  ;; ============================================================
  ;; Exports
  ;; ============================================================

  (export "safe_divide" (func $safe_divide))
  (export "divide_with_default" (func $divide_with_default))
  (export "try_operation" (func $try_operation))
  (export "nested" (func $nested))
  (export "log_and_rethrow" (func $log_and_rethrow))
  (export "delegating_handler" (func $delegating_handler))
  (export "safe_array_access" (func $safe_array_access))
  (export "catch_custom" (func $catch_custom)))
