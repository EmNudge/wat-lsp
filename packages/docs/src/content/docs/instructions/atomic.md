---
title: Atomics
description: Atomic memory operations (threads)
---

## i32.atomic.load

Atomically load a 32-bit integer from memory. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32) (result i32)`

**Example:**

```wat
;; Atomically load i32 from address
(i32.atomic.load (i32.const 0))
```

---

## i64.atomic.load

Atomically load a 64-bit integer from memory. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32) (result i64)`

**Example:**

```wat
;; Atomically load i64 from address
(i64.atomic.load (i32.const 0))
```

---

## i32.atomic.load8_u

Atomically load an 8-bit value from memory and zero-extend to i32. Requires shared memory.

**Signature:** `(param i32) (result i32)`

**Example:**

```wat
;; Atomically load byte and zero-extend to i32
(i32.atomic.load8_u (i32.const 0))
```

---

## i32.atomic.load16_u

Atomically load a 16-bit value from memory and zero-extend to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32) (result i32)`

**Example:**

```wat
;; Atomically load 16-bit value and zero-extend to i32
(i32.atomic.load16_u (i32.const 0))
```

---

## i64.atomic.load8_u

Atomically load an 8-bit value from memory and zero-extend to i64. Requires shared memory.

**Signature:** `(param i32) (result i64)`

**Example:**

```wat
;; Atomically load byte and zero-extend to i64
(i64.atomic.load8_u (i32.const 0))
```

---

## i64.atomic.load16_u

Atomically load a 16-bit value from memory and zero-extend to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32) (result i64)`

**Example:**

```wat
;; Atomically load 16-bit value and zero-extend to i64
(i64.atomic.load16_u (i32.const 0))
```

---

## i64.atomic.load32_u

Atomically load a 32-bit value from memory and zero-extend to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32) (result i64)`

**Example:**

```wat
;; Atomically load 32-bit value and zero-extend to i64
(i64.atomic.load32_u (i32.const 0))
```

---

## i32.atomic.store

Atomically store a 32-bit integer to memory. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32)`

**Example:**

```wat
;; Atomically store i32 value at address
(i32.atomic.store (i32.const 0) (i32.const 42))
```

---

## i64.atomic.store

Atomically store a 64-bit integer to memory. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64)`

**Example:**

```wat
;; Atomically store i64 value at address
(i64.atomic.store (i32.const 0) (i64.const 42))
```

---

## i32.atomic.store8

Atomically store the low 8 bits of an i32 to memory. Requires shared memory.

**Signature:** `(param i32 i32)`

**Example:**

```wat
;; Atomically store low byte of i32 at address
(i32.atomic.store8 (i32.const 0) (i32.const 255))
```

---

## i32.atomic.store16

Atomically store the low 16 bits of an i32 to memory. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32)`

**Example:**

```wat
;; Atomically store low 16 bits of i32 at address
(i32.atomic.store16 (i32.const 0) (i32.const 1000))
```

---

## i64.atomic.store8

Atomically store the low 8 bits of an i64 to memory. Requires shared memory.

**Signature:** `(param i32 i64)`

**Example:**

```wat
;; Atomically store low byte of i64 at address
(i64.atomic.store8 (i32.const 0) (i64.const 255))
```

---

## i64.atomic.store16

Atomically store the low 16 bits of an i64 to memory. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64)`

**Example:**

```wat
;; Atomically store low 16 bits of i64 at address
(i64.atomic.store16 (i32.const 0) (i64.const 1000))
```

---

## i64.atomic.store32

Atomically store the low 32 bits of an i64 to memory. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64)`

**Example:**

```wat
;; Atomically store low 32 bits of i64 at address
(i64.atomic.store32 (i32.const 0) (i64.const 100000))
```

---

## i32.atomic.rmw.add

Atomically read a 32-bit value, add to it, and store the result. Returns the original value. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically add 10 to value at address, return old value
(i32.atomic.rmw.add (i32.const 0) (i32.const 10))
```

---

## i32.atomic.rmw.sub

Atomically read a 32-bit value, subtract from it, and store the result. Returns the original value. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically subtract 5 from value at address, return old value
(i32.atomic.rmw.sub (i32.const 0) (i32.const 5))
```

---

## i32.atomic.rmw.and

