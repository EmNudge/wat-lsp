---
title: 'Multi-value'
description: 'Functions and blocks can produce multiple results; blocks can take parameters.'
---

## Functions returning multiple values

```wat
(module
  (func (export "pair") (param $x i32) (result i32 i32)
    (local.get $x)
    (i32.const 1))
)
```

## Blocks with params and results

```wat
(module
  (func (param $a i32) (param $b i32) (result i32)
    (local.get $a)
    (local.get $b)
    (block (param i32 i32) (result i32)
      (i32.add)))
)
```

## Instruction Reference

- [Module Structure](/instructions/module) — `func`, `param`, `result`
- [Control Flow Instructions](/instructions/control) — `block`, `loop`, `if` with multiple results
