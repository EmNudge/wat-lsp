---
title: select
description: Choose between two stack values based on an i32 condition.
---

`select` pops three values — two candidates and a condition — and pushes the first if the condition is non-zero, otherwise the second. Both candidates must share the same type.

```wat
(module
  (func (param $x i32) (param $y i32) (param $cond i32) (result i32)
    (select (local.get $x) (local.get $y) (local.get $cond)))
)
```

Typed `select` annotates the result type explicitly: `(select (result i32) ...)`.

Useful as a branchless min/max building block or compact conditional expression.

## Instruction Reference

- [Parametric Instructions](/instructions/parametric) — `drop`, `select`
