---
title: drop
description: Discard the top-of-stack value.
---

`drop` removes the top value from the stack.

```wat
(module
  (func (result i32)
    (drop (i32.const 20))
    (i32.const 10))
)
```

Use `drop` to balance unwanted values — for example, when an instruction pushes a result you don't need.

## Instruction Reference

- [Parametric Instructions](/instructions/parametric) — `drop`, `select`