Atomically read a 32-bit value, perform bitwise AND, and store the result. Returns the original value. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically AND with mask, return old value
(i32.atomic.rmw.and (i32.const 0) (i32.const 0xFF))
```

---

## i32.atomic.rmw.or

Atomically read a 32-bit value, perform bitwise OR, and store the result. Returns the original value. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically OR with flags, return old value
(i32.atomic.rmw.or (i32.const 0) (i32.const 0x80))
```

---

## i32.atomic.rmw.xor

Atomically read a 32-bit value, perform bitwise XOR, and store the result. Returns the original value. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically XOR with value, return old value
(i32.atomic.rmw.xor (i32.const 0) (i32.const 0xFF))
```

---

## i32.atomic.rmw.xchg

Atomically exchange (swap) a 32-bit value in memory. Returns the original value. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically swap value at address, return old value
(i32.atomic.rmw.xchg (i32.const 0) (i32.const 100))
```

---

## i64.atomic.rmw.add

Atomically read a 64-bit value, add to it, and store the result. Returns the original value. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically add 10 to i64 value at address, return old value
(i64.atomic.rmw.add (i32.const 0) (i64.const 10))
```

---

## i64.atomic.rmw.sub

Atomically read a 64-bit value, subtract from it, and store the result. Returns the original value. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically subtract 5 from i64 value at address, return old value
(i64.atomic.rmw.sub (i32.const 0) (i64.const 5))
```

---

## i64.atomic.rmw.and

Atomically read a 64-bit value, perform bitwise AND, and store the result. Returns the original value. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically AND with mask, return old value
(i64.atomic.rmw.and (i32.const 0) (i64.const 0xFF))
```

---

## i64.atomic.rmw.or

Atomically read a 64-bit value, perform bitwise OR, and store the result. Returns the original value. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically OR with flags, return old value
(i64.atomic.rmw.or (i32.const 0) (i64.const 0x80))
```

---

## i64.atomic.rmw.xor

Atomically read a 64-bit value, perform bitwise XOR, and store the result. Returns the original value. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically XOR with value, return old value
(i64.atomic.rmw.xor (i32.const 0) (i64.const 0xFF))
```

---

## i64.atomic.rmw.xchg

Atomically exchange (swap) a 64-bit value in memory. Returns the original value. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically swap i64 value at address, return old value
(i64.atomic.rmw.xchg (i32.const 0) (i64.const 100))
```

---

## i32.atomic.rmw8.add_u

Atomically read an 8-bit value, add to it, and store the result. Returns the original value zero-extended to i32. Requires shared memory.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically add to byte at address, return old value
(i32.atomic.rmw8.add_u (i32.const 0) (i32.const 1))
```

---

## i32.atomic.rmw8.sub_u

Atomically read an 8-bit value, subtract from it, and store the result. Returns the original value zero-extended to i32. Requires shared memory.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically subtract from byte at address, return old value
(i32.atomic.rmw8.sub_u (i32.const 0) (i32.const 1))
```

---

## i32.atomic.rmw8.and_u

Atomically read an 8-bit value, perform bitwise AND, and store the result. Returns the original value zero-extended to i32. Requires shared memory.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically AND byte at address with mask, return old value
(i32.atomic.rmw8.and_u (i32.const 0) (i32.const 0x0F))
```

---

## i32.atomic.rmw8.or_u

Atomically read an 8-bit value, perform bitwise OR, and store the result. Returns the original value zero-extended to i32. Requires shared memory.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically OR byte at address with flags, return old value
(i32.atomic.rmw8.or_u (i32.const 0) (i32.const 0x80))
```

---

## i32.atomic.rmw8.xor_u

Atomically read an 8-bit value, perform bitwise XOR, and store the result. Returns the original value zero-extended to i32. Requires shared memory.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically XOR byte at address, return old value
(i32.atomic.rmw8.xor_u (i32.const 0) (i32.const 0xFF))
```

---

## i32.atomic.rmw8.xchg_u

Atomically exchange an 8-bit value in memory. Returns the original value zero-extended to i32. Requires shared memory.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically swap byte at address, return old value
(i32.atomic.rmw8.xchg_u (i32.const 0) (i32.const 42))
```

---

