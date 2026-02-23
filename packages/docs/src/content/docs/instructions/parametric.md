---
title: Parametric
description: Stack manipulation instructions
sidebar:
  order: 7
---

## drop

Remove the top value from the stack.

**Example:**

```wat
# (module (func
(i32.const 42)
(drop)  ;; Remove 42 from stack
# ))
```

---

## select

Select one of two values based on a condition.

**Signature:** `(param T T i32) (result T)`

**Example:**

```wat
# (module (func (param $x i32)
# (drop
;; Returns first value if condition is non-zero, else second
(select
  (i32.const 10)
  (i32.const 20)
  (i32.const 1))  ;; Returns 10
# )
# (drop
;; Can specify type
(select (result i32)
  (i32.const 42)
  (i32.const 0)
  (i32.eqz (local.get $x)))
# )))
```

---
