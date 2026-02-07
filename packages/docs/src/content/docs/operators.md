---
title: Operators
description: The essential WAT instructions for arithmetic, comparison, locals, and control flow.
---

WebAssembly is a stack machine. Most instructions pop operands from the stack and push results back.

## Reading and writing locals

```wat
(module
  (func (param $a i32) (param $b i32) (result i32)
    (local $tmp i32)
    (local.set $tmp (i32.add (local.get $a) (local.get $b)))
    (local.get $tmp))
)
```

- `local.get $x` — push the value of local `$x`.
- `local.set $x` — pop and store into local `$x`.
- `local.tee $x` — like set, but also keeps the value on the stack.

## Numeric arithmetic

Integer arithmetic wraps on overflow:

```wat
(module
  (func (param $x i32) (param $y i32) (result i32)
    (i32.add (local.get $x) (local.get $y)))
)
```

Also: `i32.sub`, `i32.mul`, `i32.div_s`, `i32.div_u`, `i32.rem_s`, `i32.rem_u` and `i64.*` variants.

Float arithmetic:

```wat
(module
  (func (param $x f64) (param $y f64) (result f64)
    (f64.mul (local.get $x) (local.get $y)))
)
```

Also: `f32.add`, `f32.sub`, `f32.mul`, `f32.div` and `f64.*` variants.

Bitwise and shifts (integers only):

```wat
(module
  (func (param $x i32) (param $y i32) (result i32)
    (i32.and (local.get $x) (local.get $y)))
)
```

Also: `i32.or`, `i32.xor`, `i32.shl`, `i32.shr_s`, `i32.shr_u`, `i32.rotl`, `i32.rotr`.

## Comparisons

Comparisons push `i32` booleans (0 = false, 1 = true).

```wat
(module
  (func (param $x i64) (param $y i64) (result i32)
    (i64.lt_s (local.get $x) (local.get $y)))
)
```

Also: `i64.eq`, `i64.ne`, `i64.le_s/u`, `i64.gt_s/u`, `i64.ge_s/u`.

Floats use IEEE 754 semantics (NaN-aware):

```wat
(module
  (func (param $x f32) (param $y f32) (result i32)
    (f32.eq (local.get $x) (local.get $y)))
)
```

Also: `f32.ne`, `f32.lt`, `f32.le`, `f32.gt`, `f32.ge` and `f64.*` variants.

## Conversions

```wat
(module
  (func (param $x i32) (param $y f32) (result f64)
    (f64.add
      (f64.convert_i32_s (local.get $x))
      (f64.promote_f32 (local.get $y))))
)
```

Other families: `extend`/`wrap` between integer widths, `trunc` from floats to ints (with `_s`/`_u` variants).

## Control flow

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

- `block` introduces a label you can `br` to (exits the block).
- `loop` treats `br` as continue (jumps to top).
- `br_if` pops a condition and branches if non-zero.

## Instruction Reference

- [i32](/instructions/i32), [i64](/instructions/i64), [f32](/instructions/f32), [f64](/instructions/f64) — all numeric operations
- [Local & Global](/instructions/local-global) — `local.get`, `local.set`, `local.tee`, `global.get`, `global.set`
- [Control Flow](/instructions/control) — `block`, `loop`, `if`, `br`, `br_if`, `call`, etc.
