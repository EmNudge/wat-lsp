---
title: 'Memory & Data'
description: Linear memory, loads/stores, memory size/grow, and data segments.
---

WebAssembly exposes a contiguous byte array called linear memory. You interact with it via typed load/store instructions and can initialize it with data segments.

## Declaring and exporting memory

```wat
(module
  (memory (export "memory") 1 4)  ;; min 1 page (64 KiB), max 4
)
```

## Loads and stores

Typed operations read/write values at byte offsets. Narrow variants sign/zero-extend on load and truncate on store.

```wat
(module
  (memory 1)
  (func (param $ptr i32) (result i32)
    (i32.store8 (local.get $ptr) (i32.const 0x7F))
    (i32.load8_u (local.get $ptr)))
)
```

Common ops:

- **Loads**: `i32.load`, `i64.load`, `f32.load`, `f64.load`, `i32.load8_s/u`, `i32.load16_s/u`, ...
- **Stores**: `i32.store`, `i64.store`, `f32.store`, `f64.store`, `i32.store8`, `i32.store16`, ...

## memory.size and memory.grow

```wat
(module
  (memory 1)
  (func (result i32)
    (memory.size))
  (func (param $pages i32) (result i32)
    (memory.grow (local.get $pages)))  ;; returns old size, or -1 on failure
)
```

After growth the underlying `ArrayBuffer` can change — recreate typed array views on the host.

## Data segments

```wat
(module
  (memory 1)
  (data (i32.const 0) "Hi")
)
```

## Instruction Reference

- [Memory Instructions](/instructions/memory) — `memory.size`, `memory.grow`, `memory.fill`, `memory.copy`
- [i32](/instructions/i32), [i64](/instructions/i64), [f32](/instructions/f32), [f64](/instructions/f64) — load/store variants
