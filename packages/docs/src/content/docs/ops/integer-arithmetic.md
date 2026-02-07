---
title: Integer Arithmetic
description: i32/i64 add, sub, mul, div, rem and typical usage.
---

Integer instructions wrap on overflow (two's complement). Division by zero traps.

## add, sub, mul

```wat
(module
  (func (export "ops") (param $x i32) (param $y i32) (result i32)
    (i32.add (local.get $x) (local.get $y)))
)
```

Also: `i32.sub`, `i32.mul` and all `i64.*` variants.

## div and rem

Signed vs unsigned matters for negatives:

```wat
(module
  (func (export "divrem") (param $x i32) (param $y i32) (result i32)
    (i32.div_s (local.get $x) (local.get $y)))
)
```

Also: `i32.div_u`, `i32.rem_s`, `i32.rem_u` and `i64.*` variants.

## Instruction Reference

- [i32 Instructions](/instructions/i32), [i64 Instructions](/instructions/i64)
