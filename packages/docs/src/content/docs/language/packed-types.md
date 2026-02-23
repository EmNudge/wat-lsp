---
title: Packed Types
description: i8 and i16 storage types for struct and array fields (GC proposal).
sidebar:
  order: 5
---

The GC proposal introduces **packed storage types** `i8` and `i16`. These are _not_ value types — they can only appear in struct fields and array element definitions.

## Valid usage

Packed types are valid as storage types inside `field` and `array` definitions:

```wat
(module
  (type $pixel (struct
    (field $r i8)
    (field $g i8)
    (field $b i8)
    (field $a i8)))

  (type $buffer (array (mut i16)))
)
```

## Invalid usage

Using `i8` or `i16` as value types in parameters, results, locals, or globals is an error:

```wat-snippet
;; All of these are invalid:
(func (param i8))        ;; error: i8 is not a value type
(func (result i16))      ;; error: i16 is not a value type
(local $x i8)            ;; error: i8 is not a value type
(global $g i16 ...)      ;; error: i16 is not a value type
```

The LSP reports: _"i8 is a packed storage type and can only be used in struct/array field definitions"_.

## Accessing packed fields

Use `struct.get_s` / `struct.get_u` (signed/unsigned extension) to read packed fields, and `struct.set` to write them. Reading with plain `struct.get` on a packed field is invalid — you must specify the sign extension.
