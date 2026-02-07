---
title: SIMD (v128)
description: 128-bit SIMD operations
---

## v128.const

Create a constant 128-bit vector value.

**Signature:** `(result v128)`

**Example:**

```wat
(v128.const i32x4 1 2 3 4)
(v128.const f32x4 1.0 2.0 3.0 4.0)
(v128.const i8x16 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15)
```

---

## v128.load

Load 128-bit vector from memory.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load (i32.const 0))
(v128.load offset=16 align=16 (i32.const 0))
```

---

## v128.store

Store 128-bit vector to memory.

**Signature:** `(param i32 v128)`

**Example:**

```wat
(v128.store (i32.const 0) (local.get $vec))
```

---

## v128.load8x8_s

Load 8 signed 8-bit integers and extend to 16-bit.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load8x8_s (i32.const 0))
```

---

## v128.load8x8_u

Load 8 unsigned 8-bit integers and extend to 16-bit.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load8x8_u (i32.const 0))
```

---

## v128.load16x4_s

Load 4 signed 16-bit integers and extend to 32-bit.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load16x4_s (i32.const 0))
```

---

## v128.load16x4_u

Load 4 unsigned 16-bit integers and extend to 32-bit.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load16x4_u (i32.const 0))
```

---

## v128.load32x2_s

Load 2 signed 32-bit integers and extend to 64-bit.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load32x2_s (i32.const 0))
```

---

## v128.load32x2_u

Load 2 unsigned 32-bit integers and extend to 64-bit.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load32x2_u (i32.const 0))
```

---

## v128.load8_splat

Load 8-bit value and replicate to all lanes.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load8_splat (i32.const 0))  ;; Loads byte and replicates 16 times
```

---

## v128.load16_splat

Load 16-bit value and replicate to all lanes.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load16_splat (i32.const 0))  ;; Loads i16 and replicates 8 times
```

---

## v128.load32_splat

Load 32-bit value and replicate to all lanes.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load32_splat (i32.const 0))  ;; Loads i32 and replicates 4 times
```

---

## v128.load64_splat

Load 64-bit value and replicate to all lanes.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load64_splat (i32.const 0))  ;; Loads i64 and replicates 2 times
```

---

## v128.load32_zero

Load 32-bit value into the lowest lane and zero all other lanes.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load32_zero (i32.const 0))  ;; Loads i32 into lane 0, zeros lanes 1-3
```

---

## v128.load64_zero

Load 64-bit value into the lowest lane and zero all other lanes.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(v128.load64_zero (i32.const 0))  ;; Loads i64 into lane 0, zeros lane 1
```

---

## v128.load8_lane

Load 8-bit value into a specific lane of an existing vector.

**Signature:** `(param i32 v128) (result v128)`

**Example:**

```wat
(v128.load8_lane 0 (i32.const 0) (local.get $vec))  ;; Load byte into lane 0
(v128.load8_lane offset=4 5 (i32.const 0) (local.get $vec))  ;; With offset, lane 5
```

---

## v128.load16_lane

Load 16-bit value into a specific lane of an existing vector.

**Signature:** `(param i32 v128) (result v128)`

**Example:**

```wat
(v128.load16_lane 0 (i32.const 0) (local.get $vec))  ;; Load i16 into lane 0
```

---

## v128.load32_lane

Load 32-bit value into a specific lane of an existing vector.

**Signature:** `(param i32 v128) (result v128)`

**Example:**

```wat
(v128.load32_lane 0 (i32.const 0) (local.get $vec))  ;; Load i32 into lane 0
```

---

## v128.load64_lane

Load 64-bit value into a specific lane of an existing vector.

**Signature:** `(param i32 v128) (result v128)`

**Example:**

```wat
(v128.load64_lane 0 (i32.const 0) (local.get $vec))  ;; Load i64 into lane 0
(v128.load64_lane offset=8 1 (i32.const 0) (local.get $vec))  ;; With offset, lane 1
```

---

## v128.store8_lane

Store 8-bit value from a specific lane to memory.

**Signature:** `(param i32 v128)`

**Example:**

```wat
(v128.store8_lane 0 (i32.const 0) (local.get $vec))  ;; Store lane 0 as byte
```

---

## v128.store16_lane

Store 16-bit value from a specific lane to memory.

**Signature:** `(param i32 v128)`

**Example:**

```wat
(v128.store16_lane 0 (i32.const 0) (local.get $vec))  ;; Store lane 0 as i16
```

---

## v128.store32_lane

Store 32-bit value from a specific lane to memory.

**Signature:** `(param i32 v128)`

**Example:**

```wat
(v128.store32_lane 0 (i32.const 0) (local.get $vec))  ;; Store lane 0 as i32
```

---

## v128.store64_lane

Store 64-bit value from a specific lane to memory.

**Signature:** `(param i32 v128)`

**Example:**

```wat
(v128.store64_lane 0 (i32.const 0) (local.get $vec))  ;; Store lane 0 as i64
(v128.store64_lane offset=8 1 (i32.const 0) (local.get $vec))  ;; With offset, lane 1
```

---

## v128.any_true

Check if any bit in the vector is non-zero.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
(v128.any_true (local.get $vec))  ;; Returns 1 if any bit is set
```

---

## v128.and

Compute bitwise AND of two v128 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(v128.and (local.get $a) (local.get $b))
```

---

## v128.or

Compute bitwise OR of two v128 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(v128.or (local.get $a) (local.get $b))
```

---

## v128.xor

Compute bitwise XOR of two v128 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(v128.xor (local.get $a) (local.get $b))
```

---

## v128.not

Compute bitwise NOT of a v128 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(v128.not (local.get $a))
```

---

## i8x16.add

Add two i8x16 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i8x16.add (local.get $a) (local.get $b))
```

---

## i16x8.add

Add two i16x8 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i16x8.add (local.get $a) (local.get $b))
```

---

## i32x4.add

Add two i32x4 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i32x4.add (local.get $a) (local.get $b))
```

---

## i64x2.add

Add two i64x2 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i64x2.add (local.get $a) (local.get $b))
```

---

## f32x4.add

Add two f32x4 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.add (local.get $a) (local.get $b))
```

---

## f32x4.mul

Multiply two f32x4 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.mul (local.get $a) (local.get $b))
```

---

## f32x4.sub

Subtract two f32x4 vectors lane-wise (first minus second).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.sub (local.get $a) (local.get $b))
```

---

## f32x4.div

Divide two f32x4 vectors lane-wise (first divided by second).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.div (local.get $a) (local.get $b))
```

---

## f32x4.sqrt

