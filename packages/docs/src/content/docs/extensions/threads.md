---
title: 'Threads & Atomics'
description: 'Shared memory and atomic operations for multi-threaded WebAssembly.'
---

The threads proposal adds shared linear memory and atomic operations to WebAssembly, enabling safe multi-threaded programming. Memory is declared with the `shared` keyword so it can be accessed from multiple threads via `SharedArrayBuffer` on the host side. All atomic operations provide sequential consistency and require naturally aligned addresses (e.g., 4-byte alignment for `i32` operations). Thread synchronization is supported through `memory.atomic.wait32`/`memory.atomic.wait64` to suspend a thread until a memory location changes, and `memory.atomic.notify` to wake waiting threads. An `atomic.fence` instruction ensures memory ordering between atomic and non-atomic accesses.

```wat
(module
  ;; Shared memory: 1 page minimum, 4 pages maximum
  (memory (export "memory") 1 4 shared)

  ;; Atomically increment a counter at address 0, return the old value
  (func (export "atomicAdd") (param $value i32) (result i32)
    (i32.atomic.rmw.add
      (i32.const 0)           ;; address of counter
      (local.get $value)))    ;; value to add

  ;; Wait for a signal at address 0, then notify other threads
  (func (export "waitAndNotify") (param $addr i32) (result i32)
    ;; Wait until the value at $addr is no longer 0
    (drop
      (memory.atomic.wait32
        (local.get $addr)     ;; address to wait on
        (i32.const 0)         ;; expected value (wait while equal)
        (i64.const -1)))      ;; timeout (-1 = infinite)
    ;; Wake up to 1 other thread waiting on this address
    (memory.atomic.notify
      (local.get $addr)       ;; address to notify
      (i32.const 1)))         ;; max waiters to wake
)
```

## Instruction Reference

- [Atomic Instructions](/instructions/atomic) — all atomic loads, stores, RMW operations, wait/notify, fence
