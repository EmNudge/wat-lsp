---
title: 'Non-trapping float-to-int'
description: 'Saturating conversions from floats to integers that do not trap.'
---

Instead of trapping on overflow or NaN, these instructions clamp to min/max and convert NaN to 0.

```wat
(module
  (func (export "sat") (param $x f32) (result i32)
    (i32.trunc_sat_f32_s (local.get $x)))
)
```

All variants: `i32.trunc_sat_f32_s/u`, `i32.trunc_sat_f64_s/u`, `i64.trunc_sat_f32_s/u`, `i64.trunc_sat_f64_s/u`.

## Instruction Reference

- [i32 Instructions](/instructions/i32), [i64 Instructions](/instructions/i64)