Compute square root of each lane in an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.sqrt (local.get $a))
```

---

## f32x4.neg

Negate each lane in an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.neg (local.get $a))
```

---

## f32x4.abs

Compute absolute value of each lane in an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.abs (local.get $a))
```

---

## f32x4.min

Compute lane-wise minimum of two f32x4 vectors. Returns NaN if either operand is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.min (local.get $a) (local.get $b))
```

---

## f32x4.max

Compute lane-wise maximum of two f32x4 vectors. Returns NaN if either operand is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.max (local.get $a) (local.get $b))
```

---

## f32x4.pmin

Pseudo-minimum: lane-wise `a < b ? a : b`. Unlike f32x4.min, returns second operand if first is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.pmin (local.get $a) (local.get $b))
```

---

## f32x4.pmax

Pseudo-maximum: lane-wise `a > b ? a : b`. Unlike f32x4.max, returns second operand if first is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.pmax (local.get $a) (local.get $b))
```

---

## f32x4.ceil

Round each lane to the nearest integer towards positive infinity.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.ceil (local.get $a))
```

---

## f32x4.floor

Round each lane to the nearest integer towards negative infinity.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.floor (local.get $a))
```

---

## f32x4.trunc

Round each lane to the nearest integer towards zero.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.trunc (local.get $a))
```

---

## f32x4.nearest

Round each lane to the nearest integer, with ties to even.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.nearest (local.get $a))
```

---

## f32x4.eq

Compare two f32x4 vectors for equality lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.eq (local.get $a) (local.get $b))
```

---

## f32x4.ne

Compare two f32x4 vectors for inequality lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.ne (local.get $a) (local.get $b))
```

---

## f32x4.lt

Compare two f32x4 vectors for less-than lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.lt (local.get $a) (local.get $b))
```

---

## f32x4.gt

Compare two f32x4 vectors for greater-than lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.gt (local.get $a) (local.get $b))
```

---

## f32x4.le

Compare two f32x4 vectors for less-than-or-equal lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.le (local.get $a) (local.get $b))
```

---

## f32x4.ge

Compare two f32x4 vectors for greater-than-or-equal lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f32x4.ge (local.get $a) (local.get $b))
```

---

## i32x4.splat

Create an i32x4 vector with all lanes set to the same i32 value.

**Signature:** `(param i32) (result v128)`

**Example:**

```wat
(i32x4.splat (i32.const 42))
```

---

## f32x4.splat

Create an f32x4 vector with all lanes set to the same f32 value.

**Signature:** `(param f32) (result v128)`

**Example:**

```wat
(f32x4.splat (f32.const 1.0))
```

---

## f64x2.add

Add two f64x2 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.add (local.get $a) (local.get $b))
```

---

## f64x2.sub

Subtract two f64x2 vectors lane-wise (first minus second).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.sub (local.get $a) (local.get $b))
```

---

## f64x2.mul

Multiply two f64x2 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.mul (local.get $a) (local.get $b))
```

---

## f64x2.div

Divide two f64x2 vectors lane-wise (first divided by second).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.div (local.get $a) (local.get $b))
```

---

## f64x2.sqrt

Compute square root of each lane in an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f64x2.sqrt (local.get $a))
```

---

## f64x2.neg

Negate each lane in an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f64x2.neg (local.get $a))
```

---

## f64x2.abs

Compute absolute value of each lane in an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f64x2.abs (local.get $a))
```

---

## f64x2.min

Compute lane-wise minimum of two f64x2 vectors. Returns NaN if either operand is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.min (local.get $a) (local.get $b))
```

---

## f64x2.max

Compute lane-wise maximum of two f64x2 vectors. Returns NaN if either operand is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.max (local.get $a) (local.get $b))
```

---

## f64x2.pmin

Pseudo-minimum: lane-wise `a < b ? a : b`. Unlike f64x2.min, returns second operand if first is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.pmin (local.get $a) (local.get $b))
```

---

## f64x2.pmax

Pseudo-maximum: lane-wise `a > b ? a : b`. Unlike f64x2.max, returns second operand if first is NaN.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.pmax (local.get $a) (local.get $b))
```

---

## f64x2.ceil

Round each lane to the nearest integer towards positive infinity.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f64x2.ceil (local.get $a))
```

---

## f64x2.floor

Round each lane to the nearest integer towards negative infinity.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f64x2.floor (local.get $a))
```

---

## f64x2.trunc

Round each lane to the nearest integer towards zero.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f64x2.trunc (local.get $a))
```

---

## f64x2.nearest

Round each lane to the nearest integer, with ties to even.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f64x2.nearest (local.get $a))
```

---

## f64x2.eq

Compare two f64x2 vectors for equality lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.eq (local.get $a) (local.get $b))
```

---

## f64x2.ne

Compare two f64x2 vectors for inequality lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.ne (local.get $a) (local.get $b))
```

---

## f64x2.lt

Compare two f64x2 vectors for less-than lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.lt (local.get $a) (local.get $b))
```

---

## f64x2.gt

Compare two f64x2 vectors for greater-than lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.gt (local.get $a) (local.get $b))
```

---

## f64x2.le

Compare two f64x2 vectors for less-than-or-equal lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.le (local.get $a) (local.get $b))
```

---

## f64x2.ge

Compare two f64x2 vectors for greater-than-or-equal lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(f64x2.ge (local.get $a) (local.get $b))
```

---

## f64x2.splat

Create an f64x2 vector with all lanes set to the same f64 value.

**Signature:** `(param f64) (result v128)`

**Example:**

```wat
(f64x2.splat (f64.const 1.0))
```

---

## i32x4.trunc_sat_f32x4_s

Convert an f32x4 vector to i32x4 with signed saturation. Values outside the signed i32 range are clamped.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(i32x4.trunc_sat_f32x4_s (local.get $floats))
```

---

## i32x4.trunc_sat_f32x4_u

Convert an f32x4 vector to i32x4 with unsigned saturation. Values outside the unsigned i32 range are clamped.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(i32x4.trunc_sat_f32x4_u (local.get $floats))
```

---

## f32x4.convert_i32x4_s

Convert an i32x4 vector to f32x4, interpreting each lane as a signed integer.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.convert_i32x4_s (local.get $ints))
```

---

## f32x4.convert_i32x4_u

Convert an i32x4 vector to f32x4, interpreting each lane as an unsigned integer.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(f32x4.convert_i32x4_u (local.get $ints))
```

---

## i32x4.eq

