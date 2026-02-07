---
title: 'Execution Model'
description: 'Stack machine, memory layout, traps, and module instantiation.'
---

Understanding the execution model helps you reason about what your WAT code actually does at runtime.

## Stack machine

WebAssembly is a stack machine. Instructions consume operands from and push results to an implicit operand stack. There are no general-purpose registers.

Consider adding two numbers:

```wat
(module
  (func (result i32)
    (i32.add (i32.const 1) (i32.const 2)))
)
```

The folded S-expression syntax desugars to a flat sequence of stack operations:

1. `i32.const 1` — push `1` onto the stack.
2. `i32.const 2` — push `2` onto the stack.
3. `i32.add` — pop two values, push their sum (`3`).

The final value left on the stack becomes the function's return value.

Each function call creates a **call frame** with its own isolated operand stack, parameters, and locals. Locals are initialized to zero (or `ref.null` for reference types) and are accessed by index or name.

```wat
(module
  (func (param $a i32) (param $b i32) (result i32)
    (local $tmp i32)
    (local.set $tmp (i32.mul (local.get $a) (local.get $b)))
    (i32.add (local.get $tmp) (i32.const 1)))
)
```

## Memory layout

Linear memory is a contiguous, byte-addressable array. It is allocated in **pages** of 65,536 bytes (64 KiB) and can be grown at runtime with `memory.grow`.

All loads and stores use **little-endian** byte order.

```wat
(module
  (memory 1)  ;; 1 page = 64 KiB

  (func (result i32)
    ;; store 42 at byte offset 0
    (i32.store offset=0 (i32.const 0) (i32.const 42))
    ;; load it back
    (i32.load offset=0 (i32.const 0)))
)
```

Load/store instructions accept two immediates:

- `offset=N` — a static byte offset added to the dynamic address on the stack. Defaults to `0`.
- `align=N` — an alignment hint in bytes. Must be a power of 2 and at most the natural alignment of the operation (e.g. 4 for `i32.load`). Defaults to natural alignment. Misaligned access does not trap but may be slower on some platforms.

## Traps

A **trap** is an unrecoverable error that immediately terminates the current WebAssembly execution. The following conditions cause a trap:

- Executing the `unreachable` instruction.
- Integer division by zero (`i32.div_s`, `i32.div_u`, `i64.div_s`, `i64.div_u`, and corresponding `rem` operations).
- Signed integer overflow: `i32.div_s` or `i64.div_s` with `INT_MIN / -1`.
- Out-of-bounds linear memory access (load or store beyond allocated pages).
- Out-of-bounds table access (`table.get`, `table.set`, `call_indirect` with an index past the table size).
- Type mismatch on `call_indirect` — the function at the table index has a different signature than expected.
- Null reference dereference (`ref.as_non_null`, `struct.get`, `array.get`, etc. on a null reference).
- Cast failure on `ref.cast` when the reference does not match the target type.
- Stack overflow — implementation-defined limit on call depth.

Traps cannot be caught within WebAssembly itself. The host environment (e.g. JavaScript) receives the trap as an exception.

## Module instantiation

A WebAssembly module goes through several phases before it can execute:

1. **Validation** — the binary is checked for well-formedness: type correctness, stack consistency, and valid indices. Invalid modules are rejected before any code runs.
2. **Import matching** — each declared import must be satisfied by a value from the host with a compatible type (function signature, memory limits, table limits, global type).
3. **Allocation** — internal memories, tables, and globals are allocated.
4. **Initialization** — runs in a fixed order:
   - **Globals** are initialized from their init expressions.
   - **Tables** receive entries from element segments.
   - **Element segments** with passive mode are available for `table.init`.
   - **Data segments** are copied into memory (active segments) or held for `memory.init` (passive segments).
   - **Start function** runs, if declared.

```wat
(module
  (memory 1)
  (global $base (mut i32) (i32.const 100))

  ;; active data segment: copied into memory at instantiation
  (data (i32.const 0) "hello")

  ;; start function: runs after initialization
  (func $init
    (global.set $base (i32.const 200)))
  (start $init)
)
```

## Determinism

WebAssembly execution is almost fully deterministic. Two conforming engines given the same module and inputs will produce the same results, with a few exceptions:

- **NaN bit patterns** — floating-point operations that produce NaN may have different payload bits across engines. The sign, exponent, and quiet/signaling bit are specified, but the trailing significand bits are not.
- **`memory.grow` and `table.grow` failure** — when the engine cannot allocate more memory, these return `-1`. The threshold at which allocation fails is implementation-defined.
- **Host functions** — imported functions from the host environment may behave differently across embeddings.
- **Thread scheduling** — with the threads proposal, the interleaving of atomic operations across threads is nondeterministic.

For most practical purposes, WebAssembly behaves identically everywhere.

## Instruction Reference

- [Memory](/instructions/memory) — `i32.load`, `i32.store`, `memory.grow`, `memory.size`, etc.
- [Control Flow](/instructions/control) — `block`, `loop`, `if`, `br`, `unreachable`, `call`, `call_indirect`
- [Local & Global](/instructions/local-global) — `local.get`, `local.set`, `global.get`, `global.set`
- [Module Structure](/instructions/module) — `func`, `memory`, `table`, `global`, `data`, `start`
