---
title: 'br & br_if'
description: Structured branches to labeled blocks and loops.
---

`br` performs an unconditional jump to the end of a `block` or to the top of a `loop`. `br_if` pops a condition and branches if non-zero.

```wat
(module
  (func (param $n i32) (result i32)
    (block $exit (result i32)
      (br_if $exit (i32.gt_s (local.get $n) (i32.const 10)))
      (i32.const 1)
      (br $exit)))
)
```

Relative labels — `br 0` targets the innermost label, `br 1` jumps one level out:

```wat
(module
  (func (result i32)
    (block (result i32)
      (block (result i32)
        (i32.const 42)
        (br 1))))
)
```

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `block`, `loop`, `if`, `br`, `br_if`
