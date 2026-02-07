---
title: 'Exception Handling'
description: 'try_table/throw for native exception support in Wasm.'
---

Exception handling introduces `try_table`, `throw`, and related constructs for structured error handling.

```wat
(module
  (tag $e (param i32))

  (func (export "mayThrow") (param $x i32) (result i32)
    (block $handler (result i32)
      (try_table (catch $e $handler)
        (if (i32.eqz (local.get $x))
          (then (throw $e (i32.const 1))))
        (i32.const 0)
      )
    )
  )
)
```

## Instruction Reference

- [Exception Instructions](/instructions/exceptions) — `throw`, `throw_ref`, `tag`, `try_table`, `catch`, `catch_all`