## i32.atomic.rmw16.add_u

Atomically read a 16-bit value, add to it, and store the result. Returns the original value zero-extended to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically add to 16-bit value at address, return old value
(i32.atomic.rmw16.add_u (i32.const 0) (i32.const 100))
```

---

## i32.atomic.rmw16.sub_u

Atomically read a 16-bit value, subtract from it, and store the result. Returns the original value zero-extended to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically subtract from 16-bit value at address, return old value
(i32.atomic.rmw16.sub_u (i32.const 0) (i32.const 100))
```

---

## i32.atomic.rmw16.and_u

Atomically read a 16-bit value, perform bitwise AND, and store the result. Returns the original value zero-extended to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically AND 16-bit value at address with mask, return old value
(i32.atomic.rmw16.and_u (i32.const 0) (i32.const 0x00FF))
```

---

## i32.atomic.rmw16.or_u

Atomically read a 16-bit value, perform bitwise OR, and store the result. Returns the original value zero-extended to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically OR 16-bit value at address with flags, return old value
(i32.atomic.rmw16.or_u (i32.const 0) (i32.const 0x8000))
```

---

## i32.atomic.rmw16.xor_u

Atomically read a 16-bit value, perform bitwise XOR, and store the result. Returns the original value zero-extended to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically XOR 16-bit value at address, return old value
(i32.atomic.rmw16.xor_u (i32.const 0) (i32.const 0xFFFF))
```

---

## i32.atomic.rmw16.xchg_u

Atomically exchange a 16-bit value in memory. Returns the original value zero-extended to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Atomically swap 16-bit value at address, return old value
(i32.atomic.rmw16.xchg_u (i32.const 0) (i32.const 1000))
```

---

## i64.atomic.rmw8.add_u

Atomically read an 8-bit value, add to it, and store the result. Returns the original value zero-extended to i64. Requires shared memory.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically add to byte at address, return old value as i64
(i64.atomic.rmw8.add_u (i32.const 0) (i64.const 1))
```

---

## i64.atomic.rmw8.sub_u

Atomically read an 8-bit value, subtract from it, and store the result. Returns the original value zero-extended to i64. Requires shared memory.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically subtract from byte at address, return old value as i64
(i64.atomic.rmw8.sub_u (i32.const 0) (i64.const 1))
```

---

## i64.atomic.rmw8.and_u

Atomically read an 8-bit value, perform bitwise AND, and store the result. Returns the original value zero-extended to i64. Requires shared memory.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically AND byte at address with mask, return old value as i64
(i64.atomic.rmw8.and_u (i32.const 0) (i64.const 0x0F))
```

---

## i64.atomic.rmw8.or_u

Atomically read an 8-bit value, perform bitwise OR, and store the result. Returns the original value zero-extended to i64. Requires shared memory.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically OR byte at address with flags, return old value as i64
(i64.atomic.rmw8.or_u (i32.const 0) (i64.const 0x80))
```

---

## i64.atomic.rmw8.xor_u

Atomically read an 8-bit value, perform bitwise XOR, and store the result. Returns the original value zero-extended to i64. Requires shared memory.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically XOR byte at address, return old value as i64
(i64.atomic.rmw8.xor_u (i32.const 0) (i64.const 0xFF))
```

---

## i64.atomic.rmw8.xchg_u

Atomically exchange an 8-bit value in memory. Returns the original value zero-extended to i64. Requires shared memory.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically swap byte at address, return old value as i64
(i64.atomic.rmw8.xchg_u (i32.const 0) (i64.const 42))
```

---

## i64.atomic.rmw16.add_u

Atomically read a 16-bit value, add to it, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically add to 16-bit value at address, return old value as i64
(i64.atomic.rmw16.add_u (i32.const 0) (i64.const 100))
```

---

## i64.atomic.rmw16.sub_u

Atomically read a 16-bit value, subtract from it, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically subtract from 16-bit value at address, return old value as i64
(i64.atomic.rmw16.sub_u (i32.const 0) (i64.const 100))
```

---

## i64.atomic.rmw16.and_u

