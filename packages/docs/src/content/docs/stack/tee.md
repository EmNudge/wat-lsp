---
title: local.tee
description: Store a value into a local and keep it on the stack.
---

`local.tee` writes to a local variable and leaves the value on the stack for further use.

```wat
(module
  (func (param $x i32) (result i32)
    (local $y i32)
    (i32.add
      (local.tee $y (local.get $x))
      (i32.const 1)))
)
```

There is no `global.tee`. To set a global and keep the value, use `local.tee` into a temporary then `global.set`:

```wat
(module
  (global $g (mut i32) (i32.const 0))
  (func (param $x i32) (result i32)
    (local $tmp i32)
    (global.set $g (local.tee $tmp (local.get $x)))
    (i32.mul (local.get $tmp) (i32.const 2)))
)
```

## Instruction Reference

- [Local & Global Instructions](/instructions/local-global) — `local.get`, `local.set`, `local.tee`, `global.get`, `global.set`
