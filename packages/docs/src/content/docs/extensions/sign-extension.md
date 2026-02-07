---
title: 'Sign-extension Operators'
description: 'Extend smaller integer widths to larger ones with sign preservation.'
---

These operators widen integers while preserving the sign of the smaller type.

```wat
(module
  (func (export "ext8") (param $x i32) (result i32)
    (i32.extend8_s (local.get $x)))

  (func (export "ext16") (param $x i32) (result i32)
    (i32.extend16_s (local.get $x)))

  (func (export "ext32") (param $x i64) (result i64)
    (i64.extend32_s (local.get $x)))
)
```

## Instruction Reference

- [i32 Instructions](/instructions/i32) — `i32.extend8_s`, `i32.extend16_s`
- [i64 Instructions](/instructions/i64) — `i64.extend8_s`, `i64.extend16_s`, `i64.extend32_s`