Compare two i32x4 vectors for equality lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i32x4.eq (local.get $a) (local.get $b))  ;; Returns -1 for equal lanes, 0 otherwise
```

---

## i32x4.gt_s

Signed greater-than comparison for i32x4 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i32x4.gt_s (local.get $a) (local.get $b))
```

---

## i32x4.gt_u

Unsigned greater-than comparison for i32x4 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i32x4.gt_u (local.get $a) (local.get $b))
```

---

## i32x4.le_s

Signed less-than-or-equal comparison for i32x4 vectors lane-wise.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i32x4.le_s (local.get $a) (local.get $b))
```

---

## i32x4.lt_s

Signed less-than comparison for i32x4 vectors lane-wise. Returns all 1s for true, all 0s for false in each lane.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
(i32x4.lt_s (local.get $a) (local.get $b))
```

---

## i32x4.shl

Shift each lane in an i32x4 vector left by a scalar amount. The shift count is taken modulo 32.

**Signature:** `(param v128 i32) (result v128)`

**Example:**

```wat
(i32x4.shl (local.get $vec) (i32.const 2))  ;; Shift all lanes left by 2
```

---

## i32x4.shr_s

Shift each lane in an i32x4 vector right by a scalar amount (signed/arithmetic). The shift count is taken modulo 32. Sign bit is preserved.

**Signature:** `(param v128 i32) (result v128)`

**Example:**

```wat
(i32x4.shr_s (local.get $vec) (i32.const 2))  ;; Arithmetic shift right by 2
```

---

## i8x16.all_true

Check if all lanes in an i8x16 vector are non-zero.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
(i8x16.all_true (local.get $vec))  ;; Returns 1 if all 16 lanes are non-zero
```

---

## i32x4.all_true

Check if all lanes in an i32x4 vector are non-zero.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
(i32x4.all_true (local.get $vec))  ;; Returns 1 if all 4 lanes are non-zero
```

---

## i32x4.abs

Absolute value of each lane in an i32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
(i32x4.abs (local.get $vec))
```

---

## i32x4.bitmask

Extract the high bit of each lane and combine into an i32 bitmask.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
;; Returns i32 where bit N is the high bit of lane N (0-3)
(i32x4.bitmask (local.get $vec))
```

---

## i8x16.extract_lane_s

Extract a signed 8-bit lane from an i8x16 vector and sign-extend to i32.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
;; Extract lane 5 (0-15) as signed i32
(i8x16.extract_lane_s 5 (local.get $vec))
```

---

## i8x16.extract_lane_u

Extract an unsigned 8-bit lane from an i8x16 vector and zero-extend to i32.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
;; Extract lane 5 (0-15) as unsigned i32
(i8x16.extract_lane_u 5 (local.get $vec))
```

---

## i16x8.extract_lane_s

Extract a signed 16-bit lane from an i16x8 vector and sign-extend to i32.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
;; Extract lane 3 (0-7) as signed i32
(i16x8.extract_lane_s 3 (local.get $vec))
```

---

## i16x8.extract_lane_u

Extract an unsigned 16-bit lane from an i16x8 vector and zero-extend to i32.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
;; Extract lane 3 (0-7) as unsigned i32
(i16x8.extract_lane_u 3 (local.get $vec))
```

---

## i32x4.extract_lane

Extract a 32-bit lane from an i32x4 vector.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat
;; Extract lane 2 (0-3)
(i32x4.extract_lane 2 (local.get $vec))
```

---

## i64x2.extract_lane

Extract a 64-bit lane from an i64x2 vector.

**Signature:** `(param v128) (result i64)`

**Example:**

```wat
;; Extract lane 1 (0-1)
(i64x2.extract_lane 1 (local.get $vec))
```

---

## f32x4.extract_lane

Extract a 32-bit float lane from an f32x4 vector.

**Signature:** `(param v128) (result f32)`

**Example:**

```wat
;; Extract lane 0 (0-3)
(f32x4.extract_lane 0 (local.get $vec))
```

---

## f64x2.extract_lane

Extract a 64-bit float lane from an f64x2 vector.

**Signature:** `(param v128) (result f64)`

**Example:**

```wat
;; Extract lane 0 (0-1)
(f64x2.extract_lane 0 (local.get $vec))
```

---

## i8x16.replace_lane

Replace an 8-bit lane in an i8x16 vector.

**Signature:** `(param v128 i32) (result v128)`

**Example:**

```wat
;; Replace lane 5 (0-15) with value 42
(i8x16.replace_lane 5 (local.get $vec) (i32.const 42))
```

---

## i16x8.replace_lane

Replace a 16-bit lane in an i16x8 vector.

**Signature:** `(param v128 i32) (result v128)`

**Example:**

```wat
;; Replace lane 3 (0-7) with value 1000
(i16x8.replace_lane 3 (local.get $vec) (i32.const 1000))
```

---

## i32x4.replace_lane

Replace a 32-bit lane in an i32x4 vector.

**Signature:** `(param v128 i32) (result v128)`

**Example:**

```wat
;; Replace lane 2 (0-3) with value
(i32x4.replace_lane 2 (local.get $vec) (i32.const 123456))
```

---

## i64x2.replace_lane

Replace a 64-bit lane in an i64x2 vector.

**Signature:** `(param v128 i64) (result v128)`

**Example:**

```wat
;; Replace lane 1 (0-1) with value
(i64x2.replace_lane 1 (local.get $vec) (i64.const 9876543210))
```

---

## f32x4.replace_lane

Replace a 32-bit float lane in an f32x4 vector.

**Signature:** `(param v128 f32) (result v128)`

**Example:**

```wat
;; Replace lane 0 (0-3) with value
(f32x4.replace_lane 0 (local.get $vec) (f32.const 3.14))
```

---

## f64x2.replace_lane

Replace a 64-bit float lane in an f64x2 vector.

**Signature:** `(param v128 f64) (result v128)`

**Example:**

```wat
;; Replace lane 0 (0-1) with value
(f64x2.replace_lane 0 (local.get $vec) (f64.const 2.71828))
```

---

## i8x16.shuffle

Shuffle bytes from two i8x16 vectors using 16 lane indices.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Shuffle lanes from two vectors
;; Indices 0-15 select from first vector, 16-31 from second
(i8x16.shuffle 0 1 2 3 16 17 18 19 4 5 6 7 20 21 22 23
  (local.get $a)
  (local.get $b))
```

---

## f32x4.relaxed_madd

Relaxed fused multiply-add: `a * b + c` for each f32 lane. The result may be computed with or without intermediate rounding, depending on the host platform. This allows efficient use of native FMA instructions where available.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Compute a * b + c with relaxed precision
(f32x4.relaxed_madd
  (local.get $a)   ;; multiplicand
  (local.get $b)   ;; multiplier
  (local.get $c))  ;; addend
```

