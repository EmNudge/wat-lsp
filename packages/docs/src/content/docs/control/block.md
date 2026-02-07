---
title: block
description: Structured block with optional result type and label targets.
---

`block` creates a labeled scope. Branching to its label exits the block. It can optionally produce results.

```wat
(module
  (func (param $n i32) (result i32)
    (block $out (result i32)
      (br_if $out (i32.eqz (local.get $n)))
      (i32.const 42)))
)
```

- Label names are optional — you can also target blocks by relative depth index.
- A block with `(result ...)` must leave that typed value on the stack on all exits.

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `block`, `loop`, `if`, `br`, `br_if`
