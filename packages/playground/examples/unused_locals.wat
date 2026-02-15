;; Unused Locals
;; Demonstrates detection of unused local variables and parameters

(module
  ;; This function has an unused local — the LSP flags it as a warning
  (func $with_unused_local (param $x i32) (result i32)
    (local $temp i32)       ;; used below
    (local $forgotten i32)  ;; UNUSED — warning: "Unused local $forgotten"

    (local.set $temp (i32.mul (local.get $x) (i32.const 2)))
    (local.get $temp))

  ;; This function has an unused parameter — the LSP flags it as a hint
  (func $callback (param $ctx i32) (param $value i32) (result i32)
    ;; $ctx is never read — hint: "Unused parameter $ctx"
    (i32.add (local.get $value) (i32.const 1)))

  ;; All locals and parameters are used — no warnings
  (func $swap (param $a i32) (param $b i32) (result i32 i32)
    (local $tmp i32)
    (local.set $tmp (local.get $a))
    (local.get $b)
    (local.get $tmp))

  ;; Numeric indices also count as usage
  (func $numeric (param $p i32) (result i32)
    (local.get 0))  ;; index 0 references $p — no warning

  ;; local.set and local.tee both count as usage
  (func $set_and_tee (param $input i32) (result i32)
    (local $via_set i32)
    (local $via_tee i32)
    (i32.const 10)
    (local.set $via_set)
    (i32.const 20)
    (local.tee $via_tee)
    (drop)
    (i32.add (local.get $via_set) (local.get $via_tee)))

  (export "with_unused_local" (func $with_unused_local))
  (export "callback" (func $callback))
  (export "swap" (func $swap)))
