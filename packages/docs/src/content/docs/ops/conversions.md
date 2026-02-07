---
title: Conversions
description: Int/float conversions, extend/wrap, promote/demote, trunc/convert.
---

```wat
(module
  (func (export "mix") (param $i i32) (param $f f32) (result f64)
    (f64.add
      (f64.convert_i32_s (local.get $i))
      (f64.promote_f32 (local.get $f))))
)
```

Conversion families:

- **Int width**: `i64.extend_i32_s`, `i64.extend_i32_u`, `i32.wrap_i64`
- **Float width**: `f64.promote_f32`, `f32.demote_f64`
- **Float → int**: `i32.trunc_f32_s/u`, `i32.trunc_f64_s/u`, `i64.trunc_f32_s/u`, `i64.trunc_f64_s/u`
- **Int → float**: `f32.convert_i32_s/u`, `f32.convert_i64_s/u`, `f64.convert_i32_s/u`, `f64.convert_i64_s/u`

See also [Non-trapping float-to-int](/extensions/nontrapping-f2i/) for saturating variants.

## Instruction Reference

- [i32](/instructions/i32), [i64](/instructions/i64), [f32](/instructions/f32), [f64](/instructions/f64)
