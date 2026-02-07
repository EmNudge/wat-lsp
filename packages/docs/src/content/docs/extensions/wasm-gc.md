---
title: 'Garbage Collection (WasmGC)'
description: 'Low-level primitives for managed structs/arrays to support GC languages.'
---

WasmGC provides reference-typed objects (structs, arrays) that engines can manage with garbage collection. This enables efficient runtimes for languages like Kotlin, Java, and Dart.

Key concepts:

- Reference-typed heaps managed by the VM.
- Typed `struct` and `array` values.
- Interop with host via `externref`.

Syntax and support are still evolving — check your toolchain's documentation for exact WAT forms.

## Instruction Reference

- [GC Types](/instructions/gc-types) — `sub`, `final`, `rec`, `field`, `struct`, `array`
- [GC Struct](/instructions/gc-struct) — `struct.new`, `struct.get`, `struct.set`
- [GC Array](/instructions/gc-array) — `array.new`, `array.get`, `array.set`, `array.len`
- [GC i31](/instructions/gc-i31) — `ref.i31`, `i31.get_s`, `i31.get_u`
- [GC Casts](/instructions/gc-casts) — `ref.test`, `ref.cast`, `br_on_cast`
