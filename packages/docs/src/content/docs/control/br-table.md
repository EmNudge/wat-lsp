---
title: br_table
description: Multi-way branch (switch) over an index with a default.
---

`br_table` pops an `i32` index and branches to one of several labels. Out-of-range values fall through to a default label. Useful for `switch`-like dispatch.

```wat
(module
  (func (param $tag i32) (result i32)
    (block $default (result i32)
      (block $case2 (result i32)
        (block $case1 (result i32)
          (block $case0 (result i32)
            (br_table $case0 $case1 $case2 $default
              (local.get $tag)))
          (return (i32.const 10)))
        (return (i32.const 20)))
      (return (i32.const 30)))
    (i32.const -1))
)
```

- Label order matches indices 0..n-1, then a final default.
- Each target must produce the block's result type(s).

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `block`, `loop`, `if`, `br`, `br_if`, `br_table`
