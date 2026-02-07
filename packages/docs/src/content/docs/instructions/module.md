---
title: Module Structure
description: Module definition constructs
---

## module

Declares a WebAssembly module. Top-level container for all declarations.

**Example:**

```wat-snippet
(module $my_module
  ;; Module contents here
  (func ...)
  (memory ...)
  (export ...)
)
```

---

## func

Declares a function with optional name, parameters, results, and locals.

**Example:**

```wat
;; Named function with params and result
(func $add (param $a i32) (param $b i32) (result i32)
  (i32.add (local.get $a) (local.get $b)))

;; Exported function (inline export)
(func (export "main") (result i32)
  (i32.const 42))

;; Multiple params of same type
(func $multi (param i32 i32 i32) (result i32)
  (local.get 0))
```

---

## param

Declares a function parameter with optional name and type.

**Example:**

```wat
(func $example
  (param $x i32)
  (param $y i32)
  (param i64)  ;; Unnamed param
  ;; ...
)
```

---

## result

Declares function result type(s).

**Example:**

```wat
(func $single (result i32)
  (i32.const 42))

;; Multiple results (multi-value proposal)
(func $multi (result i32 i32)
  (i32.const 1)
  (i32.const 2))
```

---

## local

Declares a local variable with optional name and type.

**Example:**

```wat
(func $example
  (local $counter i32)
  (local $temp i64)
  (local i32 i32)  ;; Two unnamed locals
  ;; ...
)
```

---

## global

Declares a global variable with type, mutability, and initial value.

**Example:**

```wat
;; Immutable global
(global $pi f64 (f64.const 3.14159))

;; Mutable global
(global $counter (mut i32) (i32.const 0))

;; Import global
(import "env" "global_var" (global $imported i32))
```

---

## table

Declares a table for storing references.

**Example:**

```wat
;; Table with size 10
(table $funcs 10 funcref)

;; Table with min and max
(table $refs 1 100 externref)

;; Inline elem declaration
(table $inline funcref (elem $f1 $f2 $f3))
```

---

## memory

Declares linear memory for the module.

**Example:**

```wat
;; 1 page minimum
(memory $mem 1)

;; 1 page min, 10 pages max
(memory $limited 1 10)

;; Named export
(memory (export "memory") 1)
```

---

## import

Imports an external resource (function, global, table, or memory).

**Example:**

```wat
;; Import function
(import "env" "log" (func $log (param i32)))

;; Import global
(import "env" "offset" (global $offset i32))

;; Import memory
(import "js" "mem" (memory 1))

;; Import table
(import "env" "table" (table 10 funcref))
```

---

## export

Exports a resource for use by the host.

**Example:**

```wat
;; Export function
(export "add" (func $add))

;; Export memory
(export "memory" (memory $mem))

;; Export global
(export "counter" (global $counter))

;; Inline export
(func (export "main") (result i32)
  (i32.const 42))
```

---

## start

Declares a function to be called automatically when the module is instantiated.

**Example:**

```wat-snippet
(module
  (func $init
    ;; Initialization code here
    (call $setup_globals)
    (call $init_memory))

  ;; Set $init as the start function
  (start $init)
)
```

---

## type

Declares a function type that can be referenced elsewhere.

A **typeuse** is a reference to a function type that can appear in `func`, `call_indirect`, `return_call_indirect`, and block types. It can be written three ways:

- **Explicit type reference only** — params/results come from the referenced type
- **Inline params/results only** — a matching type is implicitly created or reused
- **Both** — the inline params/results must match the referenced type

Form 3 is useful when you need both a type index (for `call_indirect`) and named parameters.

**Example:**

```wat
;; Define a binary operation type
(type $binop (func (param i32 i32) (result i32)))

;; Use in function declaration
(func $add (type $binop)
  (i32.add (local.get 0) (local.get 1)))

;; Use in call_indirect
(call_indirect (type $binop)
  (i32.const 5)
  (i32.const 3)
  (local.get $index))
```

---

## elem

Declares elements for a table.

**Example:**

```wat
(table $funcs 10 funcref)

;; Passive element (for table.init)
(elem $passive func $f1 $f2 $f3)

;; Active element (auto-initialized)
(elem (table $funcs) (i32.const 0) func $f1 $f2)

;; Declarative (just for ref.func)
(elem declare func $helper)
```

---

## data

Declares data to be loaded into memory.

**Example:**

```wat
(memory 1)

;; Active data (auto-initialized)
(data (i32.const 0) "Hello, World!")

;; Passive data (for memory.init)
(data $message "Error message")

;; Multiple segments
(data (i32.const 100) "\00\01\02\03")
```

---
