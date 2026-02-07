---
title: 'Constant Expressions'
description: 'Instructions allowed in initializer contexts.'
---

Some positions in a WebAssembly module require values known at instantiation time. These are called constant expressions, and only a restricted set of instructions may appear in them.

## Where constant expressions are used

- **Global initializers** -- the initial value of a `global`.
- **Element segment offsets** -- the table index where an element segment is placed.
- **Data segment offsets** -- the memory address where a data segment is placed.

## Allowed instructions

Numeric and vector constants:

- `i32.const`, `i64.const`, `f32.const`, `f64.const`, `v128.const`

Reference constructors:

- `ref.null`, `ref.func`, `ref.i31`

Global reads (immutable imported or previously defined globals only):

- `global.get`

GC allocation:

- `struct.new`, `struct.new_default`, `array.new`, `array.new_default`, `array.new_fixed`

Reference conversions:

- `any.convert_extern`, `extern.convert_any`

Extended constants (extended-const proposal):

- `i32.add`, `i32.sub`, `i32.mul`
- `i64.add`, `i64.sub`, `i64.mul`

## Global initializers

The most common use. Each global's init expression can only reference imports or globals defined earlier in the module.

```wat
(module
  (global $base (import "env" "base") i32)

  ;; Simple constant
  (global $flags i32 (i32.const 0x0F))

  ;; Reference an imported global (must be immutable)
  (global $offset i32 (global.get $base))

  ;; Extended-const: compute from imported global
  (global $next i32 (i32.add (global.get $base) (i32.const 16)))

  ;; Null reference global
  (global $callback (ref null func) (ref.null func))
)
```

## Element segment offsets

Element segments place function references (or other reference values) into a table at a computed offset.

```wat
(module
  (global $table_base (import "env" "table_base") i32)
  (table 10 funcref)

  (func $a (result i32) (i32.const 1))
  (func $b (result i32) (i32.const 2))

  ;; Constant offset
  (elem (i32.const 0) func $a $b)

  ;; Offset from an imported global
  (elem (global.get $table_base) func $a $b)
)
```

## Data segment offsets

Data segments copy bytes into linear memory at an offset determined by a constant expression.

```wat
(module
  (global $mem_base (import "env" "mem_base") i32)
  (memory 1)

  ;; Constant offset
  (data (i32.const 0) "hello")

  ;; Computed offset using an imported global
  (data (global.get $mem_base) "\01\02\03\04")

  ;; Extended-const arithmetic
  (data (i32.add (global.get $mem_base) (i32.const 256)) "world")
)
```

## GC constant expressions

With the GC proposal, constant expressions can allocate structs and arrays at instantiation time.

```wat
(module
  (type $pair (struct (field $a i32) (field $b i32)))
  (type $vec (array i32))

  ;; Struct allocated at instantiation
  (global $origin (ref $pair)
    (struct.new $pair (i32.const 0) (i32.const 0)))

  ;; Fixed-length array
  (global $ids (ref $vec)
    (array.new_fixed $vec 3
      (i32.const 10) (i32.const 20) (i32.const 30)))

  ;; Default-initialized struct (all fields zeroed)
  (global $zeroed (ref $pair)
    (struct.new_default $pair))
)
```

## Instruction Reference

- [Module Structure](/instructions/module) -- `module`, `global`, `elem`, `data`, `memory`, `table`
- [Local & Global](/instructions/local-global) -- `global.get`, `global.set`
- [Reference](/instructions/reference) -- `ref.null`, `ref.func`, `any.convert_extern`, `extern.convert_any`
