---
title: Float Unary Ops
description: abs, neg, sqrt, ceil, floor, trunc, nearest, min, max, copysign.
---

```wat
(module
  (func (export "rounding") (param $x f32) (result f32)
    (f32.ceil (local.get $x)))
)
```

Also: `f32.floor`, `f32.trunc`, `f32.nearest`, `f32.abs`, `f32.neg`, `f32.sqrt`, `f32.min`, `f32.max`, `f32.copysign` and all `f64.*` variants.

## Instruction Reference

- [f32 Instructions](/instructions/f32), [f64 Instructions](/instructions/f64)
