# WebAssembly Instruction Documentation

This file contains documentation for WebAssembly instructions. It is parsed at build time to generate hover documentation.

Format:
```
## instruction.name
Description of the instruction.

Signature: `(param types) (result types)`

Example:
\`\`\`wat
example code here
\`\`\`
---
```

## i32.add
Add two i32 values.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.add (i32.const 5) (i32.const 3))  ;; Returns 8
```
---

## i32.sub
Subtract the second i32 value from the first.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.sub (i32.const 10) (i32.const 3))  ;; Returns 7
```
---

## i32.mul
Multiply two i32 values.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.mul (i32.const 4) (i32.const 5))  ;; Returns 20
```
---

## i32.div_s
Signed division of two i32 values. Traps if divisor is zero.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.div_s (i32.const -10) (i32.const 3))  ;; Returns -3
```
---

## i32.div_u
Unsigned division of two i32 values. Traps if divisor is zero.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.div_u (i32.const 10) (i32.const 3))  ;; Returns 3
```
---

## i32.rem_s
Signed remainder of division.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.rem_s (i32.const 10) (i32.const 3))  ;; Returns 1
```
---

## i32.rem_u
Unsigned remainder of division.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.rem_u (i32.const 10) (i32.const 3))  ;; Returns 1
```
---

## i32.and
Bitwise AND of two i32 values.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.and (i32.const 0b1100) (i32.const 0b1010))  ;; Returns 0b1000 (8)
```
---

## i32.or
Bitwise OR of two i32 values.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.or (i32.const 0b1100) (i32.const 0b1010))  ;; Returns 0b1110 (14)
```
---

## i32.xor
Bitwise XOR of two i32 values.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.xor (i32.const 0b1100) (i32.const 0b1010))  ;; Returns 0b0110 (6)
```
---

## i32.shl
Shift left.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.shl (i32.const 5) (i32.const 2))  ;; Returns 20 (5 << 2)
```
---

## i32.shr_s
Signed shift right (arithmetic shift).

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.shr_s (i32.const -8) (i32.const 2))  ;; Returns -2 (preserves sign)
```
---

## i32.shr_u
Unsigned shift right (logical shift).

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.shr_u (i32.const 20) (i32.const 2))  ;; Returns 5
```
---

## i32.rotl
Rotate left.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.rotl (i32.const 0x12345678) (i32.const 4))
```
---

## i32.rotr
Rotate right.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.rotr (i32.const 0x12345678) (i32.const 4))
```
---

## i32.clz
Count leading zeros.

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.clz (i32.const 0x00FF0000))  ;; Returns 8
```
---

## i32.ctz
Count trailing zeros.

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.ctz (i32.const 0xFF000000))  ;; Returns 24
```
---

## i32.popcnt
Count number of 1 bits (population count).

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.popcnt (i32.const 0b1101))  ;; Returns 3
```
---

## i32.eq
Check if two i32 values are equal.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.eq (i32.const 5) (i32.const 5))  ;; Returns 1 (true)
(i32.eq (i32.const 5) (i32.const 3))  ;; Returns 0 (false)
```
---

## i32.ne
Check if two i32 values are not equal.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.ne (i32.const 5) (i32.const 3))  ;; Returns 1 (true)
```
---

## i32.eqz
Check if i32 value equals zero.

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.eqz (i32.const 0))  ;; Returns 1 (true)
(i32.eqz (i32.const 5))  ;; Returns 0 (false)
```
---

## i32.lt_s
Signed less than comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.lt_s (i32.const -5) (i32.const 3))  ;; Returns 1 (true)
```
---

## i32.lt_u
Unsigned less than comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.lt_u (i32.const 3) (i32.const 5))  ;; Returns 1 (true)
```
---

## i32.gt_s
Signed greater than comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.gt_s (i32.const 5) (i32.const 3))  ;; Returns 1 (true)
```
---

## i32.gt_u
Unsigned greater than comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.gt_u (i32.const 5) (i32.const 3))  ;; Returns 1 (true)
```
---

## i32.le_s
Signed less than or equal comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.le_s (i32.const 5) (i32.const 5))  ;; Returns 1 (true)
```
---

## i32.le_u
Unsigned less than or equal comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.le_u (i32.const 5) (i32.const 5))  ;; Returns 1 (true)
```
---

## i32.ge_s
Signed greater than or equal comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.ge_s (i32.const 5) (i32.const 5))  ;; Returns 1 (true)
```
---

