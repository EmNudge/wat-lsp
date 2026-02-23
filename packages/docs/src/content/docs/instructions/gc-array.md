---
title: GC Array
description: WasmGC array operations
sidebar:
  order: 14
---

## array.new

Create a new array on the heap.

**Signature:** `(param field_type i32) (result arrayref)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i32)))
# (func (result (ref $my_array))
(array.new $my_array (i32.const 0) (i32.const 10)) ;; Create size 10 array filled with 0
# ))
```

---

## array.new_default

Create a new array with default values.

**Signature:** `(param i32) (result arrayref)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i32)))
# (func (result (ref $my_array))
(array.new_default $my_array (i32.const 10))
# ))
```

---

## array.new_fixed

Create a new array from a fixed set of arguments.

**Signature:** `(param field_type...) (result arrayref)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i32)))
# (func (result (ref $my_array))
(array.new_fixed $my_array 3 (i32.const 1) (i32.const 2) (i32.const 3))
# ))
```

---

## array.new_data

Create a new array from a data segment.

**Signature:** `(param i32 i32) (result arrayref)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i8)))
# (memory 1)
# (data $data_index "0123456789")
# (func (result (ref $my_array))
(array.new_data $my_array $data_index (i32.const 0) (i32.const 10))
# ))
```

---

## array.new_elem

Create a new array from an element segment.

**Signature:** `(param i32 i32) (result arrayref)`

**Example:**

```wat
# (module
# (type $my_array (array (mut funcref)))
# (elem $elem_index funcref)
# (func (result (ref $my_array))
(array.new_elem $my_array $elem_index (i32.const 0) (i32.const 10))
# ))
```

---

## array.get

Get an element from an array.

**Signature:** `(param arrayref i32) (result field_type)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i32)))
# (func (param $arr (ref $my_array)) (result i32)
(array.get $my_array (local.get $arr) (i32.const 5))
# ))
```

---

## array.get_s

Get a signed element from an array.

**Signature:** `(param arrayref i32) (result field_type)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i8)))
# (func (param $arr (ref $my_array)) (result i32)
(array.get_s $my_array (local.get $arr) (i32.const 5))
# ))
```

---

## array.get_u

Get an unsigned element from an array.

**Signature:** `(param arrayref i32) (result field_type)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i8)))
# (func (param $arr (ref $my_array)) (result i32)
(array.get_u $my_array (local.get $arr) (i32.const 5))
# ))
```

---

## array.set

Set an element in an array.

**Signature:** `(param arrayref i32 field_type)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i32)))
# (func (param $arr (ref $my_array))
(array.set $my_array (local.get $arr) (i32.const 5) (i32.const 42))
# ))
```

---

## array.len

Get the length of an array.

**Signature:** `(param arrayref) (result i32)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i32)))
# (func (param $arr (ref $my_array)) (result i32)
(array.len (local.get $arr))
# ))
```

---

## array.fill

Fill a range of an array with a value.

**Signature:** `(param arrayref i32 field_type i32)`

**Example:**

```wat
# (module
# (type $my_array (array (mut i32)))
# (func (param $arr (ref $my_array))
(array.fill $my_array (local.get $arr) (i32.const 0) (i32.const 42) (i32.const 10))
# ))
```

---

## array.copy

Copy a range from one array to another.

**Signature:** `(param arrayref i32 arrayref i32 i32)`

**Example:**

```wat
# (module
# (type $dst_type (array (mut i32)))
# (type $src_type (array (mut i32)))
# (func (param $dst (ref $dst_type)) (param $src (ref $src_type))
(array.copy $dst_type $src_type (local.get $dst) (i32.const 0) (local.get $src) (i32.const 0) (i32.const 10))
# ))
```

---

## array.init_data

Initialize a portion of an array from a passive data segment. Takes the array, destination offset in the array, source offset in the data segment, and length.

**Signature:** `(param arrayref i32 i32 i32)`

**Example:**

```wat
# (module
# (type $byte_array (array (mut i8)))
# (memory 1)
# (data $data_segment "0123456789")
# (func (param $arr (ref $byte_array))
(array.init_data $byte_array $data_segment
  (local.get $arr)     ;; array reference
  (i32.const 0)        ;; destination offset in array
  (i32.const 0)        ;; source offset in data segment
  (i32.const 100))     ;; number of elements to copy
# ))
```

---

## array.init_elem

Initialize a portion of an array from a passive element segment. Takes the array, destination offset in the array, source offset in the element segment, and length.

**Signature:** `(param arrayref i32 i32 i32)`

**Example:**

```wat
# (module
# (type $funcref_array (array (mut funcref)))
# (elem $elem_index funcref)
# (func (param $arr (ref $funcref_array))
(array.init_elem $funcref_array $elem_index (local.get $arr) (i32.const 0) (i32.const 0) (i32.const 3))
# ))
```

---