---

## f32x4.relaxed_nmadd

Relaxed fused negative multiply-add: `-a * b + c` for each f32 lane. The result may be computed with or without intermediate rounding, depending on the host platform.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Compute -a * b + c with relaxed precision
(f32x4.relaxed_nmadd
  (local.get $a)   ;; multiplicand (negated)
  (local.get $b)   ;; multiplier
  (local.get $c))  ;; addend
```

---

## f64x2.relaxed_madd

Relaxed fused multiply-add: `a * b + c` for each f64 lane. The result may be computed with or without intermediate rounding, depending on the host platform. This allows efficient use of native FMA instructions where available.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Compute a * b + c with relaxed precision (64-bit floats)
(f64x2.relaxed_madd
  (local.get $a)   ;; multiplicand
  (local.get $b)   ;; multiplier
  (local.get $c))  ;; addend
```

---

## f64x2.relaxed_nmadd

Relaxed fused negative multiply-add: `-a * b + c` for each f64 lane. The result may be computed with or without intermediate rounding, depending on the host platform.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Compute -a * b + c with relaxed precision (64-bit floats)
(f64x2.relaxed_nmadd
  (local.get $a)   ;; multiplicand (negated)
  (local.get $b)   ;; multiplier
  (local.get $c))  ;; addend
```

---

## i8x16.relaxed_swizzle

Relaxed byte swizzle operation. Selects bytes from the first vector using indices from the second vector. Unlike i8x16.swizzle, the behavior for out-of-range indices (>= 16) is implementation-defined rather than returning 0.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Swizzle bytes with relaxed out-of-range behavior
(i8x16.relaxed_swizzle
  (local.get $data)     ;; source bytes
  (local.get $indices)) ;; lane indices (0-15 for defined behavior)
```

---

## i32x4.relaxed_trunc_f32x4_s

Relaxed truncation of f32x4 to signed i32x4. Unlike i32x4.trunc_sat_f32x4_s, the behavior for NaN and out-of-range values is implementation-defined, allowing more efficient code generation.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
;; Truncate f32 lanes to signed i32 with relaxed semantics
(i32x4.relaxed_trunc_f32x4_s (local.get $floats))
```

---

## i32x4.relaxed_trunc_f32x4_u

Relaxed truncation of f32x4 to unsigned i32x4. Unlike i32x4.trunc_sat_f32x4_u, the behavior for NaN and out-of-range values is implementation-defined, allowing more efficient code generation.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
;; Truncate f32 lanes to unsigned i32 with relaxed semantics
(i32x4.relaxed_trunc_f32x4_u (local.get $floats))
```

---

## i32x4.relaxed_trunc_f64x2_s_zero

Relaxed truncation of f64x2 to signed i32x4 with zero extension. Converts two f64 lanes to i32 and sets the upper two i32 lanes to zero. Behavior for NaN and out-of-range values is implementation-defined.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
;; Truncate two f64 lanes to signed i32, upper lanes zeroed
(i32x4.relaxed_trunc_f64x2_s_zero (local.get $doubles))
```

---

## i32x4.relaxed_trunc_f64x2_u_zero

Relaxed truncation of f64x2 to unsigned i32x4 with zero extension. Converts two f64 lanes to u32 and sets the upper two i32 lanes to zero. Behavior for NaN and out-of-range values is implementation-defined.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat
;; Truncate two f64 lanes to unsigned i32, upper lanes zeroed
(i32x4.relaxed_trunc_f64x2_u_zero (local.get $doubles))
```

---

## f32x4.relaxed_min

Relaxed lane-wise minimum of two f32x4 vectors. Unlike f32x4.min, the behavior for NaN inputs is implementation-defined, allowing use of efficient native min instructions.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Relaxed minimum with implementation-defined NaN handling
(f32x4.relaxed_min (local.get $a) (local.get $b))
```

---

## f32x4.relaxed_max

Relaxed lane-wise maximum of two f32x4 vectors. Unlike f32x4.max, the behavior for NaN inputs is implementation-defined, allowing use of efficient native max instructions.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Relaxed maximum with implementation-defined NaN handling
(f32x4.relaxed_max (local.get $a) (local.get $b))
```

---

## f64x2.relaxed_min

Relaxed lane-wise minimum of two f64x2 vectors. Unlike f64x2.min, the behavior for NaN inputs is implementation-defined, allowing use of efficient native min instructions.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Relaxed minimum with implementation-defined NaN handling (64-bit)
(f64x2.relaxed_min (local.get $a) (local.get $b))
```

---

## f64x2.relaxed_max

Relaxed lane-wise maximum of two f64x2 vectors. Unlike f64x2.max, the behavior for NaN inputs is implementation-defined, allowing use of efficient native max instructions.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Relaxed maximum with implementation-defined NaN handling (64-bit)
(f64x2.relaxed_max (local.get $a) (local.get $b))
```

---

## i8x16.relaxed_laneselect

Relaxed lane select for i8x16 vectors. Selects bytes from the first or second vector based on the mask. The exact selection semantics (whether it uses the high bit or all bits of each mask byte) is implementation-defined.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Select bytes based on mask with relaxed semantics
(i8x16.relaxed_laneselect
  (local.get $a)      ;; selected when mask bit is set
  (local.get $b)      ;; selected when mask bit is clear
  (local.get $mask))  ;; selection mask
```

---

## i16x8.relaxed_laneselect

Relaxed lane select for i16x8 vectors. Selects 16-bit lanes from the first or second vector based on the mask. The exact selection semantics is implementation-defined.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Select 16-bit lanes based on mask with relaxed semantics
(i16x8.relaxed_laneselect
  (local.get $a)      ;; selected when mask bit is set
  (local.get $b)      ;; selected when mask bit is clear
  (local.get $mask))  ;; selection mask
```

---

## i32x4.relaxed_laneselect

Relaxed lane select for i32x4 vectors. Selects 32-bit lanes from the first or second vector based on the mask. The exact selection semantics is implementation-defined.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Select 32-bit lanes based on mask with relaxed semantics
(i32x4.relaxed_laneselect
  (local.get $a)      ;; selected when mask bit is set
  (local.get $b)      ;; selected when mask bit is clear
  (local.get $mask))  ;; selection mask
