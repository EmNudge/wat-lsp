---
title: GC Types
description: WasmGC type definitions
---

## sub

Declares a subtype that extends another type, enabling type hierarchies. The subtype must be compatible with its parent type.

**Example:**

```wat
;; Base type
(type $shape (struct
  (field $x f32)
  (field $y f32)))

;; Subtype extending $shape with additional fields
(type $rectangle (sub $shape (struct
  (field $x f32)
  (field $y f32)
  (field $width f32)
  (field $height f32))))

;; Sealed subtype (cannot be further extended)
(type $square (sub final $rectangle (struct
  (field $x f32)
  (field $y f32)
  (field $width f32)
  (field $height f32))))

;; Subtype without explicit parent (subtypes the top type)
(type $any_struct (sub (struct (field i32))))
```

---

## final

Modifier for subtypes that prevents further subtyping. A final type is sealed and cannot be extended.

**Example:**

```wat
;; This type cannot be subtyped
(type $sealed (sub final (struct
  (field $value i32))))

;; Final subtype of a parent
(type $leaf (sub final $node (struct
  (field $data i32))))

;; Error: cannot subtype a final type
;; (type $invalid (sub $sealed (struct ...)))  ;; not allowed
```

---

## rec

Declares a recursive type group, allowing mutually recursive type definitions.

**Example:**

```wat
;; Mutually recursive types
(rec
  (type $tree (struct
    (field $value i32)
    (field $left (ref null $tree))
    (field $right (ref null $tree))))

  (type $forest (struct
    (field $trees (ref null $tree_list))))

  (type $tree_list (struct
    (field $head (ref $tree))
    (field $tail (ref null $tree_list)))))

;; Single recursive type
(rec
  (type $node (struct
    (field $next (ref null $node)))))
```

---

## mut

Declares a mutable global variable or struct/array field. Without `mut`, the value is immutable.

**Example:**

```wat
;; Mutable global
(global $counter (mut i32) (i32.const 0))

;; Mutable field in struct
(type $cell (struct (field $value (mut i32))))

;; Mutable array elements
(type $buffer (array (mut i32)))
```

---

## shared

Declares shared memory that can be accessed by multiple threads (threads proposal). Required for atomic operations.

**Example:**

```wat
;; Shared memory with initial 1 page, max 4 pages
(memory $mem 1 4 shared)

;; Used with atomic operations
(i32.atomic.load (i32.const 0))
```

---

## null

Heap type representing the null reference. Used in type annotations to indicate a nullable reference type, or as the heap type for `ref.null` instructions.

**Example:**

```wat
;; Nullable reference to a function type
(local $callback (ref null func))

;; Nullable reference to a struct type
(param $obj (ref null $my_struct))

;; Create a null reference
(ref.null func)
(ref.null extern)
(ref.null $my_type)
```

---

## ref

Declares a reference type, optionally nullable. Used in type annotations for parameters, locals, globals, and fields.

**Example:**

```wat
;; Non-nullable reference to a struct type
(param $p (ref $my_struct))

;; Nullable reference
(local $obj (ref null $my_struct))

;; Reference to a function type
(global $callback (ref null $callback_type) (ref.null $callback_type))

;; In a field declaration
(type $node (struct (field $next (ref null $node))))

;; In an array type declaration
(type $shape_array (array (mut (ref null $shape))))
```

---

## field

Declares a field within a struct type definition. Fields can be mutable or immutable and have optional names.

**Example:**

```wat
;; Struct with named fields
(type $point (struct
  (field $x f32)
  (field $y f32)))

;; Mutable field
(type $counter (struct
  (field $value (mut i32))))

;; Multiple fields with mixed mutability
(type $person (struct
  (field $id i32)                    ;; immutable
  (field $age (mut i32))             ;; mutable
  (field $name (ref $string))))      ;; reference type

;; Packed field types
(type $packed (struct
  (field i8)
  (field i16)))
```

---

## struct

Declares a struct type with zero or more fields. Structs are heap-allocated reference types used with the GC proposal.

**Example:**

```wat
;; Simple struct type
(type $point (struct
  (field $x f32)
  (field $y f32)))

;; Struct with mixed field types
(type $object (struct
  (field $id i32)
  (field $data (ref null $data_type))
  (field $flags (mut i32))))

;; Recursive struct (linked list node)
(type $node (struct
  (field $value i32)
  (field $next (ref null $node))))

;; Using the struct
(func $create_point (result (ref $point))
  (struct.new $point (f32.const 1.0) (f32.const 2.0)))
```

---

## array

Declares an array type with elements of a specified type. Arrays are heap-allocated reference types with a fixed length determined at creation time. Used with the GC proposal.

**Example:**

```wat
;; Simple array of i32
(type $int_array (array i32))

;; Mutable array of floats
(type $float_array (array (mut f32)))

;; Array of nullable references to a struct type
(type $shape_array (array (mut (ref null $shape))))

;; Immutable array of non-nullable references
(type $func_table (array (ref $callback)))

;; Using arrays
(func $create_int_array (result (ref $int_array))
  (array.new $int_array (i32.const 0) (i32.const 10)))  ;; 10 elements initialized to 0
```

---