Atomically read a 16-bit value, perform bitwise AND, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically AND 16-bit value at address with mask, return old value as i64
(i64.atomic.rmw16.and_u (i32.const 0) (i64.const 0x00FF))
```

---

## i64.atomic.rmw16.or_u

Atomically read a 16-bit value, perform bitwise OR, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically OR 16-bit value at address with flags, return old value as i64
(i64.atomic.rmw16.or_u (i32.const 0) (i64.const 0x8000))
```

---

## i64.atomic.rmw16.xor_u

Atomically read a 16-bit value, perform bitwise XOR, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically XOR 16-bit value at address, return old value as i64
(i64.atomic.rmw16.xor_u (i32.const 0) (i64.const 0xFFFF))
```

---

## i64.atomic.rmw16.xchg_u

Atomically exchange a 16-bit value in memory. Returns the original value zero-extended to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically swap 16-bit value at address, return old value as i64
(i64.atomic.rmw16.xchg_u (i32.const 0) (i64.const 1000))
```

---

## i64.atomic.rmw32.add_u

Atomically read a 32-bit value, add to it, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically add to 32-bit value at address, return old value as i64
(i64.atomic.rmw32.add_u (i32.const 0) (i64.const 1000))
```

---

## i64.atomic.rmw32.sub_u

Atomically read a 32-bit value, subtract from it, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically subtract from 32-bit value at address, return old value as i64
(i64.atomic.rmw32.sub_u (i32.const 0) (i64.const 1000))
```

---

## i64.atomic.rmw32.and_u

Atomically read a 32-bit value, perform bitwise AND, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically AND 32-bit value at address with mask, return old value as i64
(i64.atomic.rmw32.and_u (i32.const 0) (i64.const 0xFFFF0000))
```

---

## i64.atomic.rmw32.or_u

Atomically read a 32-bit value, perform bitwise OR, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically OR 32-bit value at address with flags, return old value as i64
(i64.atomic.rmw32.or_u (i32.const 0) (i64.const 0x80000000))
```

---

## i64.atomic.rmw32.xor_u

Atomically read a 32-bit value, perform bitwise XOR, and store the result. Returns the original value zero-extended to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically XOR 32-bit value at address, return old value as i64
(i64.atomic.rmw32.xor_u (i32.const 0) (i64.const 0xFFFFFFFF))
```

---

## i64.atomic.rmw32.xchg_u

Atomically exchange a 32-bit value in memory. Returns the original value zero-extended to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64) (result i64)`

**Example:**

```wat
;; Atomically swap 32-bit value at address, return old value as i64
(i64.atomic.rmw32.xchg_u (i32.const 0) (i64.const 100000))
```

---

## i32.atomic.rmw.cmpxchg

Atomically compare and exchange a 32-bit value. If the value at the address equals the expected value, replace it with the replacement value. Returns the original value. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32 i32) (result i32)`

**Example:**

```wat
;; Compare and exchange: if [addr] == expected, set to replacement
;; Returns original value at address
(i32.atomic.rmw.cmpxchg
  (i32.const 0)       ;; address
  (i32.const 0)       ;; expected value
  (i32.const 1))      ;; replacement value
```

---

## i64.atomic.rmw.cmpxchg

Atomically compare and exchange a 64-bit value. If the value at the address equals the expected value, replace it with the replacement value. Returns the original value. Requires shared memory. The address must be 8-byte aligned.

**Signature:** `(param i32 i64 i64) (result i64)`

**Example:**

```wat
;; Compare and exchange: if [addr] == expected, set to replacement
;; Returns original value at address
(i64.atomic.rmw.cmpxchg
  (i32.const 0)       ;; address
  (i64.const 0)       ;; expected value
  (i64.const 1))      ;; replacement value
```

---

## i32.atomic.rmw8.cmpxchg_u

Atomically compare and exchange an 8-bit value. If the value at the address equals the expected value, replace it with the replacement value. Returns the original value zero-extended to i32. Requires shared memory.

**Signature:** `(param i32 i32 i32) (result i32)`

**Example:**

```wat
;; Compare and exchange byte: if [addr] == expected, set to replacement
(i32.atomic.rmw8.cmpxchg_u
  (i32.const 0)       ;; address
  (i32.const 0)       ;; expected value
  (i32.const 1))      ;; replacement value