```

---

## i64x2.relaxed_laneselect

Relaxed lane select for i64x2 vectors. Selects 64-bit lanes from the first or second vector based on the mask. The exact selection semantics is implementation-defined.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Select 64-bit lanes based on mask with relaxed semantics
(i64x2.relaxed_laneselect
  (local.get $a)      ;; selected when mask bit is set
  (local.get $b)      ;; selected when mask bit is clear
  (local.get $mask))  ;; selection mask
```

---

## i16x8.relaxed_q15mulr_s

Relaxed Q15 rounding multiply returning high half for signed i16x8 lanes. Computes `(a * b + 0x4000) >> 15` with implementation-defined overflow behavior. Useful for fixed-point DSP operations.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Q15 fixed-point multiply with relaxed overflow
(i16x8.relaxed_q15mulr_s (local.get $a) (local.get $b))
```

---

## i16x8.relaxed_dot_i8x16_i7x16_s

Relaxed dot product of i8x16 and i7x16 (7-bit unsigned) vectors, producing i16x8 results. Multiplies pairs of 8-bit values and sums adjacent products. The behavior when the second operand has the high bit set is implementation-defined.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat
;; Dot product: sum of pairwise products of 8-bit lanes
(i16x8.relaxed_dot_i8x16_i7x16_s
  (local.get $a)   ;; signed 8-bit values
  (local.get $b))  ;; 7-bit unsigned values (high bit behavior undefined)
```

---

## i32x4.relaxed_dot_i8x16_i7x16_add_s

Relaxed dot product of i8x16 and i7x16 vectors with i32x4 accumulation. Multiplies pairs of 8-bit values, sums groups of four products, and adds to the accumulator. The behavior when the second operand has the high bit set is implementation-defined.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat
;; Dot product with accumulation: useful for neural network inference
(i32x4.relaxed_dot_i8x16_i7x16_add_s
  (local.get $a)     ;; signed 8-bit values
  (local.get $b)     ;; 7-bit unsigned values
  (local.get $acc))  ;; i32x4 accumulator
```

---

## i8x16.sub

Lane-wise subtraction of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.sub (local.get $a) (local.get $b))
```

---

## i8x16.neg

Lane-wise negation of an i8x16 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.neg (local.get $a))
```

---

## i8x16.min_s

Lane-wise signed minimum of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.min_s (local.get $a) (local.get $b))
```

---

## i8x16.min_u

Lane-wise unsigned minimum of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.min_u (local.get $a) (local.get $b))
```

---

## i8x16.max_s

Lane-wise signed maximum of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.max_s (local.get $a) (local.get $b))
```

---

## i8x16.max_u

Lane-wise unsigned maximum of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.max_u (local.get $a) (local.get $b))
```

---

## i8x16.avgr_u

Lane-wise unsigned rounding average of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.avgr_u (local.get $a) (local.get $b))
```

---

## i8x16.abs

Lane-wise absolute value of an i8x16 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.abs (local.get $a))
```

---

## i8x16.popcnt

Lane-wise population count (number of set bits) of an i8x16 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.popcnt (local.get $a))
```

---

## i8x16.all_true

Returns 1 if all lanes are non-zero, 0 otherwise.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i8x16.all_true (local.get $a))
```

---

## i8x16.bitmask

Extract the high bit of each lane as an i32 bitmask.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i8x16.bitmask (local.get $a))
```

---

## i8x16.narrow_i16x8_s

Narrow two i16x8 vectors to i8x16 with signed saturation.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.narrow_i16x8_s (local.get $a) (local.get $b))
```

---

## i8x16.narrow_i16x8_u

Narrow two i16x8 vectors to i8x16 with unsigned saturation.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.narrow_i16x8_u (local.get $a) (local.get $b))
```

---

## i8x16.add_sat_s

Lane-wise saturating signed addition of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.add_sat_s (local.get $a) (local.get $b))
```

---

## i8x16.add_sat_u

Lane-wise saturating unsigned addition of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.add_sat_u (local.get $a) (local.get $b))
```

---

## i8x16.sub_sat_s

Lane-wise saturating signed subtraction of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.sub_sat_s (local.get $a) (local.get $b))
```

---

## i8x16.sub_sat_u

Lane-wise saturating unsigned subtraction of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.sub_sat_u (local.get $a) (local.get $b))
```

---

## i16x8.sub

Lane-wise subtraction of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.sub (local.get $a) (local.get $b))
```

---

## i16x8.neg

Lane-wise negation of an i16x8 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.neg (local.get $a))
```

---

## i16x8.mul

Lane-wise multiplication of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.mul (local.get $a) (local.get $b))
```

---

## i16x8.min_s

Lane-wise signed minimum of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.min_s (local.get $a) (local.get $b))
```

---

## i16x8.min_u

Lane-wise unsigned minimum of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.min_u (local.get $a) (local.get $b))
```

---

## i16x8.max_s

Lane-wise signed maximum of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.max_s (local.get $a) (local.get $b))
```

---

## i16x8.max_u

Lane-wise unsigned maximum of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.max_u (local.get $a) (local.get $b))
```

---

## i16x8.avgr_u

Lane-wise unsigned rounding average of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.avgr_u (local.get $a) (local.get $b))
```

---

## i16x8.abs

Lane-wise absolute value of an i16x8 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.abs (local.get $a))
```

---

## i16x8.all_true

Returns 1 if all lanes are non-zero, 0 otherwise.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i16x8.all_true (local.get $a))
```

---

## i16x8.bitmask

Extract the high bit of each lane as an i32 bitmask.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i16x8.bitmask (local.get $a))
```

---

## i16x8.narrow_i32x4_s

Narrow two i32x4 vectors to i16x8 with signed saturation.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.narrow_i32x4_s (local.get $a) (local.get $b))
```

---

## i16x8.narrow_i32x4_u

Narrow two i32x4 vectors to i16x8 with unsigned saturation.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.narrow_i32x4_u (local.get $a) (local.get $b))
```

---

## i16x8.add_sat_s

Lane-wise saturating signed addition of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.add_sat_s (local.get $a) (local.get $b))
```

---

## i16x8.add_sat_u

Lane-wise saturating unsigned addition of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.add_sat_u (local.get $a) (local.get $b))
```

---

## i16x8.sub_sat_s

Lane-wise saturating signed subtraction of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.sub_sat_s (local.get $a) (local.get $b))
```

---

## i16x8.sub_sat_u

Lane-wise saturating unsigned subtraction of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.sub_sat_u (local.get $a) (local.get $b))
```

---

## i16x8.extend_low_i8x16_s

Widen the low 8 lanes of an i8x16 vector to i16x8 with sign extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extend_low_i8x16_s (local.get $a))
```

---

## i16x8.extend_low_i8x16_u

