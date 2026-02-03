;; Trapping and Control Flow
;; Demonstrates unreachable, traps, and complex control flow

(module
  (memory 1)

  ;; Global error code
  (global $last_error (mut i32) (i32.const 0))

  ;; Error codes
  (global $ERR_NONE i32 (i32.const 0))
  (global $ERR_DIV_ZERO i32 (i32.const 1))
  (global $ERR_OVERFLOW i32 (i32.const 2))
  (global $ERR_UNDERFLOW i32 (i32.const 3))
  (global $ERR_OUT_OF_BOUNDS i32 (i32.const 4))
  (global $ERR_INVALID_INPUT i32 (i32.const 5))

  ;; === Explicit Traps ===

  ;; Always trap (unreachable instruction)
  (func $always_trap
    (unreachable))

  ;; Conditional trap
  (func $trap_if_zero (param $x i32)
    (if (i32.eqz (local.get $x))
      (then (unreachable))))

  ;; Trap with message in memory (for debugging)
  (func $trap_with_code (param $code i32)
    (global.set $last_error (local.get $code))
    (unreachable))

  ;; === Division Traps ===

  ;; Integer division (traps on divide by zero)
  (func $div_i32 (param $a i32) (param $b i32) (result i32)
    ;; This will trap if b is 0
    (i32.div_s (local.get $a) (local.get $b)))

  ;; Integer division (traps on overflow: MIN_INT / -1)
  (func $div_overflow_trap (result i32)
    ;; -2147483648 / -1 would overflow, causing a trap
    (i32.div_s (i32.const -2147483648) (i32.const -1)))

  ;; Safe division that checks before dividing
  (func $safe_div (param $a i32) (param $b i32) (result i32)
    ;; Check for zero
    (if (i32.eqz (local.get $b))
      (then
        (global.set $last_error (global.get $ERR_DIV_ZERO))
        (return (i32.const 0))))
    ;; Check for overflow case
    (if (i32.and
          (i32.eq (local.get $a) (i32.const -2147483648))
          (i32.eq (local.get $b) (i32.const -1)))
      (then
        (global.set $last_error (global.get $ERR_OVERFLOW))
        (return (i32.const 2147483647))))
    (global.set $last_error (global.get $ERR_NONE))
    (i32.div_s (local.get $a) (local.get $b)))

  ;; === Memory Access Traps ===

  ;; Load from address (traps if out of bounds)
  (func $load_i32 (param $addr i32) (result i32)
    (i32.load (local.get $addr)))

  ;; Safe load with bounds check
  (func $safe_load (param $addr i32) (param $max_addr i32) (result i32)
    (if (i32.gt_u (i32.add (local.get $addr) (i32.const 4)) (local.get $max_addr))
      (then
        (global.set $last_error (global.get $ERR_OUT_OF_BOUNDS))
        (return (i32.const 0))))
    (global.set $last_error (global.get $ERR_NONE))
    (i32.load (local.get $addr)))

  ;; === Conversion Traps ===

  ;; Float to int conversion (traps on NaN or out of range)
  (func $f32_to_i32_trap (param $f f32) (result i32)
    ;; This will trap if f is NaN or outside i32 range
    (i32.trunc_f32_s (local.get $f)))

  ;; Safe float to int with saturation
  (func $f32_to_i32_safe (param $f f32) (result i32)
    ;; Saturating conversion (never traps)
    (i32.trunc_sat_f32_s (local.get $f)))

  ;; f64 to i64 (may trap)
  (func $f64_to_i64_trap (param $f f64) (result i64)
    (i64.trunc_f64_s (local.get $f)))

  ;; Safe f64 to i64
  (func $f64_to_i64_safe (param $f f64) (result i64)
    (i64.trunc_sat_f64_s (local.get $f)))

  ;; === Complex Control Flow ===

  ;; Nested blocks with breaks
  (func $nested_blocks (param $x i32) (result i32)
    (block $outer (result i32)
      (block $middle (result i32)
        (block $inner (result i32)
          (br_if $outer (i32.const 100) (i32.eq (local.get $x) (i32.const 0)))
          (br_if $middle (i32.const 200) (i32.eq (local.get $x) (i32.const 1)))
          (br_if $inner (i32.const 300) (i32.eq (local.get $x) (i32.const 2)))
          (i32.const 999)))))

  ;; Block with typed result
  (func $typed_block (param $cond i32) (result i32 i32)
    (block (result i32 i32)
      (if (local.get $cond)
        (then
          (br 1 (i32.const 1) (i32.const 2))))
      (i32.const 3)
      (i32.const 4)))

  ;; Nested loops with breaks
  (func $nested_loops (param $outer_count i32) (param $inner_count i32) (result i32)
    (local $i i32)
    (local $j i32)
    (local $sum i32)
    (block $outer_break
      (loop $outer
        (br_if $outer_break (i32.ge_u (local.get $i) (local.get $outer_count)))
        (local.set $j (i32.const 0))
        (block $inner_break
          (loop $inner
            (br_if $inner_break (i32.ge_u (local.get $j) (local.get $inner_count)))
            (local.set $sum
              (i32.add (local.get $sum)
                (i32.mul (local.get $i) (local.get $j))))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))
            (br $inner)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $outer)))
    (local.get $sum))

  ;; br_table (switch statement)
  (func $switch (param $selector i32) (result i32)
    (block $default (result i32)
      (block $case3 (result i32)
        (block $case2 (result i32)
          (block $case1 (result i32)
            (block $case0 (result i32)
              (br_table $case0 $case1 $case2 $case3 $default
                (local.get $selector))
              (i32.const -1))  ;; never reached
            (return (i32.const 100)))
          (return (i32.const 200)))
        (return (i32.const 300)))
      (return (i32.const 400)))
    (i32.const 0))  ;; default case

  ;; Complex br_table with fallthrough simulation
  (func $complex_switch (param $x i32) (result i32)
    (local $result i32)
    (block $end
      (block $default
        (block $c4
          (block $c3
            (block $c2
              (block $c1
                (block $c0
                  (br_table $c0 $c1 $c2 $c3 $c4 $default (local.get $x)))
                ;; case 0
                (local.set $result (i32.const 1))
                (br $end))
              ;; case 1
              (local.set $result (i32.const 10))
              (br $end))
            ;; case 2
            (local.set $result (i32.const 100))
            (br $end))
          ;; case 3
          (local.set $result (i32.const 1000))
          (br $end))
        ;; case 4
        (local.set $result (i32.const 10000))
        (br $end))
      ;; default
      (local.set $result (i32.const -1)))
    (local.get $result))

  ;; Return from nested blocks
  (func $early_return (param $arr_offset i32) (param $len i32) (param $target i32) (result i32)
    (local $i i32)
    (block $not_found
      (loop $search
        (br_if $not_found (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.eq
              (i32.load
                (i32.add (local.get $arr_offset)
                  (i32.shl (local.get $i) (i32.const 2))))
              (local.get $target))
          (then (return (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $search)))
    (i32.const -1))

  ;; Select instruction (ternary)
  (func $min_select (param $a i32) (param $b i32) (result i32)
    (select (local.get $a) (local.get $b)
      (i32.lt_s (local.get $a) (local.get $b))))

  ;; Typed select
  (func $max_select (param $a i32) (param $b i32) (result i32)
    (select (result i32) (local.get $a) (local.get $b)
      (i32.gt_s (local.get $a) (local.get $b))))

  ;; Clamp value between min and max using select
  (func $clamp (param $val i32) (param $min i32) (param $max i32) (result i32)
    (select
      (local.get $max)
      (select
        (local.get $min)
        (local.get $val)
        (i32.lt_s (local.get $val) (local.get $min)))
      (i32.gt_s (local.get $val) (local.get $max))))

  ;; Validate input and process
  (func $validate_process (param $input i32) (result i32)
    ;; Chain of validations
    (if (i32.lt_s (local.get $input) (i32.const 0))
      (then
        (global.set $last_error (global.get $ERR_INVALID_INPUT))
        (return (i32.const -1))))
    (if (i32.gt_s (local.get $input) (i32.const 1000))
      (then
        (global.set $last_error (global.get $ERR_OVERFLOW))
        (return (i32.const -2))))
    ;; Process valid input
    (global.set $last_error (global.get $ERR_NONE))
    (i32.mul (local.get $input) (local.get $input)))

  ;; Get last error
  (func $get_error (result i32)
    (global.get $last_error))

  ;; Clear error
  (func $clear_error
    (global.set $last_error (global.get $ERR_NONE)))

  ;; Exports
  (export "trap_if_zero" (func $trap_if_zero))
  (export "div_i32" (func $div_i32))
  (export "safe_div" (func $safe_div))
  (export "f32_to_i32_trap" (func $f32_to_i32_trap))
  (export "f32_to_i32_safe" (func $f32_to_i32_safe))
  (export "nested_blocks" (func $nested_blocks))
  (export "nested_loops" (func $nested_loops))
  (export "switch" (func $switch))
  (export "complex_switch" (func $complex_switch))
  (export "early_return" (func $early_return))
  (export "clamp" (func $clamp))
  (export "validate_process" (func $validate_process))
  (export "get_error" (func $get_error))
  (export "memory" (memory 0)))
