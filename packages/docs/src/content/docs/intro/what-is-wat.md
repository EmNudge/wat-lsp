---
title: What is WAT?
sidebar:
  order: 1
---

**WAT** (WebAssembly Text Format) is the human-readable form of WebAssembly.

Where `.wasm` files contain compact binary bytecode, `.wat` files express the same instructions as plain text you can read and edit directly.

```wat
(module
  ;; import log
  (import "env" "log" (func $log (param i32)))
  ;; export the import
  (export "log" (func $log))
)
```

## WebAssembly in brief

WebAssembly (Wasm) is a portable compilation target — a low-level language that cleanly maps to most hardware architectures without significant performance loss.

Languages like C, C++, Rust, and Go can compile to Wasm. Runtimes exist that can execute it in browsers, servers, and embedded environments.

Wasm uses a **deny-by-default** security model: modules cannot access anything (files, network, DOM) unless the host explicitly provides it via imports. This makes it well-suited for running untrusted code safely.

## How WAT relates to Wasm

WAT and Wasm are two representations of the same thing.

The conversion between them is almost 1:1 — you can compile WAT to Wasm and decompile it back with barely any loss of information. WAT exists so humans can inspect, debug, and hand-write WebAssembly without staring at hex dumps.

A small example:

```wat
(module
  (func (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)
```

```sh title="binary output"
00000000: 0061 736d 0100 0000 0107 0160 027f 7f01  .asm.......`....
00000010: 7f03 0201 0007 0701 0361 6464 0000 0a09  .........add....
00000020: 0107 0020 0020 016a 0b                   ... . .j.
```

This module exports a single function `add` that takes two 32-bit integers and returns their sum. The parenthesized syntax (S-expressions) may look unusual, but every construct maps directly to a binary instruction.

## Why learn WAT?

- **Understand what compilers produce.** When Rust or C compiles to Wasm, WAT lets you see exactly what ended up in the binary.
- **Debug at the lowest level.** Browser devtools can show you WAT when stepping through Wasm modules.
- **Learn how Wasm actually works.** Writing WAT by hand is the most direct way to understand the stack machine, type system, and module structure.