Widen the low 8 lanes of an i8x16 vector to i16x8 with zero extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extend_low_i8x16_u (local.get $a))
```

---

## i16x8.extend_high_i8x16_s

Widen the high 8 lanes of an i8x16 vector to i16x8 with sign extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extend_high_i8x16_s (local.get $a))
```

---

## i16x8.extend_high_i8x16_u

Widen the high 8 lanes of an i8x16 vector to i16x8 with zero extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extend_high_i8x16_u (local.get $a))
```

---

## i16x8.extmul_low_i8x16_s

Extended multiply: multiply low 8 lanes of two i8x16 vectors and widen to i16x8 (signed).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extmul_low_i8x16_s (local.get $a) (local.get $b))
```

---

## i16x8.extmul_low_i8x16_u

Extended multiply: multiply low 8 lanes of two i8x16 vectors and widen to i16x8 (unsigned).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extmul_low_i8x16_u (local.get $a) (local.get $b))
```

---

## i16x8.extmul_high_i8x16_s

Extended multiply: multiply high 8 lanes of two i8x16 vectors and widen to i16x8 (signed).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extmul_high_i8x16_s (local.get $a) (local.get $b))
```

---

## i16x8.extmul_high_i8x16_u

Extended multiply: multiply high 8 lanes of two i8x16 vectors and widen to i16x8 (unsigned).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extmul_high_i8x16_u (local.get $a) (local.get $b))
```

---

## i16x8.extadd_pairwise_i8x16_s

Pairwise add adjacent i8x16 lanes and widen to i16x8 (signed).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extadd_pairwise_i8x16_s (local.get $a))
```

---

## i16x8.extadd_pairwise_i8x16_u

Pairwise add adjacent i8x16 lanes and widen to i16x8 (unsigned).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.extadd_pairwise_i8x16_u (local.get $a))
```

---

## i16x8.q15mulr_sat_s

Q15 fixed-point saturating rounding multiply of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.q15mulr_sat_s (local.get $a) (local.get $b))
```

---

## i32x4.sub

Lane-wise subtraction of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.sub (local.get $a) (local.get $b))
```

---

## i32x4.neg

Lane-wise negation of an i32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.neg (local.get $a))
```

---

## i32x4.mul

Lane-wise multiplication of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.mul (local.get $a) (local.get $b))
```

---

## i32x4.min_s

Lane-wise signed minimum of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.min_s (local.get $a) (local.get $b))
```

---

## i32x4.min_u

Lane-wise unsigned minimum of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.min_u (local.get $a) (local.get $b))
```

---

## i32x4.max_s

Lane-wise signed maximum of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.max_s (local.get $a) (local.get $b))
```

---

## i32x4.max_u

Lane-wise unsigned maximum of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.max_u (local.get $a) (local.get $b))
```

---

## i32x4.abs

Lane-wise absolute value of an i32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.abs (local.get $a))
```

---

## i32x4.all_true

Returns 1 if all lanes are non-zero, 0 otherwise.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i32x4.all_true (local.get $a))
```

---

## i32x4.bitmask

Extract the high bit of each lane as an i32 bitmask.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i32x4.bitmask (local.get $a))
```

---

## i32x4.extend_low_i16x8_s

Widen the low 4 lanes of an i16x8 vector to i32x4 with sign extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extend_low_i16x8_s (local.get $a))
```

---

## i32x4.extend_low_i16x8_u

Widen the low 4 lanes of an i16x8 vector to i32x4 with zero extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extend_low_i16x8_u (local.get $a))
```

---

## i32x4.extend_high_i16x8_s

Widen the high 4 lanes of an i16x8 vector to i32x4 with sign extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extend_high_i16x8_s (local.get $a))
```

---

## i32x4.extend_high_i16x8_u

Widen the high 4 lanes of an i16x8 vector to i32x4 with zero extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extend_high_i16x8_u (local.get $a))
```

---

## i32x4.extmul_low_i16x8_s

Extended multiply: multiply low 4 lanes of two i16x8 vectors and widen to i32x4 (signed).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extmul_low_i16x8_s (local.get $a) (local.get $b))
```

---

## i32x4.extmul_low_i16x8_u

Extended multiply: multiply low 4 lanes of two i16x8 vectors and widen to i32x4 (unsigned).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extmul_low_i16x8_u (local.get $a) (local.get $b))
```

---

## i32x4.extmul_high_i16x8_s

Extended multiply: multiply high 4 lanes of two i16x8 vectors and widen to i32x4 (signed).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extmul_high_i16x8_s (local.get $a) (local.get $b))
```

---

## i32x4.extmul_high_i16x8_u

Extended multiply: multiply high 4 lanes of two i16x8 vectors and widen to i32x4 (unsigned).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extmul_high_i16x8_u (local.get $a) (local.get $b))
```

---

## i32x4.extadd_pairwise_i16x8_s

Pairwise add adjacent i16x8 lanes and widen to i32x4 (signed).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extadd_pairwise_i16x8_s (local.get $a))
```

---

## i32x4.extadd_pairwise_i16x8_u

Pairwise add adjacent i16x8 lanes and widen to i32x4 (unsigned).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.extadd_pairwise_i16x8_u (local.get $a))
```

---

## i32x4.dot_i16x8_s

Dot product: multiply pairs of i16x8 lanes and sum adjacent products to i32x4.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.dot_i16x8_s (local.get $a) (local.get $b))
```

---

## i32x4.trunc_sat_f32x4_s

Convert f32x4 to i32x4 with signed saturating truncation.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.trunc_sat_f32x4_s (local.get $a))
```

---

## i32x4.trunc_sat_f32x4_u

Convert f32x4 to i32x4 with unsigned saturating truncation.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.trunc_sat_f32x4_u (local.get $a))
```

---

## i32x4.trunc_sat_f64x2_s_zero

Convert f64x2 to i32x4 with signed saturating truncation, zero-extending the upper lanes.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.trunc_sat_f64x2_s_zero (local.get $a))
```

---

## i32x4.trunc_sat_f64x2_u_zero

Convert f64x2 to i32x4 with unsigned saturating truncation, zero-extending the upper lanes.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.trunc_sat_f64x2_u_zero (local.get $a))
```

---

## i64x2.sub

Lane-wise subtraction of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.sub (local.get $a) (local.get $b))
```

---

## i64x2.neg

Lane-wise negation of an i64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.neg (local.get $a))
```

---

## i64x2.mul

Lane-wise multiplication of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.mul (local.get $a) (local.get $b))
```

---

## i64x2.abs