## i32.ge_u
Unsigned greater than or equal comparison.

Signature: `(param i32 i32) (result i32)`

Example:
```wat
(i32.ge_u (i32.const 5) (i32.const 3))  ;; Returns 1 (true)
```
---

## i32.const
Create a constant i32 value.

Signature: `(result i32)`

Example:
```wat
(i32.const 42)
(i32.const -10)
(i32.const 0xFF)  ;; Hexadecimal
```
---

## i32.load
Load i32 from memory at the given address.

Signature: `(param i32) (result i32)`

Example:
```wat
;; Load from memory at offset 0
(i32.load (i32.const 0))

;; Load with alignment hint
(i32.load offset=4 align=4 (i32.const 0))
```
---

## i32.load8_s
Load signed 8-bit value from memory and sign-extend to i32.

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.load8_s (i32.const 100))  ;; Load byte at address 100
```
---

## i32.load8_u
Load unsigned 8-bit value from memory and zero-extend to i32.

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.load8_u (i32.const 100))
```
---

## i32.load16_s
Load signed 16-bit value from memory and sign-extend to i32.

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.load16_s (i32.const 100))
```
---

## i32.load16_u
Load unsigned 16-bit value from memory and zero-extend to i32.

Signature: `(param i32) (result i32)`

Example:
```wat
(i32.load16_u (i32.const 100))
```
---

## i32.store
Store i32 value to memory.

Signature: `(param i32 i32)`

Example:
```wat
;; Store 42 at memory address 0
(i32.store (i32.const 0) (i32.const 42))

