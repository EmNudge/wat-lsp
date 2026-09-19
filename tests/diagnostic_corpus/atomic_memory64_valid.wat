;; On a memory64 module the atomic address operand is i64, not i32.
;; Regression test: atomic rmw/wait/notify previously hardcoded an i32 address
;; and produced false type-mismatch errors here.
(module
  (memory i64 1 1 shared)

  (func $rmw (param $addr i64) (param $val i32) (result i32)
    local.get $addr
    local.get $val
    i32.atomic.rmw.add)

  (func $cmpxchg (param $addr i64) (param $e i64) (param $r i64) (result i64)
    local.get $addr
    local.get $e
    local.get $r
    i64.atomic.rmw.cmpxchg)

  (func $wait (param $addr i64) (param $e i32) (param $t i64) (result i32)
    local.get $addr
    local.get $e
    local.get $t
    memory.atomic.wait32)

  (func $notify (param $addr i64) (param $c i32) (result i32)
    local.get $addr
    local.get $c
    memory.atomic.notify))