Lane-wise absolute value of an i64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.abs (local.get $a))
```

---

## i64x2.all_true

Returns 1 if all lanes are non-zero, 0 otherwise.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i64x2.all_true (local.get $a))
```

---

## i64x2.bitmask

Extract the high bit of each lane as an i32 bitmask.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(i64x2.bitmask (local.get $a))
```

---

## i64x2.extend_low_i32x4_s

Widen the low 2 lanes of an i32x4 vector to i64x2 with sign extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extend_low_i32x4_s (local.get $a))
```

---

## i64x2.extend_low_i32x4_u

Widen the low 2 lanes of an i32x4 vector to i64x2 with zero extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extend_low_i32x4_u (local.get $a))
```

---

## i64x2.extend_high_i32x4_s

Widen the high 2 lanes of an i32x4 vector to i64x2 with sign extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extend_high_i32x4_s (local.get $a))
```

---

## i64x2.extend_high_i32x4_u

Widen the high 2 lanes of an i32x4 vector to i64x2 with zero extension.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extend_high_i32x4_u (local.get $a))
```

---

## i64x2.extmul_low_i32x4_s

Extended multiply: multiply low 2 lanes of two i32x4 vectors and widen to i64x2 (signed).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extmul_low_i32x4_s (local.get $a) (local.get $b))
```

---

## i64x2.extmul_low_i32x4_u

Extended multiply: multiply low 2 lanes of two i32x4 vectors and widen to i64x2 (unsigned).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extmul_low_i32x4_u (local.get $a) (local.get $b))
```

---

## i64x2.extmul_high_i32x4_s

Extended multiply: multiply high 2 lanes of two i32x4 vectors and widen to i64x2 (signed).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extmul_high_i32x4_s (local.get $a) (local.get $b))
```

---

## i64x2.extmul_high_i32x4_u

Extended multiply: multiply high 2 lanes of two i32x4 vectors and widen to i64x2 (unsigned).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.extmul_high_i32x4_u (local.get $a) (local.get $b))
```

---

## f32x4.sub

Lane-wise subtraction of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.sub (local.get $a) (local.get $b))
```

---

## f32x4.mul

Lane-wise multiplication of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.mul (local.get $a) (local.get $b))
```

---

## f32x4.div

Lane-wise division of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.div (local.get $a) (local.get $b))
```

---

## f32x4.neg

Lane-wise negation of an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.neg (local.get $a))
```

---

## f32x4.abs

Lane-wise absolute value of an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.abs (local.get $a))
```

---

## f32x4.sqrt

Lane-wise square root of an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.sqrt (local.get $a))
```

---

## f32x4.min

Lane-wise minimum of two f32x4 vectors (IEEE 754 semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.min (local.get $a) (local.get $b))
```

---

## f32x4.max

Lane-wise maximum of two f32x4 vectors (IEEE 754 semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.max (local.get $a) (local.get $b))
```

---

## f32x4.pmin

Lane-wise pseudo-minimum of two f32x4 vectors (C-like semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.pmin (local.get $a) (local.get $b))
```

---

## f32x4.pmax

Lane-wise pseudo-maximum of two f32x4 vectors (C-like semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.pmax (local.get $a) (local.get $b))
```

---

## f32x4.ceil

Lane-wise ceiling (round toward positive infinity) of an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.ceil (local.get $a))
```

---

## f32x4.floor

Lane-wise floor (round toward negative infinity) of an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.floor (local.get $a))
```

---

## f32x4.trunc

Lane-wise truncation (round toward zero) of an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.trunc (local.get $a))
```

---

## f32x4.nearest

Lane-wise rounding to nearest integer (ties to even) of an f32x4 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.nearest (local.get $a))
```

---

## f32x4.convert_i32x4_s

Convert i32x4 to f32x4 (signed).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.convert_i32x4_s (local.get $a))
```

---

## f32x4.convert_i32x4_u

Convert i32x4 to f32x4 (unsigned).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.convert_i32x4_u (local.get $a))
```

---

## f32x4.demote_f64x2_zero

Demote f64x2 to f32x4, zero-extending the upper lanes.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.demote_f64x2_zero (local.get $a))
```

---

## f64x2.sub

Lane-wise subtraction of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.sub (local.get $a) (local.get $b))
```

---

## f64x2.mul

Lane-wise multiplication of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.mul (local.get $a) (local.get $b))
```

---

## f64x2.div

Lane-wise division of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.div (local.get $a) (local.get $b))
```

---

## f64x2.neg

Lane-wise negation of an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.neg (local.get $a))
```

---

## f64x2.abs

Lane-wise absolute value of an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.abs (local.get $a))
```

---

## f64x2.sqrt

Lane-wise square root of an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.sqrt (local.get $a))
```

---

## f64x2.min

Lane-wise minimum of two f64x2 vectors (IEEE 754 semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.min (local.get $a) (local.get $b))
```

---

## f64x2.max

Lane-wise maximum of two f64x2 vectors (IEEE 754 semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.max (local.get $a) (local.get $b))
```

---

## f64x2.pmin

Lane-wise pseudo-minimum of two f64x2 vectors (C-like semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.pmin (local.get $a) (local.get $b))
```

---

## f64x2.pmax

Lane-wise pseudo-maximum of two f64x2 vectors (C-like semantics).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.pmax (local.get $a) (local.get $b))
```

---

## f64x2.ceil

Lane-wise ceiling (round toward positive infinity) of an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.ceil (local.get $a))
```

---

## f64x2.floor

Lane-wise floor (round toward negative infinity) of an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.floor (local.get $a))
```

---

## f64x2.trunc

Lane-wise truncation (round toward zero) of an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.trunc (local.get $a))
```

---

## f64x2.nearest

Lane-wise rounding to nearest integer (ties to even) of an f64x2 vector.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.nearest (local.get $a))
```

---

## f64x2.convert_low_i32x4_s

Convert the low 2 lanes of i32x4 to f64x2 (signed).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.convert_low_i32x4_s (local.get $a))
```

---

## f64x2.convert_low_i32x4_u

Convert the low 2 lanes of i32x4 to f64x2 (unsigned).

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.convert_low_i32x4_u (local.get $a))
```

---

## f64x2.promote_low_f32x4

Promote the low 2 lanes of f32x4 to f64x2.

**Signature:** `(param v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.promote_low_f32x4 (local.get $a))
```

---

## v128.andnot

