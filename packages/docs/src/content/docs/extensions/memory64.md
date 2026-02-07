---
title: 'Memory64'
description: '64-bit memory and table addressing for large linear memories.'
---

Memory64 allows memories and tables to use `i64` indices instead of `i32`, expanding the addressable space from 4 GiB to 16 EiB.

```wat-snippet
(module
  ;; 64-bit addressed memory (1 page minimum)
  (memory $mem i64 1)

  (func (export "load64") (param $addr i64) (result i32)
    (i32.load (local.get $addr)))

  (func (export "size") (result i64)
    (memory.size))
)
```

When used with an `i64`-addressed memory:

- Memory instructions (`i32.load`, `i64.store`, etc.) accept `i64` addresses
- `memory.size` and `memory.grow` return and accept `i64` values
- Bulk memory operations (`memory.copy`, `memory.fill`, `memory.init`) use `i64` for addresses and lengths
- Tables also support `i64` indexing: `(table $tbl i64 10 funcref)`

## Instruction Reference

- [Memory Instructions](/instructions/memory) — `i32.load`, `i32.store`, `memory.size`, `memory.grow`, `memory.copy`, `memory.fill`
- [Table Instructions](/instructions/table) — `table.get`, `table.set`, `table.size`, `table.grow`
