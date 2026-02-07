---
title: Type Names
description: Value type names used in signatures
---

## i32

32-bit integer type. Not inherently signed or unsigned - interpretation depends on the operation.

**Example:**

```wat
(local $x i32)
(global $counter (mut i32) (i32.const 0))
(param $value i32)
```

---

## i64

64-bit integer type. Not inherently signed or unsigned - interpretation depends on the operation.

**Example:**

```wat
(local $timestamp i64)
(global $big_number i64 (i64.const 9223372036854775807))
```

---

## f32

32-bit floating point type (IEEE 754-2019 single precision).

**Example:**

```wat
(local $pi f32)
(global $epsilon f32 (f32.const 1.1920929e-07))
```

---

## f64

64-bit floating point type (IEEE 754-2019 double precision).

**Example:**

```wat
(local $precise f64)
(global $e f64 (f64.const 2.718281828459045))
```

---

## v128

128-bit vector type for SIMD operations. Can hold 16 i8, 8 i16, 4 i32, 2 i64, 4 f32, or 2 f64 lanes.

**Example:**

```wat
(local $vec v128)
(global $zeros v128 (v128.const i32x4 0 0 0 0))
```

---

## i8x16

SIMD shape annotation for v128 interpreted as 16 lanes of 8-bit integers. Used with v128.const and SIMD operations.

**Example:**

```wat
(v128.const i8x16 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15)
```

---

## i16x8

SIMD shape annotation for v128 interpreted as 8 lanes of 16-bit integers. Used with v128.const and SIMD operations.

**Example:**

```wat
(v128.const i16x8 0 1 2 3 4 5 6 7)
```

---

## i32x4

SIMD shape annotation for v128 interpreted as 4 lanes of 32-bit integers. Used with v128.const and SIMD operations.

**Example:**

```wat
(v128.const i32x4 1 2 3 4)
```

---

## i64x2

SIMD shape annotation for v128 interpreted as 2 lanes of 64-bit integers. Used with v128.const and SIMD operations.

**Example:**

```wat
(v128.const i64x2 1 2)
```

---

## f32x4

SIMD shape annotation for v128 interpreted as 4 lanes of 32-bit floats. Used with v128.const and SIMD operations.

**Example:**

```wat
(v128.const f32x4 1.0 2.0 3.0 4.0)
```

---

## f64x2

SIMD shape annotation for v128 interpreted as 2 lanes of 64-bit floats. Used with v128.const and SIMD operations.

**Example:**

```wat
(v128.const f64x2 1.0 2.0)
```

---

## funcref

Reference type for functions. Can be null.

**Example:**

```wat
(table $callbacks 10 funcref)
(global $current_handler (mut funcref) (ref.null func))
```

---

## externref

Reference type for external (host) values. Can be null.

**Example:**

```wat
(table $objects 10 externref)
(func $process (param $obj externref) ...)
```

---

## anyref

Reference type that can hold any reference (GC proposal). Equivalent to `(ref null any)`.

**Example:**

```wat
(global $obj anyref (ref.null any))
(func $store (param $val anyref) ...)
```

---

## eqref

Reference type for values that support equality comparison (GC proposal). Equivalent to `(ref null eq)`. Subtypes include i31ref, structref, and arrayref.

**Example:**

```wat
(func $compare (param $a eqref) (param $b eqref) (result i32)
  (ref.eq (local.get $a) (local.get $b)))
```

---

## i31ref

Reference type for 31-bit integers packed into a reference (GC proposal). Equivalent to `(ref null i31)`. Used for efficient small integer representation without heap allocation.

**Example:**

```wat
(func $box (param $val i32) (result i31ref)
  (ref.i31 (local.get $val)))
(func $unbox (param $ref i31ref) (result i32)
  (i31.get_s (local.get $ref)))
```

---

## structref

Reference type that can hold any struct reference (GC proposal). Equivalent to `(ref null struct)`.

**Example:**

```wat
(func $get_struct (result structref) ...)
(func $process (param $s structref) ...)
```

---

## arrayref

Reference type that can hold any array reference (GC proposal). Equivalent to `(ref null array)`.

**Example:**

```wat
(func $get_array (result arrayref) ...)
(func $process (param $arr arrayref) ...)
```

---

## nullref

Bottom reference type that represents only the null reference (GC proposal). Equivalent to `(ref null none)`.

**Example:**

```wat
(global $empty nullref (ref.null none))
```

---

## nullfuncref

Null reference type for functions (GC proposal). Equivalent to `(ref null nofunc)`.

**Example:**

```wat
(global $no_func nullfuncref (ref.null nofunc))
```

---

## nullexternref

Null reference type for external values (GC proposal). Equivalent to `(ref null noextern)`.

**Example:**

```wat
(global $no_extern nullexternref (ref.null noextern))
```

---

## exnref

Reference type for exception objects (exception handling proposal). Equivalent to `(ref null exn)`.

**Example:**

```wat
(block $handler (result exnref)
  (try_table (catch_all_ref $handler)
    (call $may_throw)
    (unreachable)
  )
)
```

---

## nullexnref

Null reference type for exceptions (exception handling proposal). Equivalent to `(ref null noexn)`.

**Example:**

```wat
(global $no_exn nullexnref (ref.null noexn))
```

---

## i8

8-bit integer storage type for packed struct fields and arrays (GC proposal). Not a value type - values are widened to i32 when accessed.

**Example:**

```wat
;; Packed array of bytes
(type $bytes (array (mut i8)))

;; Packed struct field
(type $pixel (struct
  (field $r i8)
  (field $g i8)
  (field $b i8)))

;; Access widens to i32
(array.get_u $bytes (local.get $arr) (i32.const 0))  ;; returns i32
```

---

## i16

16-bit integer storage type for packed struct fields and arrays (GC proposal). Not a value type - values are widened to i32 when accessed.

**Example:**

```wat
;; Packed array of shorts
(type $shorts (array (mut i16)))

;; Packed struct field
(type $coord (struct
  (field $x i16)
  (field $y i16)))

;; Access widens to i32
(array.get_s $shorts (local.get $arr) (i32.const 0))  ;; returns i32 (sign-extended)
```

---
