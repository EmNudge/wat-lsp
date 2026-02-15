---
title: try_table
description: Exception handling with try_table, catch clauses, and tags.
sidebar:
  order: 8
---

`try_table` creates a block that can catch exceptions thrown during its execution. Catch clauses specify which exception tags to handle and where to branch when caught.

```wat
(module
  (tag $e-i32 (param i32))

  (func $example (result i32)
    (block $handler (result i32)
      (try_table (result i32) (catch $e-i32 $handler)
        (throw $e-i32 (i32.const 42))
        (i32.const 0)
      )
    )
  )
)
```

## Catch clause variants

| Clause          | Syntax                    | Behavior                                                         |
| --------------- | ------------------------- | ---------------------------------------------------------------- |
| `catch`         | `(catch $tag $label)`     | Catches the tag, branches to label with the tag's payload values |
| `catch_ref`     | `(catch_ref $tag $label)` | Like `catch`, also pushes an `exnref`                            |
| `catch_all`     | `(catch_all $label)`      | Catches any exception, branches to label                         |
| `catch_all_ref` | `(catch_all_ref $label)`  | Like `catch_all`, also pushes an `exnref`                        |

## Tags

Tags declare exception types with their payload signatures:

```wat
(tag $my-error (param i32 i32))
```

When a `catch` clause matches, the tag's parameter types are delivered to the branch target label.

## Stack typing

- `try_table` blocks follow the same typing rules as `block` — they declare params and results.
- The LSP validates that the body leaves the correct result types on the stack.
- Catch clauses are declarative (label references) and don't contain code.

## Related instructions

- `throw $tag` — throws an exception with the given tag
- `throw_ref` — rethrows a caught exception reference
- `block` / `loop` / `if` — other structured control flow
