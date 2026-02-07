---
title: Control Flow
description: Control flow instructions
---

## block

Define a block with a label at the end. Branching to this label exits the block.

**Example:**

```wat
(block $exit (result i32)
  (i32.const 10)
  (br_if $exit (i32.const 1))  ;; Exit early
  (i32.const 20)  ;; This won't execute
)
;; Returns 10
```

---

## loop

Define a loop with a label at the start. Branching to this label restarts the loop.

**Example:**

```wat
(local $i i32)
(local.set $i (i32.const 0))
(loop $continue
  (local.set $i (i32.add (local.get $i) (i32.const 1)))
  (br_if $continue (i32.lt_s (local.get $i) (i32.const 10)))
)
;; $i is now 10
```

---

## if

Conditional execution based on stack value. Execute 'then' if non-zero, 'else' if zero.

**Example:**

```wat
(if (result i32) (i32.const 1)
  (then (i32.const 42))
  (else (i32.const 0))
)
;; Returns 42

;; Without else
(if (i32.eq (local.get $x) (i32.const 0))
  (then (call $handle_zero))
)
```

---

## then

Marks the branch of an `if` statement that executes when the condition is non-zero (true).

**Example:**

```wat
(if (i32.gt_s (local.get $x) (i32.const 0))
  (then
    (call $log (i32.const 1))
    (local.set $positive (i32.const 1))))

;; With result type
(if (result i32) (local.get $condition)
  (then (i32.const 100))
  (else (i32.const 0)))
```

---

## else

Marks the branch of an `if` statement that executes when the condition is zero (false).

**Example:**

```wat
(if (result i32) (local.get $flag)
  (then (i32.const 1))
  (else (i32.const 0)))

;; Multi-statement else
(if (i32.eqz (local.get $x))
  (then (call $handle_zero))
  (else
    (call $handle_nonzero)
    (local.set $processed (i32.const 1))))
```

---

## br

Unconditional branch to a label. Exits blocks/loops.

**Example:**

```wat
(block $outer
  (block $inner
    (br $outer)  ;; Jump to end of $outer
    (unreachable)  ;; Never executed
  )
)
```

---

## br_if

Conditional branch to a label if top stack value is non-zero.

**Signature:** `(param i32)`

**Example:**

```wat
(block $exit
  (br_if $exit (i32.eq (local.get $x) (i32.const 0)))
  ;; Code here runs if $x != 0
)
```

---

## br_table

Table-based branch. Jumps to label based on index.

**Signature:** `(param i32)`

**Example:**

```wat
(block $case0
  (block $case1
    (block $case2
      (block $default
        (br_table $case0 $case1 $case2 $default
          (local.get $selector))
      )
      ;; default case
      (return)
    )
    ;; case 2
    (return)
  )
  ;; case 1
  (return)
)
;; case 0
```

---

## call

Call a function by name or index.

**Example:**

```wat
(func $add (param i32 i32) (result i32)
  (i32.add (local.get 0) (local.get 1)))

(func $main
  (call $add (i32.const 5) (i32.const 3))  ;; Returns 8
  (call 0 (i32.const 1) (i32.const 2))     ;; Call by index
)
```

---

## call_indirect

Call a function from a table using a dynamic index.

**Example:**

```wat
(type $binop (func (param i32 i32) (result i32)))
(table 2 funcref)
(elem (i32.const 0) $add $mul)

(func $add (param i32 i32) (result i32)
  (i32.add (local.get 0) (local.get 1)))

(func $mul (param i32 i32) (result i32)
  (i32.mul (local.get 0) (local.get 1)))

(func $dispatch (param $fn_index i32) (result i32)
  (call_indirect (type $binop)
    (i32.const 5)
    (i32.const 3)
    (local.get $fn_index))
)
```

---

## return

Return from the current function immediately.

**Example:**

```wat
(func $early_return (param $x i32) (result i32)
  (if (i32.eqz (local.get $x))
    (then (return (i32.const 0))))
  ;; More code here
  (i32.const 1)
)
```

---

## return_call

Tail call: calls a function and returns its result directly. The current function's frame is replaced, avoiding stack growth for recursive calls.

**Example:**

```wat
(func $factorial_tail (param $n i32) (param $acc i32) (result i32)
  (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
    (then (local.get $acc))
    (else (return_call $factorial_tail
      (i32.sub (local.get $n) (i32.const 1))
      (i32.mul (local.get $n) (local.get $acc))))))
```

---

## unreachable

Trap unconditionally. Used for code that should never be reached.

**Example:**

```wat
(func $divide (param $x i32) (param $y i32) (result i32)
  (if (i32.eqz (local.get $y))
    (then (unreachable)))  ;; Trap on division by zero
  (i32.div_s (local.get $x) (local.get $y))
)
```

---

## nop

No operation. Does nothing.

**Example:**

```wat
(nop)  ;; Useful for debugging or as placeholder
```

---

## call_ref

Call a function through a typed function reference. The reference type determines the expected function signature.

**Signature:** `(param args... funcref) (result results...)`

**Example:**

```wat
(type $sig (func (param i32) (result i32)))
(call_ref $sig (i32.const 42) (local.get $func_ref))
```

---

## return_call_ref

Tail call a function through a typed function reference. Immediately returns the callee's result without growing the call stack.

**Signature:** `(param args... funcref) (result results...)`

**Example:**

```wat
(type $sig (func (param i32) (result i32)))
(return_call_ref $sig (i32.const 42) (local.get $func_ref))
```

---

## br_on_null

Branch to a label if the reference is null. If not null, the non-null reference remains on the stack.

**Signature:** `(param ref) (result ref?)`

**Example:**

```wat
(block $is_null
  (br_on_null $is_null (local.get $maybe_null_ref))
  ;; Reference is not null here, use it
  (call $use_ref)
)
;; Jumped here if ref was null
```

---

## br_on_non_null

Branch to a label if the reference is not null. The non-null reference is passed to the target block.

**Signature:** `(param ref) (result)`

**Example:**

```wat
(block $not_null (param (ref $type))
  (br_on_non_null $not_null (local.get $maybe_null_ref))
  ;; Reference was null, handle null case
  (return)
)
;; Target block receives non-null ref
```

---

## return_call_indirect

Tail-call version of `call_indirect`. Calls a function from a table by index and returns its result directly, reusing the current call frame. The type must match the target function. This enables constant-space mutual recursion through tables.

**Signature:** `(param args... i32) (result T)`

**Example:**

```wat-snippet
(module
  (type $sig (func (param i32) (result i32)))
  (table 1 funcref)
  (elem (i32.const 0) $ping)
  (func $ping (type $sig) (param $n i32) (result i32)
    (if (result i32) (i32.eqz (local.get $n))
      (then (i32.const 0))
      (else (return_call_indirect (type $sig)
        (i32.sub (local.get $n) (i32.const 1))
        (i32.const 0)))))
)
```

---
