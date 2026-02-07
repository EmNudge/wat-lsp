---
title: return
description: Exit the current function, optionally pushing results.
---

`return` exits the current function immediately. Any required result values must already be on the stack.

```wat
(module
  (func (param $x i32) (result i32)
    (if (i32.eqz (local.get $x))
      (then (return (i32.const 0))))
    (i32.const 1))
)
```

Multi-value:

```wat
(module
  (func (param $a i32) (param $b i64) (result i32 i64)
    (return (local.get $a) (local.get $b)))
)
```

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `return`, `return_call`