;; Store with offset
(i32.store offset=8 (i32.const 0) (i32.const 100))
```
---

## i32.store8
Store low 8 bits of i32 to memory.

Signature: `(param i32 i32)`

Example:
```wat
(i32.store8 (i32.const 100) (i32.const 0xFF))
```
---

## i32.store16
Store low 16 bits of i32 to memory.

Signature: `(param i32 i32)`

Example:
```wat
(i32.store16 (i32.const 100) (i32.const 0xFFFF))
```
---

## i64.add
Add two i64 values.

Signature: `(param i64 i64) (result i64)`

Example:
```wat
(i64.add (i64.const 5000000000) (i64.const 3000000000))
```
---

## i64.sub
Subtract two i64 values.

Signature: `(param i64 i64) (result i64)`

Example:
```wat
(i64.sub (i64.const 10) (i64.const 3))
```
---

## i64.mul
Multiply two i64 values.

Signature: `(param i64 i64) (result i64)`

Example:
```wat
(i64.mul (i64.const 1000) (i64.const 1000))
```
---

## i64.const
Create a constant i64 value.

Signature: `(result i64)`

Example:
```wat
(i64.const 9223372036854775807)  ;; Max i64
(i64.const -1)
```
---

## i64.eqz
Check if i64 value equals zero.

Signature: `(param i64) (result i32)`

Example:
```wat
(i64.eqz (i64.const 0))  ;; Returns 1
```
---

## f32.add
Add two f32 values.

Signature: `(param f32 f32) (result f32)`

Example:
```wat
(f32.add (f32.const 3.14) (f32.const 2.86))
```
---

## f32.sub
Subtract two f32 values.

Signature: `(param f32 f32) (result f32)`

Example:
```wat
(f32.sub (f32.const 10.5) (f32.const 3.2))
```
---

## f32.mul
Multiply two f32 values.

Signature: `(param f32 f32) (result f32)`

Example:
```wat
(f32.mul (f32.const 3.5) (f32.const 2.0))
```
---

## f32.div
Divide two f32 values.

Signature: `(param f32 f32) (result f32)`

Example:
```wat
(f32.div (f32.const 10.0) (f32.const 3.0))
```
---

## f32.sqrt
Calculate square root.

Signature: `(param f32) (result f32)`

Example:
```wat
(f32.sqrt (f32.const 16.0))  ;; Returns 4.0
```
---

## f32.min
Return minimum of two f32 values.

Signature: `(param f32 f32) (result f32)`

Example:
```wat
(f32.min (f32.const 3.5) (f32.const 2.1))  ;; Returns 2.1
```
---

## f32.max
Return maximum of two f32 values.

Signature: `(param f32 f32) (result f32)`

Example:
```wat
(f32.max (f32.const 3.5) (f32.const 2.1))  ;; Returns 3.5
```
---

## f32.abs
Absolute value.

Signature: `(param f32) (result f32)`

Example:
```wat
(f32.abs (f32.const -3.14))  ;; Returns 3.14
```
---

## f32.neg
Negate value.

Signature: `(param f32) (result f32)`

Example:
```wat
(f32.neg (f32.const 3.14))  ;; Returns -3.14
```
---

## f32.ceil
Round up to nearest integer.

Signature: `(param f32) (result f32)`

Example:
```wat
(f32.ceil (f32.const 3.2))  ;; Returns 4.0
```
---

## f32.floor
Round down to nearest integer.

Signature: `(param f32) (result f32)`

Example:
```wat
(f32.floor (f32.const 3.8))  ;; Returns 3.0
```
---

## f32.trunc
Round toward zero.

Signature: `(param f32) (result f32)`

Example:
```wat
(f32.trunc (f32.const 3.8))   ;; Returns 3.0
(f32.trunc (f32.const -3.8))  ;; Returns -3.0
```
---

## f32.nearest
Round to nearest integer, ties to even.

Signature: `(param f32) (result f32)`

Example:
```wat
(f32.nearest (f32.const 3.5))  ;; Returns 4.0
(f32.nearest (f32.const 2.5))  ;; Returns 2.0 (ties to even)
```
---

## f32.const
Create a constant f32 value.

Signature: `(result f32)`

Example:
```wat
(f32.const 3.14159)
(f32.const -0.0)
(f32.const inf)
(f32.const nan)
```
---

## f64.add
Add two f64 values.

Signature: `(param f64 f64) (result f64)`

Example:
```wat
(f64.add (f64.const 3.14159) (f64.const 2.71828))
```
---

## f64.sqrt
Calculate square root with double precision.

Signature: `(param f64) (result f64)`

Example:
```wat
(f64.sqrt (f64.const 2.0))  ;; Returns 1.4142135623730951
```
---

## f64.const
Create a constant f64 value.

Signature: `(result f64)`

Example:
```wat
(f64.const 3.141592653589793)
(f64.const 1.7976931348623157e+308)  ;; Max f64
```
---

## local.get
Get the value of a local variable by name or index.

Signature: Varies based on local type

Example:
```wat
(func $example (param $x i32) (result i32)
  (local $temp i32)
  (local.get $x)      ;; Get parameter
  (local.get $temp)   ;; Get local
  (local.get 0)       ;; Get by index
)
```
---

## local.set
Set the value of a local variable by name or index.

Signature: Varies based on local type

Example:
```wat
(func $example (param $x i32)
  (local $result i32)
  (local.set $result (i32.const 42))
  (local.set 1 (i32.const 100))  ;; Set by index
)
```
---

## local.tee
Set the value of a local variable and return it (combination of set and get).

Signature: Varies based on local type

Example:
```wat
(func $example (result i32)
  (local $x i32)
  ;; Set $x to 42 and also return it
  (local.tee $x (i32.const 42))
)
```
---

## global.get
Get the value of a global variable by name or index.

Signature: Varies based on global type

Example:
```wat
(global $counter (mut i32) (i32.const 0))

(func $read_counter (result i32)
  (global.get $counter)
)
```
---

## global.set
Set the value of a global variable by name or index. Only works on mutable globals.

Signature: Varies based on global type

Example:
```wat
(global $counter (mut i32) (i32.const 0))

(func $increment
  (global.set $counter
    (i32.add (global.get $counter) (i32.const 1)))
)
```
---

## block
Define a block with a label at the end. Branching to this label exits the block.

Signature: Varies

Example:
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

Signature: Varies

Example:
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

Signature: Varies

Example:
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

## br
Unconditional branch to a label. Exits blocks/loops.

Signature: Varies

Example:
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

Signature: `(param i32)`

Example:
```wat
(block $exit
  (br_if $exit (i32.eq (local.get $x) (i32.const 0)))
  ;; Code here runs if $x != 0
)
```
---

## br_table
Table-based branch. Jumps to label based on index.

Signature: `(param i32)`

Example:
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

Signature: Varies based on function

Example:
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

Signature: Varies

Example:
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

Example:
```wat
(func $early_return (param $x i32) (result i32)
  (if (i32.eqz (local.get $x))
    (then (return (i32.const 0))))
  ;; More code here
  (i32.const 1)
)
```
---

## drop
Remove the top value from the stack.

Example:
```wat
(i32.const 42)
(drop)  ;; Remove 42 from stack
```
---

## select
Select one of two values based on a condition.

Signature: `(param T T i32) (result T)`

Example:
```wat
;; Returns first value if condition is non-zero, else second
(select
  (i32.const 10)
  (i32.const 20)
  (i32.const 1))  ;; Returns 10

