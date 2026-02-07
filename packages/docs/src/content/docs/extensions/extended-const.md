---
title: 'Extended Constant Expressions'
description: 'Richer constant initializers for globals, tables, and element/data offsets.'
---

Extended const expressions allow more forms in places that require constants, like global initializers and element/data offsets.

## global.get in initializers

```wat
(module
  (global $base i32 (i32.const 100))
  (global (export "copy_from") i32 (global.get $base))
)
```

## ref.func and ref.null in element segments

```wat
(module
  (type $t0 (func (param i32) (result i32)))
  (func $id (type $t0) (param $x i32) (result i32)
    (local.get $x))

  (table 2 funcref)
  (elem (i32.const 0) func $id)
)
```

## Instruction Reference

- [Local & Global Instructions](/instructions/local-global) — `global.get`
- [Reference Instructions](/instructions/reference) — `ref.func`, `ref.null`
- [Module Structure](/instructions/module) — `global`, `elem`, `data`
