---
title: Traps
description: When and how execution traps (errors) occur in WebAssembly.
---

A trap is a runtime error that aborts execution. Common sources:

- `unreachable` executed
- Integer division by zero
- Signed overflow: `i32.div_s` with `INT_MIN / -1`
- `call_indirect` type mismatch or out-of-bounds table index
- Out-of-bounds memory access

## unreachable

```wat
(module
  (func (param $x i32)
    (if (i32.lt_s (local.get $x) (i32.const 0))
      (then (unreachable))))
)
```

## Memory out-of-bounds

```wat
(module
  (memory 1)
  (func
    (drop (i32.load (i32.const 70000))))  ;; > 64 KiB — traps
)
```

## call_indirect mismatch

```wat
(module
  (type $t0 (func (param i32) (result i32)))
  (func $f0 (type $t0) (param $x i32) (result i32)
    (local.get $x))
  (table 1 funcref)
  (elem (i32.const 0) $f0)
  (func (param $x i32) (result i32)
    (call_indirect (type $t0) (local.get $x) (i32.const 0)))
)
```

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `unreachable`, `call_indirect`
- [i32 Instructions](/instructions/i32) — `i32.div_s`, `i32.load`, `i32.store`
