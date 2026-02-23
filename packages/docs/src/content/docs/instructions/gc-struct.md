---
title: GC Struct
description: WasmGC struct operations
sidebar:
  order: 13
---

## struct.new

Create a new structure on the heap.

**Signature:** `(param field_types...) (result structref)`

**Example:**

```wat
# (module
# (type $my_struct (struct (field $a i32) (field $b f32)))
# (func (result (ref $my_struct))
(struct.new $my_struct (i32.const 1) (f32.const 2.0))
# ))
```

---

## struct.new_default

Create a new structure with default values.

**Signature:** `(result structref)`

**Example:**

```wat
# (module
# (type $my_struct (struct (field $a i32) (field $b f32)))
# (func (result (ref $my_struct))
(struct.new_default $my_struct)
# ))
```

---

## struct.get

Get a field from a structure.

**Signature:** `(param structref) (result field_type)`

**Example:**

```wat
# (module
# (type $my_struct (struct (field $field_name i32)))
# (func (param $s (ref $my_struct)) (result i32)
(struct.get $my_struct $field_name (local.get $s))
# ))
```

---

## struct.get_s

Get a signed field from a structure (sign-extending).

**Signature:** `(param structref) (result field_type)`

**Example:**

```wat
# (module
# (type $my_struct (struct (field $field_index i8)))
# (func (param $s (ref $my_struct)) (result i32)
(struct.get_s $my_struct $field_index (local.get $s))
# ))
```

---

## struct.get_u

Get an unsigned field from a structure (zero-extending).

**Signature:** `(param structref) (result field_type)`

**Example:**

```wat
# (module
# (type $my_struct (struct (field $field_index i8)))
# (func (param $s (ref $my_struct)) (result i32)
(struct.get_u $my_struct $field_index (local.get $s))
# ))
```

---

## struct.set

Set a field in a structure.

**Signature:** `(param structref field_type)`

**Example:**

```wat
# (module
# (type $my_struct (struct (field $field_index (mut i32))))
# (func (param $s (ref $my_struct))
(struct.set $my_struct $field_index (local.get $s) (i32.const 42))
# ))
```

---
