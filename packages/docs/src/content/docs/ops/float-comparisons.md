---
title: Float Comparisons
description: f32/f64 eq, ne, lt/le/gt/ge with NaN semantics.
---

Float comparisons push `i32` booleans. NaN makes `eq` false and `ne` true; all other comparisons with NaN return false.

```wat
(module
  (func (export "fcmp") (param $x f32) (param $y f32) (result i32)
    (f32.le (local.get $x) (local.get $y)))
)
```

Also: `f32.eq`, `f32.ne`, `f32.lt`, `f32.gt`, `f32.ge` and all `f64.*` variants.

## Instruction Reference

- [f32 Instructions](/instructions/f32), [f64 Instructions](/instructions/f64)
