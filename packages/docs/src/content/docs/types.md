---
title: Types
description: Understand WAT value types, function signatures, locals, and results.
---

WAT's type system is intentionally small. Most modules only need a handful of core types.

## Value types

Number types:

- `i32` and `i64`: 32-bit and 64-bit integers. Signed/unsigned use the same type — the operator determines interpretation.
- `f32` and `f64`: 32-bit and 64-bit IEEE 754 floats.

Reference types:

- `funcref`: reference to a function.
- `externref`: opaque reference to a host value (e.g. a JS object).

## Function types

Functions declare parameter and result types. You can name parameters for readability.

```wat
(module
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  (func $mix (param $x i64) (param $y f32) (result f64)
    (f64.add
      (f64.convert_i64_s (local.get $x))
      (f64.promote_f32 (local.get $y))))
)
```

- Multiple results are supported: `(result i32 i64)` pushes two values.
- Use `local` to declare function-local variables: `(local $acc f64)`.

## Blocks and block types

Control blocks (`block`, `loop`, `if`) can declare result types, which affect stack typing.

```wat
(module
  (func (param $n i32) (result i32)
    (if (result i32) (i32.eqz (local.get $n))
      (then (i32.const 42))
      (else (i32.const 7))))
)
```

Both branches of an `if` with a result must produce the same typed value(s).

## Tables, memories, and globals

- **Tables**: `funcref` or `externref` elements with limits, e.g. `(table 1 funcref)` or `(table 1 10 funcref)`.
- **Memories**: linear byte arrays sized in 64 KiB pages, e.g. `(memory 1)` or `(memory 1 4)`.
- **Globals**: typed, optionally mutable values, e.g. `(global (mut i32) (i32.const 0))`.

```wat
(module
  (table $t 1 funcref)
  (memory $mem 1)
  (global $counter (mut i32) (i32.const 0))
)
```

## Instruction Reference

- [Type Names](/instructions/types) — all value type keywords
- [Module Structure](/instructions/module) — `func`, `param`, `result`, `local`, `global`, `table`, `memory`
