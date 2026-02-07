---
title: 'Bitwise & Shifts'
description: and, or, xor, shifts, and rotates for integers.
---

Bitwise ops are for integers only.

```wat
(module
  (func (export "bits") (param $x i32) (param $y i32) (result i32)
    (i32.and (local.get $x) (local.get $y)))
)
```

Also: `i32.or`, `i32.xor`.

Shifts and rotates:

```wat
(module
  (func (export "shifts") (param $x i32) (param $amt i32) (result i32)
    (i32.shl (local.get $x) (local.get $amt)))
)
```

Also: `i32.shr_s`, `i32.shr_u`, `i32.rotl`, `i32.rotr` and all `i64.*` variants.

## Instruction Reference

- [i32 Instructions](/instructions/i32), [i64 Instructions](/instructions/i64)
