---
title: GC i31
description: WasmGC i31 operations
---

## ref.i31

Create a 31-bit integer reference from an i32.

**Signature:** `(param i32) (result i31ref)`

**Example:**

```wat
(ref.i31 (i32.const 42))
```

---

## i31.get_s

Get the signed i32 value from an i31 reference.

**Signature:** `(param i31ref) (result i32)`

**Example:**

```wat
(i31.get_s (local.get $i31))
```

---

## i31.get_u

Get the unsigned i32 value from an i31 reference.

**Signature:** `(param i31ref) (result i32)`

**Example:**

```wat
(i31.get_u (local.get $i31))
```

---
