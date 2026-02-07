---
title: Local & Global
description: Local and global variable operations
---

## local.get

Get the value of a local variable by name or index.

**Example:**

```wat
(func $example (param $x i32) (result i32)
  (local $temp i32)
  (local.get $x)      ;; Get parameter
  (local.get $temp)   ;; Get local
  (local.get 0)       ;; Get by index
)
```

---

## local.set

Set the value of a local variable by name or index.

**Example:**

```wat
(func $example (param $x i32)
  (local $result i32)
  (local.set $result (i32.const 42))
  (local.set 1 (i32.const 100))  ;; Set by index
)
```

---

## local.tee

Set the value of a local variable and return it (combination of set and get).

**Example:**

```wat
(func $example (result i32)
  (local $x i32)
  ;; Set $x to 42 and also return it
  (local.tee $x (i32.const 42))
)
```

---

## global.get

Get the value of a global variable by name or index.

**Example:**

```wat
(global $counter (mut i32) (i32.const 0))

(func $read_counter (result i32)
  (global.get $counter)
)
```

---

## global.set

Set the value of a global variable by name or index. Only works on mutable globals.

**Example:**

```wat
(global $counter (mut i32) (i32.const 0))

(func $increment
  (global.set $counter
    (i32.add (global.get $counter) (i32.const 1)))
)
```

---
