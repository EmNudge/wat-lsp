---
title: Integer Comparisons
description: eq, ne, lt/le/gt/ge with signed/unsigned variants.
---

Comparisons push `i32` booleans (0 = false, 1 = true).

```wat
(module
  (func (export "cmp") (param $x i32) (param $y i32) (result i32)
    (i32.lt_s (local.get $x) (local.get $y)))
)
```

- **Signed**: `lt_s`, `le_s`, `gt_s`, `ge_s`
- **Unsigned**: `lt_u`, `le_u`, `gt_u`, `ge_u`
- **Equality**: `eq`, `ne`
- All available as `i64.*` variants.

## Instruction Reference

- [i32 Instructions](/instructions/i32), [i64 Instructions](/instructions/i64)
