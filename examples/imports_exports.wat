;; Imports and Exports
;; Demonstrates various import/export patterns

(module
  ;; === Import Functions ===

  ;; Import console.log equivalent
  (import "env" "log" (func $log (param i32)))

  ;; Import console.log for floats
  (import "env" "logf" (func $logf (param f64)))

  ;; Import a function that returns a value
  (import "env" "get_time" (func $get_time (result f64)))

  ;; Import a binary math function
  (import "math" "pow" (func $pow (param f64 f64) (result f64)))

  ;; Import memory from host
  (import "env" "memory" (memory $shared_mem 1 10))

  ;; Import a global from host
  (import "env" "base_address" (global $base_addr i32))

  ;; Import a mutable global
  (import "env" "counter" (global $external_counter (mut i32)))

  ;; Import a table
  (import "env" "callback_table" (table $callbacks 4 funcref))

  ;; Import a tag for exceptions
  (import "env" "error_tag" (tag $imported_error (param i32)))

  ;; === Local Definitions ===

  ;; Local memory (separate from imported)
  (memory $local_mem 2)

  ;; Local table
  (table $local_fns 8 funcref)

  ;; Local globals
  (global $local_counter (mut i32) (i32.const 0))
  (global $pi f64 (f64.const 3.141592653589793))

  ;; Tag for local exceptions
  (tag $local_error (param i32 i32))

  ;; === Functions Using Imports ===

  ;; Use imported log
  (func $print_value (param $x i32)
    (call $log (local.get $x)))

  ;; Use imported math function
  (func $square_root (param $x f64) (result f64)
    (call $pow (local.get $x) (f64.const 0.5)))

  ;; Use imported memory
  (func $read_shared (param $offset i32) (result i32)
    (i32.load $shared_mem
      (i32.add (global.get $base_addr) (local.get $offset))))

  ;; Write to imported memory
  (func $write_shared (param $offset i32) (param $val i32)
    (i32.store $shared_mem
      (i32.add (global.get $base_addr) (local.get $offset))
      (local.get $val)))

  ;; Use imported mutable global
  (func $increment_external (result i32)
    (global.set $external_counter
      (i32.add (global.get $external_counter) (i32.const 1)))
    (global.get $external_counter))

  ;; Call through imported table
  (func $call_callback (param $idx i32) (param $arg i32) (result i32)
    (call_indirect $callbacks (type $callback_type)
      (local.get $arg)
      (local.get $idx)))

  (type $callback_type (func (param i32) (result i32)))

  ;; Throw imported exception
  (func $throw_imported_error (param $code i32)
    (throw $imported_error (local.get $code)))

  ;; === Local Functions to Export ===

  ;; Simple add function
  (func $add (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  ;; Multiply function
  (func $multiply (export "multiply") (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b)))

  ;; Function that uses local memory
  (func $store_local (export "store_local") (param $offset i32) (param $val i32)
    (i32.store $local_mem (local.get $offset) (local.get $val)))

  (func $load_local (export "load_local") (param $offset i32) (result i32)
    (i32.load $local_mem (local.get $offset)))

  ;; Process that uses both imported and local resources
  (func $process (export "process") (param $input i32) (result i32)
    (local $result i32)
    ;; Log input
    (call $log (local.get $input))
    ;; Square using imported pow
    (local.set $result
      (i32.trunc_f64_s
        (call $pow
          (f64.convert_i32_s (local.get $input))
          (f64.const 2.0))))
    ;; Increment local counter
    (global.set $local_counter
      (i32.add (global.get $local_counter) (i32.const 1)))
    ;; Log result
    (call $log (local.get $result))
    (local.get $result))

  ;; Get local counter
  (func $get_count (export "get_count") (result i32)
    (global.get $local_counter))

  ;; Get pi value
  (func $get_pi (export "get_pi") (result f64)
    (global.get $pi))

  ;; Compute circle area
  (func $circle_area (export "circle_area") (param $radius f64) (result f64)
    (f64.mul
      (global.get $pi)
      (call $pow (local.get $radius) (f64.const 2.0))))

  ;; Initialize local table
  (elem (table $local_fns) (i32.const 0) func $add $multiply)

  ;; Call local table function
  (func $call_local_fn (export "call_local_fn") (param $idx i32) (param $a i32) (param $b i32) (result i32)
    (call_indirect $local_fns (type $binary_type)
      (local.get $a)
      (local.get $b)
      (local.get $idx)))

  (type $binary_type (func (param i32 i32) (result i32)))

  ;; Function that handles both exception types
  (func $safe_operation (export "safe_operation") (param $x i32) (result i32)
    (return (i32.mul (local.get $x) (i32.const 2))))

  ;; === Additional Exports ===

  ;; Export local memory
  (export "local_memory" (memory $local_mem))

  ;; Export local table
  (export "local_functions" (table $local_fns))

  ;; Export local global
  (export "counter" (global $local_counter))

  ;; Export pi constant
  (export "PI" (global $pi))

  ;; Export local error tag
  (export "error" (tag $local_error))

  ;; Re-export imported function under different name
  (export "log_value" (func $log))

  ;; Export helper functions
  (export "print_value" (func $print_value))
  (export "square_root" (func $square_root))
  (export "increment_external" (func $increment_external)))
