;; Atomic memory instructions accept an optional offset=/align= memarg.
;; Regression test: memargs on atomics previously produced syntax ERROR nodes.
(module
  (memory 1 1 shared)

  (func $load (param $addr i32) (result i32)
    local.get $addr
    i32.atomic.load offset=0 align=4)

  (func $store (param $addr i32) (param $val i64)
    local.get $addr
    local.get $val
    i64.atomic.store offset=8 align=8)

  (func $rmw (param $addr i32) (param $val i32) (result i32)
    local.get $addr
    local.get $val
    i32.atomic.rmw.add offset=4 align=4)

  (func $rmw8 (param $addr i32) (param $val i32) (result i32)
    local.get $addr
    local.get $val
    i32.atomic.rmw8.add_u align=1)

  (func $cmpxchg (param $addr i32) (param $e i32) (param $r i32) (result i32)
    local.get $addr
    local.get $e
    local.get $r
    i32.atomic.rmw.cmpxchg offset=16)

  (func $wait (param $addr i32) (param $e i32) (param $t i64) (result i32)
    local.get $addr
    local.get $e
    local.get $t
    memory.atomic.wait32 offset=0 align=4)

  (func $notify (param $addr i32) (param $c i32) (result i32)
    local.get $addr
    local.get $c
    memory.atomic.notify offset=0 align=4))
