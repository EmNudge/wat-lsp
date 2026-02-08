;; Reference Expressions
;; Demonstrates ref.null, ref.func, and ref.is_null in various contexts,
;; including elem segment expression-form initializers (ref.func $id / ref.null func).

(module
  ;; Types
  (type $i32_to_i32 (func (param i32) (result i32)))

  ;; Functions to reference
  (func $inc (param $x i32) (result i32)
    (i32.add (local.get $x) (i32.const 1)))

  (func $dec (param $x i32) (result i32)
    (i32.sub (local.get $x) (i32.const 1)))

  (func $negate (param $x i32) (result i32)
    (i32.sub (i32.const 0) (local.get $x)))

  ;; Table with funcref
  (table $ops 4 funcref)

  ;; === Elem with expression-form initializers ===
  ;; Each element is a (ref.func ...) or (ref.null ...) expression
  (elem (table $ops) (i32.const 0) funcref
    (ref.func $inc)
    (ref.func $dec)
    (ref.func $negate)
    (ref.null func))

  ;; === ref.func: obtain a function reference ===
  (func $get_inc (export "get_inc") (result funcref)
    ref.func $inc)

  (func $get_dec (export "get_dec") (result funcref)
    ref.func $dec)

  ;; === ref.null: create a null reference ===
  ;; Using heap types (spec-correct form)
  (func $null_func (export "null_func") (result funcref)
    ref.null func)

  (func $null_extern (export "null_extern") (result externref)
    ref.null extern)

  ;; === ref.is_null: test for null ===
  (func $check_null (export "check_null") (param $r funcref) (result i32)
    local.get $r
    ref.is_null)

  ;; Folded form
  (func $check_null_folded (export "check_null_folded") (param $r funcref) (result i32)
    (ref.is_null (local.get $r)))

  ;; === Practical pattern: safe indirect call ===
  ;; Call a function from the table if present, otherwise return a default
  (func $safe_call (export "safe_call") (param $idx i32) (param $arg i32) (result i32)
    (if (result i32) (ref.is_null (table.get $ops (local.get $idx)))
      (then
        (i32.const -1))
      (else
        (call_indirect $ops (type $i32_to_i32)
          (local.get $arg)
          (local.get $idx)))))

  ;; === Global references ===
  (global $g_func (mut funcref) (ref.null func))
  (global $g_extern (mut externref) (ref.null extern))

  ;; Store and retrieve function references via globals
  (func $set_global_fn (export "set_global_fn") (param $fn funcref)
    (global.set $g_func (local.get $fn)))

  (func $get_global_fn (export "get_global_fn") (result funcref)
    (global.get $g_func))

  (func $has_global_fn (export "has_global_fn") (result i32)
    (i32.eqz (ref.is_null (global.get $g_func))))

  ;; === Passive elem segment with ref expressions ===
  (elem $passive funcref
    (ref.func $inc)
    (ref.func $dec)
    (ref.null func))
)
