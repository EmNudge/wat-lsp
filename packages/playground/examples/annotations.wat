;; Annotations
;; Demonstrates WebAssembly annotation syntax for metadata

(module
  ;; === Module-level Annotations ===

  ;; Name annotation provides debug names for the module
  (@name "math_utils")

  ;; Producers annotation records toolchain information
  (@producers
    (language "WAT" "1.0")
    (processed-by "wat-lsp" "0.1.0"))

  ;; Custom section for arbitrary metadata
  (@custom "version" "1.2.3")
  (@custom "author" "WebAssembly Developer")

  ;; === Type Definitions ===

  (type $binary_op (func (param i32 i32) (result i32)))
  (type $unary_op (func (param i32) (result i32)))

  ;; === Memory ===

  (memory $mem 1)

  ;; === Global Variables ===

  ;; Counter with debug name annotation
  (global $counter (mut i32) (i32.const 0))

  ;; === Functions with Name Annotations ===

  ;; Add function with descriptive name annotation
  (func $add (@name "integer_addition")
    (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  ;; Subtract function
  (func $sub (@name "integer_subtraction")
    (param $a i32) (param $b i32) (result i32)
    (i32.sub (local.get $a) (local.get $b)))

  ;; Multiply function
  (func $mul (@name "integer_multiplication")
    (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b)))

  ;; Division with safety check
  (func $safe_div (@name "safe_integer_division")
    (param $a i32) (param $b i32) (result i32)
    (if (result i32) (i32.eqz (local.get $b))
      (then (i32.const 0))  ;; Return 0 on division by zero
      (else (i32.div_s (local.get $a) (local.get $b)))))

  ;; Factorial with loop
  (func $factorial (@name "factorial_iterative")
    (param $n i32) (result i32)
    (local $result i32)
    (local $i i32)

    (local.set $result (i32.const 1))
    (local.set $i (i32.const 1))

    (block $done
      (loop $continue
        (br_if $done (i32.gt_s (local.get $i) (local.get $n)))
        (local.set $result
          (i32.mul (local.get $result) (local.get $i)))
        (local.set $i
          (i32.add (local.get $i) (i32.const 1)))
        (br $continue)))

    (local.get $result))

  ;; Fibonacci using recursion
  (func $fibonacci (@name "fibonacci_recursive")
    (param $n i32) (result i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then (local.get $n))
      (else
        (i32.add
          (call $fibonacci (i32.sub (local.get $n) (i32.const 1)))
          (call $fibonacci (i32.sub (local.get $n) (i32.const 2)))))))

  ;; Counter operations
  (func $increment (@name "increment_counter") (result i32)
    (global.set $counter
      (i32.add (global.get $counter) (i32.const 1)))
    (global.get $counter))

  (func $reset (@name "reset_counter")
    (global.set $counter (i32.const 0)))

  (func $get_counter (@name "get_counter_value") (result i32)
    (global.get $counter))

  ;; === Exports ===

  (export "add" (func $add))
  (export "sub" (func $sub))
  (export "mul" (func $mul))
  (export "safe_div" (func $safe_div))
  (export "factorial" (func $factorial))
  (export "fibonacci" (func $fibonacci))
  (export "increment" (func $increment))
  (export "reset" (func $reset))
  (export "get_counter" (func $get_counter))
  (export "memory" (memory $mem)))
