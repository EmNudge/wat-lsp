---
title: Exceptions
description: Exception handling operations
sidebar:
  order: 17
---

## throw

Throw an exception with a tag.

**Signature:** `(param args...)`

**Example:**

```wat
# (module
# (tag $error_tag (param i32))
# (func
(throw $error_tag (i32.const 500))
# ))
```

---

## throw_ref

Throw an existing exception reference.

**Signature:** `(param exnref)`

**Example:**

```wat
# (module (func (param $exn exnref)
(throw_ref (local.get $exn))
# ))
```

---

## rethrow

Rethrow an exception from a catch block.

**Signature:** `(param i32)`

**Example:**

```wat-snippet
(rethrow $label)
```

---

## tag

Declare an exception tag with a type signature for exception handling.

**Example:**

```wat
# (module
(tag $error (param i32))
(tag $complex_error (param i32 i32 f64))
# )
```

---

## try_table

Define a block that catches exceptions using a jump table.

**Example:**

```wat
# (module
# (tag $tag (param i32))
# (func (result i32)
# (block $handler_label (result i32)
(try_table (catch $tag $handler_label)
  (throw $tag (i32.const 1))
)
# (unreachable))))
```

---

## catch

Catches exceptions with a specific tag in a `try_table` block. Branches to a label with the exception payload.

**Example:**

```wat-snippet
(block $handler (result i32)
  (try_table (catch $error_tag $handler)
    (call $may_throw)
    (i32.const 0)  ;; No error
  )
)
;; $handler receives the i32 payload from $error_tag

;; Multiple catch clauses
(try_table
  (catch $error1 $handle_error1)
  (catch $error2 $handle_error2)
  (catch_all $handle_any)
  (call $risky_operation))
```

---

## catch_all

Catches any exception in a `try_table` block, regardless of tag.

**Example:**

```wat
# (module
# (func $may_throw_anything)
# (func
(block $fallback
  (try_table (catch_all $fallback)
    (call $may_throw_anything)
  )
)
;; $fallback receives exnref
# ))
```

---

## catch_ref

Catches exceptions with a specific tag and provides the exception reference.

**Example:**

```wat
# (module
# (tag $error_tag (param i32))
# (func $may_throw)
# (func (result i32 exnref)
(block $handler (result i32 exnref)
  (try_table (catch_ref $error_tag $handler)
    (call $may_throw)
  )
  (unreachable)
)
# ))
```

---

## catch_all_ref

Catches any exception and provides the exception reference.

**Example:**

```wat
# (module
# (func $may_throw)
# (func (result exnref)
(block $handler (result exnref)
  (try_table (catch_all_ref $handler)
    (call $may_throw)
  )
  (unreachable)
)
# ))
```

---
