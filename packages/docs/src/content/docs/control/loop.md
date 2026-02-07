---
title: loop
description: Repeat a block; branches target the top to continue.
---

Branching to a `loop` label jumps back to the top, like `continue` in other languages. Combine with an outer `block` for a clean break target.

```wat
(module
  (func (param $n i32) (result i32)
    (local $acc i32)
    (block $done
      (loop $again
        (br_if $done (i32.eqz (local.get $n)))
        (local.set $acc (i32.add (local.get $acc) (i32.const 1)))
        (local.set $n (i32.sub (local.get $n) (i32.const 1)))
        (br $again)))
    (local.get $acc))
)
```

- `br $again` jumps to the top of the loop.
- `br_if $done` exits the outer block when `$n` reaches zero.

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `block`, `loop`, `br`, `br_if`
