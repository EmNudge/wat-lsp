---
title: GC Casts
description: WasmGC type cast operations
sidebar:
  order: 16
---

## ref.test

Test if a reference is null or of a specific type.

**Signature:** `(param ref) (result i32)`

**Example:**

```wat
# (module
# (type $type (struct (field i32)))
# (func (param $ref (ref null $type)) (result i32)
(ref.test (ref null $type) (local.get $ref))
# ))
```

---

## ref.cast

Cast a reference to a specific type (traps on failure).

**Signature:** `(param ref) (result ref)`

**Example:**

```wat
# (module
# (type $type (struct (field i32)))
# (func (param $ref (ref null $type)) (result (ref null $type))
(ref.cast (ref null $type) (local.get $ref))
# ))
```

---

## ref.cast_null

Cast a reference to a specific type (returns null on failure).

**Signature:** `(param ref) (result ref)`

**Example:**

```wat
# (module
# (type $type (struct (field i32)))
# (func (param $ref (ref null $type)) (result (ref null $type))
(ref.cast (ref null $type) (local.get $ref))
# ))
```

---

## br_on_cast

Branch if a reference can be cast to a type.

**Signature:** `(param ref) (result ref?)`

**Example:**

```wat
# (module
# (type $target_type (struct (field i32)))
# (func (param $ref (ref null $target_type)) (result (ref $target_type))
# (block $label (result (ref $target_type))
(br_on_cast $label (ref null $target_type) (ref $target_type) (local.get $ref))
# (unreachable))
# ))
```

---

## br_on_cast_fail

Branch if a reference cannot be cast to a type.

**Signature:** `(param ref) (result ref)`

**Example:**

```wat
# (module
# (type $target_type (struct (field i32)))
# (func (param $ref (ref null $target_type)) (result (ref null $target_type))
# (block $label (result (ref null $target_type))
(br_on_cast_fail $label (ref null $target_type) (ref $target_type) (local.get $ref))
# (unreachable))
# ))
```

---

## ref.eq

Compare two references for equality. Both references must be of type eqref or a subtype.

**Signature:** `(param eqref eqref) (result i32)`

**Example:**

```wat
# (module (func (param $ref1 eqref) (param $ref2 eqref) (result i32)
(ref.eq (local.get $ref1) (local.get $ref2))  ;; Returns 1 if equal, 0 otherwise
# ))
```

---
