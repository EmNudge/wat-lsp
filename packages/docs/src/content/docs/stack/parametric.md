---
title: Parametric Instructions
description: 'Stack-level utilities: drop and select for branchless control.'
---

Parametric instructions operate directly on the value stack.

## drop

Pop and discard the top stack value.

```wat
(module
  (func (param $x i32) (result i32)
    (drop (i32.const 123))
    (local.get $x))
)
```

Use `drop` to ignore an unused result or clean up the stack before producing a block result.

## select

Choose between two values based on an `i32` condition (non-zero = first value):

```wat
(module
  (func (param $a i32) (param $b i32) (param $cond i32) (result i32)
    (select (local.get $a) (local.get $b) (local.get $cond)))
)
```

Both candidate values must have the same type.

## Instruction Reference

- [Parametric Instructions](/instructions/parametric) — `drop`, `select`
