;; Atomic accesses must use exactly their natural alignment. Every form below
;; is at its natural alignment, so no alignment diagnostics should be produced.
(module
  (memory 1 1 shared)

  (func $ok (param $a i32) (param $v32 i32) (param $v64 i64) (result i32)
    ;; full-width load/store (i32 -> 4, i64 -> 8)
    local.get $a
    i32.atomic.load align=4
    drop
    local.get $a
    local.get $v64
    i64.atomic.store align=8

    ;; sub-width forms
    local.get $a
    i32.atomic.load8_u align=1
    drop
    local.get $a
    i32.atomic.load16_u align=2
    drop
    local.get $a
    local.get $v64
    i64.atomic.store32 align=4

    ;; read-modify-write, full and sub width
    local.get $a
    local.get $v32
    i32.atomic.rmw.add align=4
    drop
    local.get $a
    local.get $v32
    i32.atomic.rmw16.xchg_u align=2
    drop

    ;; wait/notify
    local.get $a
    i32.const 0
    i64.const 0
    memory.atomic.wait32 align=4
    drop
    local.get $a
    i32.const 0
    memory.atomic.notify align=4))
