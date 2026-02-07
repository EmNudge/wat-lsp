---
title: Reference Types
description: Using funcref and externref, with tables and ref.* instructions.
---

Reference types let Wasm refer to functions and host objects.

## Types

- `funcref` — reference to a function inside the module or table.
- `externref` — opaque reference to a host value (e.g. a JS object).

## ref.null, ref.func, ref.is_null

```wat
(module
  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (table 2 funcref)
  (elem (i32.const 0) $double)

  (func (result i32)
    (ref.is_null (ref.null funcref)))    ;; -> 1

  (func (result i32)
    (ref.is_null (ref.func $double)))    ;; -> 0
)
```

## Tables and call_indirect

```wat
(module
  (type $t0 (func (param i32) (result i32)))
  (func $double (type $t0) (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (table 1 funcref)
  (elem (i32.const 0) $double)

  (func (param $n i32) (result i32)
    (call_indirect (type $t0) (local.get $n) (i32.const 0)))
)
```

The last argument to `call_indirect` is the table element index. The preceding arguments are passed to the function.

## externref

From JS, you can pass host references as `externref` via imports or function parameters.

## Instruction Reference

- [Reference Instructions](/instructions/reference) — `ref.null`, `ref.func`, `ref.is_null`, `ref.as_non_null`
- [Table Instructions](/instructions/table) — `table.get`, `table.set`, `table.grow`, `table.fill`
- [Type Names](/instructions/types) — `funcref`, `externref`
