---
title: Table
description: Table operations
---

## table.get

Get element from table at index.

**Signature:** `(param i32) (result reftype)`

**Example:**

```wat
(table $funcs 10 funcref)
(table.get $funcs (i32.const 0))
```

---

## table.set

Set element in table at index.

**Signature:** `(param i32 reftype)`

**Example:**

```wat
(table $funcs 10 funcref)
(table.set $funcs
  (i32.const 0)
  (ref.func $my_function))
```

---

## table.size

Get current table size.

**Signature:** `(result i32)`

**Example:**

```wat
(table.size $funcs)
```

---

## table.grow

Grow table by delta, returns previous size or -1 on failure.

**Signature:** `(param reftype i32) (result i32)`

**Example:**

```wat
(table.grow $funcs
  (ref.null func)
  (i32.const 5))  ;; Grow by 5 elements
```

---

## table.fill

Fill a region of a table with a value.

**Signature:** `(param i32 reftype i32)`

**Example:**

```wat
(table $funcs 10 funcref)
;; Fill 5 slots starting at index 0 with null
(table.fill $funcs
  (i32.const 0)       ;; start index
  (ref.null func)     ;; value
  (i32.const 5))      ;; count
```

---

## table.copy

Copy elements from one table region to another.

**Signature:** `(param i32 i32 i32)`

**Example:**

```wat
(table $funcs 10 funcref)
;; Copy 3 elements from index 0 to index 5
(table.copy $funcs $funcs
  (i32.const 5)   ;; destination index
  (i32.const 0)   ;; source index
  (i32.const 3))  ;; count
```

---

## table.init

Initialize table region from a passive element segment.

**Signature:** `(param i32 i32 i32)`

**Example:**

```wat
(elem $my_elem func $f1 $f2 $f3)
;; Copy 3 elements from elem segment to table
(table.init $my_table $my_elem
  (i32.const 0)   ;; table destination
  (i32.const 0)   ;; elem segment offset
  (i32.const 3))  ;; count
```

---

## elem.drop

Drop a passive element segment, freeing its memory.

**Example:**

```wat
(elem $my_elem func $f1 $f2)
(elem.drop $my_elem)  ;; Element segment can no longer be used
```

---
