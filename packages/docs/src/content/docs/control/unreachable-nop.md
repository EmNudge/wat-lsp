---
title: 'unreachable & nop'
description: Trap immediately, or do nothing.
---

- `unreachable` — traps when executed. Useful for asserting impossible states.
- `nop` — does nothing. Occasionally useful as a placeholder.

```wat
(module
  (func (param $x i32)
    (if (i32.lt_s (local.get $x) (i32.const 0))
      (then (unreachable))
      (else (nop))))
)
```

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `unreachable`, `nop`
