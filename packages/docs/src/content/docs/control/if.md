---
title: 'if / else'
description: Conditional execution with optional result values.
---

`if` consumes an `i32` condition (0 = false, non-zero = true). Both branches must match result types when present.

```wat
(module
  (func (param $x i32) (result i32)
    (if (result i32) (i32.eqz (local.get $x))
      (then (i32.const 1))
      (else (i32.const 0))))
)
```

- Use `(result t)` to push a value from either branch.
- Omit `else` when the block produces no result.

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `block`, `loop`, `if`, `br`, `br_if`
