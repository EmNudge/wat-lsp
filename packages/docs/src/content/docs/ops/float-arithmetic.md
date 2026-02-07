---
title: Float Arithmetic
description: f32/f64 add, sub, mul, div with IEEE 754 semantics.
---

Floats follow IEEE 754: NaNs propagate and division by zero yields ±Inf (no trap).

```wat
(module
  (func (export "fops") (param $x f64) (param $y f64) (result f64)
    (f64.mul (local.get $x) (local.get $y)))
)
```

Also: `f64.add`, `f64.sub`, `f64.div` and all `f32.*` variants.

## Instruction Reference

- [f32 Instructions](/instructions/f32), [f64 Instructions](/instructions/f64)
