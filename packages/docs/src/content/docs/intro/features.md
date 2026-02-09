---
title: Feature Overview
description: A quick tour of WebAssembly features — what shipped in the MVP and what has been added since.
sidebar:
  order: 3
---

WebAssembly launched with a deliberately minimal feature set. New capabilities are added through a [standardization process](https://webassembly.org/features/) that moves proposals through phases before engines ship them.

This page gives a brief overview of what each feature means. For deeper coverage, see the [Extensions](/extensions/mutable-globals/) section of the guides.

## MVP (2017)

The initial release included everything needed for a working compilation target:

- **Four numeric types** — `i32`, `i64`, `f32`, `f64`
- **Linear memory** — a resizable byte array accessed by load/store instructions
- **Tables** — indirect function calls via `call_indirect`
- **Imports and exports** — modules declare what they need and what they provide
- **Structured control flow** — `block`, `loop`, `if`/`else`, `br`, `br_table`
- **A stack machine model** — instructions push and pop values; no registers

This was enough for C/C++ compilers (via Emscripten) to target Wasm effectively.

## Standardized extensions

These proposals have reached phase 5 (standardized) and are shipped in all major engines.

### Numeric & conversion improvements

| Feature                                                   | What it does                                                                         |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| [Sign extension ops](/extensions/sign-extension/)         | `i32.extend8_s`, etc. — sign-extend narrow values without masking tricks             |
| [Non-trapping float-to-int](/extensions/nontrapping-f2i/) | `i32.trunc_sat_f64_s`, etc. — saturating conversions instead of trapping on overflow |
| [JS BigInt ↔ i64](/extensions/js-bigint-i64/)             | Pass `i64` values to/from JavaScript as `BigInt` without manual splitting            |

### Control flow & functions

| Feature                                               | What it does                                                                  |
| ----------------------------------------------------- | ----------------------------------------------------------------------------- |
| [Multi-value](/extensions/multi-value/)               | Functions and blocks can return multiple values; blocks can take parameters   |
| [Tail call](/extensions/tail-call/)                   | `return_call` and `return_call_indirect` — guaranteed tail-call optimization  |
| [Exception handling](/extensions/exception-handling/) | `try_table`, `throw`, `catch` — structured exceptions without unwinding hacks |

### Memory & data

| Feature                                         | What it does                                                                               |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------ |
| [Mutable globals](/extensions/mutable-globals/) | Globals can be imported/exported as mutable, not just immutable                            |
| [Bulk memory](/extensions/bulk-memory/)         | `memory.copy`, `memory.fill`, `table.copy` — efficient bulk operations                     |
| [Extended const](/extensions/extended-const/)   | Use `i32.add`, `i32.mul`, `global.get` in constant expressions (global initializers, etc.) |
| [Memory64](/extensions/memory64/)               | Address memory with 64-bit indices for >4 GB heaps                                         |

### Types & references

| Feature                                                   | What it does                                                                                         |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| [Reference types](/extensions/reference-types/)           | `externref` and `funcref` — opaque host references that Wasm can store but not forge                 |
| [Typed function references](/extensions/typed-func-refs/) | `ref.func`, `call_ref` — first-class typed function references, enabling direct calls without tables |
| [Garbage collection (GC)](/extensions/wasm-gc/)           | `struct`, `array`, `ref.cast` — managed heap types for languages like Java, Kotlin, Dart             |

### SIMD & parallelism

| Feature                               | What it does                                                                           |
| ------------------------------------- | -------------------------------------------------------------------------------------- |
| [Fixed-width SIMD](/extensions/simd/) | 128-bit `v128` type with lane-wise operations (`i32x4.add`, `f64x2.mul`, etc.)         |
| [Threads](/extensions/threads/)       | Shared memory, `memory.atomic.*` instructions, and `wait`/`notify` for synchronization |

## Tracking proposals

The full list of proposals and their current phase is maintained at [webassembly.org/features](https://webassembly.org/features/) and in the [proposals repository](https://github.com/WebAssembly/proposals). Notable in-progress work includes:

- **Stack switching** — lightweight coroutines and green threads
- **Branch hinting** — hint to engines which branch is more likely
- **Relaxed SIMD** — faster SIMD by relaxing determinism constraints
- **Shared-everything threads** — share more than just memory across threads
- **Component model** — high-level module composition and interface types
