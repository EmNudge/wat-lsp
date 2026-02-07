---
title: Constants
description: i32.const, i64.const, f32.const, f64.const and using immediates.
---

Push literal values onto the stack.

```wat
(module
  (func (export "consts") (result i32)
    (i32.const 42))
)
```

Hex and float literals:

```wat
(module
  (func
    (drop (i32.const 0xFF))
    (drop (f64.const 3.14159)))
)
```

Also: `i64.const`, `f32.const`.

## Instruction Reference

- [i32](/instructions/i32#i32const), [i64](/instructions/i64#i64const), [f32](/instructions/f32#f32const), [f64](/instructions/f64#f64const)