;; Can specify type
(select (result i32)
  (i32.const 42)
  (i32.const 0)
  (i32.eqz (local.get $x)))
```
---

## unreachable
Trap unconditionally. Used for code that should never be reached.

Example:
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

Example:
```wat
(nop)  ;; Useful for debugging or as placeholder
```
---

## memory.size
Get current memory size in pages (1 page = 64KB).

Signature: `(result i32)`

Example:
```wat
(memory.size)  ;; Returns current number of pages
```
---

## memory.grow
Grow memory by delta pages. Returns previous size, or -1 on failure.

Signature: `(param i32) (result i32)`

Example:
```wat
;; Grow memory by 1 page
(memory.grow (i32.const 1))
;; Returns old size (e.g., 1) or -1 if failed
```
---

## memory.fill
Fill a region of memory with a byte value.

Signature: `(param i32 i32 i32)`

Example:
```wat
;; Fill 100 bytes starting at address 0 with value 0xFF
(memory.fill
  (i32.const 0)    ;; destination
  (i32.const 0xFF) ;; value
  (i32.const 100)) ;; size
```
---

## memory.copy
Copy a region of memory to another location.

Signature: `(param i32 i32 i32)`

Example:
```wat
;; Copy 50 bytes from address 100 to address 200
(memory.copy
  (i32.const 200)  ;; destination
  (i32.const 100)  ;; source
  (i32.const 50))  ;; size
```
---

## table.get
Get element from table at index.

Signature: `(param i32) (result reftype)`

Example:
```wat
(table $funcs 10 funcref)
(table.get $funcs (i32.const 0))
```
---

## table.set
Set element in table at index.

Signature: `(param i32 reftype)`

Example:
```wat
(table $funcs 10 funcref)
(table.set $funcs
  (i32.const 0)
  (ref.func $my_function))
```
---

## table.size
Get current table size.

Signature: `(result i32)`

Example:
```wat
(table.size $funcs)
```
---

## table.grow
Grow table by delta, returns previous size or -1 on failure.

Signature: `(param reftype i32) (result i32)`

Example:
```wat
(table.grow $funcs
  (ref.null func)
  (i32.const 5))  ;; Grow by 5 elements
```
---

## ref.null
Create a null reference.

Signature: `(result reftype)`

Example:
```wat
(ref.null func)
(ref.null extern)
```
---

## ref.func
Create a function reference.

Signature: `(result funcref)`

Example:
```wat
(func $my_func (result i32) (i32.const 42))
(ref.func $my_func)
```
---

## ref.is_null
Check if reference is null.

Signature: `(param reftype) (result i32)`

Example:
```wat
(ref.is_null (ref.null func))  ;; Returns 1
```
---

## module
Declares a WebAssembly module. Top-level container for all declarations.

Example:
```wat
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

Example:
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

Example:
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

Example:
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

Example:
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

Example:
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

Example:
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

Example:
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

Example:
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

Example:
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

## type
Declares a function type that can be referenced elsewhere.

Example:
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

Example:
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

Example:
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

## i32
32-bit integer type. Not inherently signed or unsigned - interpretation depends on the operation.

Example:
```wat
(local $x i32)
(global $counter (mut i32) (i32.const 0))
(param $value i32)
```
---

## i64
64-bit integer type. Not inherently signed or unsigned - interpretation depends on the operation.

Example:
```wat
(local $timestamp i64)
(global $big_number i64 (i64.const 9223372036854775807))
```
---

## f32
32-bit floating point type (IEEE 754-2019 single precision).

Example:
```wat
(local $pi f32)
(global $epsilon f32 (f32.const 1.1920929e-07))
```
---

## f64
64-bit floating point type (IEEE 754-2019 double precision).

Example:
```wat
(local $precise f64)
(global $e f64 (f64.const 2.718281828459045))
```
---

## funcref
Reference type for functions. Can be null.

Example:
```wat
(table $callbacks 10 funcref)
(global $current_handler (mut funcref) (ref.null func))
```
---

## externref
Reference type for external (host) values. Can be null.

Example:
```wat
(table $objects 10 externref)
(func $process (param $obj externref) ...)
```
---