Bitwise AND NOT of two v128 vectors (a AND (NOT b)).

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(v128.andnot (local.get $a) (local.get $b))
```

---

## v128.bitselect

Bitwise select from two v128 vectors based on a mask.

**Signature:** `(param v128 v128 v128) (result v128)`

**Example:**

```wat-snippet
(v128.bitselect (local.get $a) (local.get $b) (local.get $mask))
```

---

## v128.any_true

Returns 1 if any bit in the v128 vector is set, 0 otherwise.

**Signature:** `(param v128) (result i32)`

**Example:**

```wat-snippet
(v128.any_true (local.get $a))
```

---

## i8x16.eq

Lane-wise equality comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.eq (local.get $a) (local.get $b))
```

---

## i8x16.ne

Lane-wise inequality comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.ne (local.get $a) (local.get $b))
```

---

## i8x16.lt_s

Lane-wise signed less-than comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.lt_s (local.get $a) (local.get $b))
```

---

## i8x16.lt_u

Lane-wise unsigned less-than comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.lt_u (local.get $a) (local.get $b))
```

---

## i8x16.gt_s

Lane-wise signed greater-than comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.gt_s (local.get $a) (local.get $b))
```

---

## i8x16.gt_u

Lane-wise unsigned greater-than comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.gt_u (local.get $a) (local.get $b))
```

---

## i8x16.le_s

Lane-wise signed less-than-or-equal comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.le_s (local.get $a) (local.get $b))
```

---

## i8x16.le_u

Lane-wise unsigned less-than-or-equal comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.le_u (local.get $a) (local.get $b))
```

---

## i8x16.ge_s

Lane-wise signed greater-than-or-equal comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.ge_s (local.get $a) (local.get $b))
```

---

## i8x16.ge_u

Lane-wise unsigned greater-than-or-equal comparison of two i8x16 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i8x16.ge_u (local.get $a) (local.get $b))
```

---

## i16x8.eq

Lane-wise equality comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.eq (local.get $a) (local.get $b))
```

---

## i16x8.ne

Lane-wise inequality comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.ne (local.get $a) (local.get $b))
```

---

## i16x8.lt_s

Lane-wise signed less-than comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.lt_s (local.get $a) (local.get $b))
```

---

## i16x8.lt_u

Lane-wise unsigned less-than comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.lt_u (local.get $a) (local.get $b))
```

---

## i16x8.gt_s

Lane-wise signed greater-than comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.gt_s (local.get $a) (local.get $b))
```

---

## i16x8.gt_u

Lane-wise unsigned greater-than comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.gt_u (local.get $a) (local.get $b))
```

---

## i16x8.le_s

Lane-wise signed less-than-or-equal comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.le_s (local.get $a) (local.get $b))
```

---

## i16x8.le_u

Lane-wise unsigned less-than-or-equal comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.le_u (local.get $a) (local.get $b))
```

---

## i16x8.ge_s

Lane-wise signed greater-than-or-equal comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.ge_s (local.get $a) (local.get $b))
```

---

## i16x8.ge_u

Lane-wise unsigned greater-than-or-equal comparison of two i16x8 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i16x8.ge_u (local.get $a) (local.get $b))
```

---

## i32x4.eq

Lane-wise equality comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.eq (local.get $a) (local.get $b))
```

---

## i32x4.ne

Lane-wise inequality comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.ne (local.get $a) (local.get $b))
```

---

## i32x4.lt_s

Lane-wise signed less-than comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.lt_s (local.get $a) (local.get $b))
```

---

## i32x4.lt_u

Lane-wise unsigned less-than comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.lt_u (local.get $a) (local.get $b))
```

---

## i32x4.gt_s

Lane-wise signed greater-than comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.gt_s (local.get $a) (local.get $b))
```

---

## i32x4.gt_u

Lane-wise unsigned greater-than comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.gt_u (local.get $a) (local.get $b))
```

---

## i32x4.le_s

Lane-wise signed less-than-or-equal comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.le_s (local.get $a) (local.get $b))
```

---

## i32x4.le_u

Lane-wise unsigned less-than-or-equal comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.le_u (local.get $a) (local.get $b))
```

---

## i32x4.ge_s

Lane-wise signed greater-than-or-equal comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.ge_s (local.get $a) (local.get $b))
```

---

## i32x4.ge_u

Lane-wise unsigned greater-than-or-equal comparison of two i32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i32x4.ge_u (local.get $a) (local.get $b))
```

---

## i64x2.eq

Lane-wise equality comparison of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.eq (local.get $a) (local.get $b))
```

---

## i64x2.ne

Lane-wise inequality comparison of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.ne (local.get $a) (local.get $b))
```

---

## i64x2.lt_s

Lane-wise signed less-than comparison of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.lt_s (local.get $a) (local.get $b))
```

---

## i64x2.gt_s

Lane-wise signed greater-than comparison of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.gt_s (local.get $a) (local.get $b))
```

---

## i64x2.le_s

Lane-wise signed less-than-or-equal comparison of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.le_s (local.get $a) (local.get $b))
```

---

## i64x2.ge_s

Lane-wise signed greater-than-or-equal comparison of two i64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(i64x2.ge_s (local.get $a) (local.get $b))
```

---

## f32x4.eq

Lane-wise equality comparison of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.eq (local.get $a) (local.get $b))
```

---

## f32x4.ne

Lane-wise inequality comparison of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.ne (local.get $a) (local.get $b))
```

---

## f32x4.lt

Lane-wise less-than comparison of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.lt (local.get $a) (local.get $b))
```

---

## f32x4.gt

Lane-wise greater-than comparison of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.gt (local.get $a) (local.get $b))
```

---

## f32x4.le

Lane-wise less-than-or-equal comparison of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.le (local.get $a) (local.get $b))
```

---

## f32x4.ge

Lane-wise greater-than-or-equal comparison of two f32x4 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f32x4.ge (local.get $a) (local.get $b))
```

---

## f64x2.eq

Lane-wise equality comparison of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.eq (local.get $a) (local.get $b))
```

---

## f64x2.ne

Lane-wise inequality comparison of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.ne (local.get $a) (local.get $b))
```

---

## f64x2.lt

Lane-wise less-than comparison of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.lt (local.get $a) (local.get $b))
```

---

## f64x2.gt

Lane-wise greater-than comparison of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.gt (local.get $a) (local.get $b))
```

---

## f64x2.le

Lane-wise less-than-or-equal comparison of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.le (local.get $a) (local.get $b))
```

---

## f64x2.ge

Lane-wise greater-than-or-equal comparison of two f64x2 vectors.

**Signature:** `(param v128 v128) (result v128)`

**Example:**

```wat-snippet
(f64x2.ge (local.get $a) (local.get $b))
```

---
