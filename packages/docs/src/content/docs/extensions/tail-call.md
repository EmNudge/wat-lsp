---
title: 'Tail Calls'
description: 'return_call and return_call_indirect for optimized recursion.'
---

Tail calls let a function transfer control to another function as its final action without growing the stack.

```wat
(module
  (func $f (param $n i32) (result i32)
    (if (result i32) (i32.eqz (local.get $n))
      (then (i32.const 0))
      (else (return_call $f (i32.sub (local.get $n) (i32.const 1))))))
)
```

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `return_call`, `return_call_indirect`
