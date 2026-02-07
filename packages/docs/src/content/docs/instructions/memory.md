---
title: Memory
description: Memory operations
---

## memory.size

Get current memory size in pages (1 page = 64KB).

**Signature:** `(result i32)`

**Example:**

```wat
(memory.size)  ;; Returns current number of pages
```

---

## memory.grow

Grow memory by delta pages. Returns previous size, or -1 on failure.

**Signature:** `(param i32) (result i32)`

**Example:**

```wat
;; Grow memory by 1 page
(memory.grow (i32.const 1))
;; Returns old size (e.g., 1) or -1 if failed
```

---

## memory.fill

Fill a region of memory with a byte value.

**Signature:** `(param i32 i32 i32)`

**Example:**

```wat
;; Fill 100 bytes starting at address 0 with value 0xFF
(memory.fill
  (i32.const 0)    ;; destination
  (i32.const 0xFF) ;; value
  (i32.const 100)) ;; size
```

---

## memory.copy

Copy a region of memory to another location.

**Signature:** `(param i32 i32 i32)`

**Example:**

```wat
;; Copy 50 bytes from address 100 to address 200
(memory.copy
  (i32.const 200)  ;; destination
  (i32.const 100)  ;; source
  (i32.const 50))  ;; size
```

---

## memory.init

Initialize memory region from a passive data segment.

**Signature:** `(param i32 i32 i32)`

**Example:**

```wat
(data $my_data "Hello")
;; Copy 5 bytes from data segment offset 0 to memory address 0
(memory.init $my_data
  (i32.const 0)   ;; memory destination
  (i32.const 0)   ;; data segment offset
  (i32.const 5))  ;; size
```

---

## data.drop

Drop a passive data segment, freeing its memory.

**Example:**

```wat
(data $my_data "Hello")
(data.drop $my_data)  ;; Data segment can no longer be used
```

---
