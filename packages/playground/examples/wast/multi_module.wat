;; Multi-Module (WAST Format)
;; Multiple independent modules in a single file — each has its own index space

;; Module 1: Math utilities
(module
  (func $add (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  (func $mul (export "mul") (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b)))

  (func $square (export "square") (param $x i32) (result i32)
    (call $mul (local.get $x) (local.get $x)))
)

;; Module 2: String-like memory operations
;; Note: reuses $add name without conflict — separate module!
(module
  (memory (export "mem") 1)

  (func $add (param $offset i32) (param $val i32)
    (i32.store (local.get $offset) (local.get $val)))

  (func $get (export "get") (param $offset i32) (result i32)
    (i32.load (local.get $offset)))

  (func $fill (export "fill") (param $start i32) (param $len i32) (param $val i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (call $add
          (i32.add (local.get $start) (i32.mul (local.get $i) (i32.const 4)))
          (local.get $val))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))
)

;; Module 3: Counter with global state
;; Shares type and function names with previous modules — no errors
(module
  (type $binop (func (param i32 i32) (result i32)))

  (global $count (mut i32) (i32.const 0))

  (func $add (export "increment") (param $n i32) (result i32)
    (global.set $count
      (i32.add (global.get $count) (local.get $n)))
    (global.get $count))

  (func $get (export "get_count") (result i32)
    (global.get $count))

  (func $reset (export "reset") (result i32)
    (local $prev i32)
    (local.set $prev (global.get $count))
    (global.set $count (i32.const 0))
    (local.get $prev))
)
