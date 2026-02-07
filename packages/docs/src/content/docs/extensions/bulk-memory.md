---
title: 'Bulk Memory Operations'
description: 'Efficient copying and filling for memory and tables.'
---

Bulk memory adds fast, in-engine operations for copying and filling regions of memory and tables.

## memory.copy and memory.fill

```wat
(module
  (memory (export "memory") 1)
  (func (export "copy") (param $dst i32) (param $src i32) (param $len i32)
    (memory.copy (local.get $dst) (local.get $src) (local.get $len)))
  (func (export "fill") (param $dst i32) (param $value i32) (param $len i32)
    (memory.fill (local.get $dst) (local.get $value) (local.get $len)))
)
```

## table.copy, table.fill

```wat
(module
  (table (export "table") 10 funcref)
  (func (export "tfill") (param $idx i32) (param $len i32)
    (table.fill 0 (local.get $idx) (ref.null func) (local.get $len)))
)
```

## Instruction Reference

- [Memory Instructions](/instructions/memory) — `memory.copy`, `memory.fill`, `memory.init`, `data.drop`
- [Table Instructions](/instructions/table) — `table.copy`, `table.fill`, `table.init`, `elem.drop`
