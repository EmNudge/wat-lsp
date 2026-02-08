;; Valid atomic operations with shared memory - should produce no diagnostics
;; Regression test for threads proposal instruction arity
(module
  (memory 1 10 shared)

  ;; Atomic loads
  (func $atomic_load_i32 (param $addr i32) (result i32)
    local.get $addr
    i32.atomic.load)

  (func $atomic_load_i64 (param $addr i32) (result i64)
    local.get $addr
    i64.atomic.load)

  (func $atomic_load8_u (param $addr i32) (result i32)
    local.get $addr
    i32.atomic.load8_u)

  ;; Atomic stores
  (func $atomic_store_i32 (param $addr i32) (param $val i32)
    local.get $addr
    local.get $val
    i32.atomic.store)

  (func $atomic_store_i64 (param $addr i32) (param $val i64)
    local.get $addr
    local.get $val
    i64.atomic.store)

  ;; Atomic RMW
  (func $atomic_rmw_add (param $addr i32) (param $val i32) (result i32)
    local.get $addr
    local.get $val
    i32.atomic.rmw.add)

  (func $atomic_rmw_xchg (param $addr i32) (param $val i64) (result i64)
    local.get $addr
    local.get $val
    i64.atomic.rmw.xchg)

  (func $atomic_rmw8_sub_u (param $addr i32) (param $val i32) (result i32)
    local.get $addr
    local.get $val
    i32.atomic.rmw8.sub_u)

  ;; Atomic cmpxchg
  (func $atomic_cmpxchg (param $addr i32) (param $expected i32) (param $replacement i32) (result i32)
    local.get $addr
    local.get $expected
    local.get $replacement
    i32.atomic.rmw.cmpxchg)

  ;; Wait/notify
  (func $atomic_wait32 (param $addr i32) (param $expected i32) (param $timeout i64) (result i32)
    local.get $addr
    local.get $expected
    local.get $timeout
    memory.atomic.wait32)

  (func $atomic_notify (param $addr i32) (param $count i32) (result i32)
    local.get $addr
    local.get $count
    memory.atomic.notify)

  ;; Atomic fence
  (func $fence
    atomic.fence)
)
