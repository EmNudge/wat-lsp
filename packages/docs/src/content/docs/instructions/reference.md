---
title: Reference
description: Reference type operations
---

## ref.null

Create a null reference.

**Signature:** `(result reftype)`

**Example:**

```wat
(ref.null func)
(ref.null extern)
```

---

## ref.func

Create a function reference.

**Signature:** `(result funcref)`

**Example:**

```wat
(func $my_func (result i32) (i32.const 42))
(ref.func $my_func)
```

---

## ref.is_null

Check if reference is null.

**Signature:** `(param reftype) (result i32)`

**Example:**

```wat
(ref.is_null (ref.null func))  ;; Returns 1
```

---

## ref.as_non_null

Assert that a reference is not null and convert it to a non-nullable type. Traps if the reference is null.

**Signature:** `(param (ref null ht)) (result (ref ht))`

**Example:**

```wat
(ref.as_non_null (local.get $nullable_ref))  ;; Traps if null, otherwise returns non-null ref
```

---

## any.convert_extern

Convert an externref to anyref. This allows external references to be used with GC reference operations.

**Signature:** `(param externref) (result anyref)`

**Example:**

```wat
(any.convert_extern (local.get $ext_ref))
```

---

## extern.convert_any

Convert an anyref to externref. This allows any GC reference to be passed out as an external reference.

**Signature:** `(param anyref) (result externref)`

**Example:**

```wat
(extern.convert_any (local.get $any_ref))
```

---