```

---

## i32.atomic.rmw16.cmpxchg_u

Atomically compare and exchange a 16-bit value. If the value at the address equals the expected value, replace it with the replacement value. Returns the original value zero-extended to i32. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i32 i32) (result i32)`

**Example:**

```wat
;; Compare and exchange 16-bit: if [addr] == expected, set to replacement
(i32.atomic.rmw16.cmpxchg_u
  (i32.const 0)       ;; address
  (i32.const 0)       ;; expected value
  (i32.const 1))      ;; replacement value
```

---

## i64.atomic.rmw8.cmpxchg_u

Atomically compare and exchange an 8-bit value. If the value at the address equals the expected value, replace it with the replacement value. Returns the original value zero-extended to i64. Requires shared memory.

**Signature:** `(param i32 i64 i64) (result i64)`

**Example:**

```wat
;; Compare and exchange byte: if [addr] == expected, set to replacement
(i64.atomic.rmw8.cmpxchg_u
  (i32.const 0)       ;; address
  (i64.const 0)       ;; expected value
  (i64.const 1))      ;; replacement value
```

---

## i64.atomic.rmw16.cmpxchg_u

Atomically compare and exchange a 16-bit value. If the value at the address equals the expected value, replace it with the replacement value. Returns the original value zero-extended to i64. Requires shared memory. The address must be 2-byte aligned.

**Signature:** `(param i32 i64 i64) (result i64)`

**Example:**

```wat
;; Compare and exchange 16-bit: if [addr] == expected, set to replacement
(i64.atomic.rmw16.cmpxchg_u
  (i32.const 0)       ;; address
  (i64.const 0)       ;; expected value
  (i64.const 1))      ;; replacement value
```

---

## i64.atomic.rmw32.cmpxchg_u

Atomically compare and exchange a 32-bit value. If the value at the address equals the expected value, replace it with the replacement value. Returns the original value zero-extended to i64. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i64 i64) (result i64)`

**Example:**

```wat
;; Compare and exchange 32-bit: if [addr] == expected, set to replacement
(i64.atomic.rmw32.cmpxchg_u
  (i32.const 0)       ;; address
  (i64.const 0)       ;; expected value
  (i64.const 1))      ;; replacement value
```

---

## memory.atomic.wait32

Suspend the current thread until notified or timeout. The thread waits if the 32-bit value at the address equals the expected value. Returns 0 if woken by notify, 1 if value did not match, 2 if timed out. Requires shared memory. The address must be 4-byte aligned. Timeout is in nanoseconds (-1 for infinite).

**Signature:** `(param i32 i32 i64) (result i32)`

**Example:**

```wat
;; Wait on address until value changes or timeout
;; Returns: 0 = woken, 1 = not equal, 2 = timed out
(memory.atomic.wait32
  (i32.const 0)       ;; address
  (i32.const 0)       ;; expected value
  (i64.const -1))     ;; timeout (-1 = infinite)
```

---

## memory.atomic.wait64

Suspend the current thread until notified or timeout. The thread waits if the 64-bit value at the address equals the expected value. Returns 0 if woken by notify, 1 if value did not match, 2 if timed out. Requires shared memory. The address must be 8-byte aligned. Timeout is in nanoseconds (-1 for infinite).

**Signature:** `(param i32 i64 i64) (result i32)`

**Example:**

```wat
;; Wait on address until 64-bit value changes or timeout
;; Returns: 0 = woken, 1 = not equal, 2 = timed out
(memory.atomic.wait64
  (i32.const 0)       ;; address
  (i64.const 0)       ;; expected value
  (i64.const 1000000000)) ;; timeout in nanoseconds (1 second)
```

---

## memory.atomic.notify

Wake up threads waiting on an address. Returns the number of threads that were woken. Requires shared memory. The address must be 4-byte aligned.

**Signature:** `(param i32 i32) (result i32)`

**Example:**

```wat
;; Wake up to 1 waiter at address
(memory.atomic.notify
  (i32.const 0)       ;; address
  (i32.const 1))      ;; max waiters to wake
```

---

## atomic.fence

Ensure memory ordering between atomic and non-atomic operations. This is a sequentially consistent fence that prevents reordering of memory accesses across the fence.

**Signature:** `()`

**Example:**

```wat
;; Memory fence for ordering guarantees
(atomic.fence)
```

---
