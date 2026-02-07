---
title: Interfacing with Hosts
description: Import functions, export APIs, and share memory to call WAT modules from JavaScript and other hosts.
---

Most real programs talk to a host (like JavaScript) via imports/exports and linear memory.

## Imports and exports

```wat
(module
  ;; Import a function: env.log(i32) -> ()
  (import "env" "log" (func $log (param i32)))

  ;; Declare and export a memory
  (memory $mem 1)
  (export "memory" (memory $mem))

  ;; Export a function
  (func $add (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)
```

- **Imports** bring host functionality in, namespaced by module string (e.g. `"env"`).
- **Exports** make module items available to the host by name.

## Calling from JavaScript

```javascript
const wasmBytes = await fetch('/module.wasm').then((r) => r.arrayBuffer());

const imports = {
  env: {
    log: (x) => console.log('WASM says:', x),
  },
};

const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
console.log(instance.exports.add(20, 22)); // 42
```

- The JS import object shape must match your `(import ...)` declarations.
- Exported functions are callable JS functions. Memories and tables are objects.

## Working with linear memory

Share bytes via a `memory` export. From JS, view it as a typed array.

```wat
(module
  (memory (export "memory") 1)

  (func (export "write_byte") (param $offset i32) (param $value i32)
    (i32.store8 (local.get $offset) (local.get $value)))
)
```

Then from JS, view the memory as a typed array:

```javascript
const { instance } = await WebAssembly.instantiate(wasmBytes);
const mem = new Uint8Array(instance.exports.memory.buffer);

instance.exports.write_byte(0, 0x48); // 'H'
instance.exports.write_byte(1, 0x69); // 'i'
console.log(new TextDecoder().decode(mem.subarray(0, 2))); // "Hi"
```

Memory is grown in 64 KiB pages. You can allow growth with `(memory min max)`. After growth the underlying `ArrayBuffer` can change — refresh your typed array views.

## Passing strings

Wasm deals in bytes, so strings are a convention: encode to bytes on the host, pass a pointer and length to Wasm, and decode on the other side.

```javascript
function writeString(memU8, offset, str) {
  const bytes = new TextEncoder().encode(str);
  memU8.set(bytes, offset);
  return bytes.length;
}
```

## Tables and function references

```wat
(module
  (type $t0 (func (param i32) (result i32)))
  (func $inc (type $t0) (param $x i32) (result i32)
    (i32.add (local.get $x) (i32.const 1)))

  (table 1 funcref)
  (elem (i32.const 0) $inc)

  (func (export "call0") (param $n i32) (result i32)
    (call_indirect (type $t0) (local.get $n) (i32.const 0)))
)
```

The last argument to `call_indirect` is the table element index; preceding arguments are passed to the function.

## Instruction Reference

- [Module Structure](/instructions/module) — `module`, `func`, `import`, `export`, `memory`, `table`
- [Memory Instructions](/instructions/memory) — `memory.size`, `memory.grow`
- [Table Instructions](/instructions/table) — `table.get`, `table.set`, `table.grow`
- [Control Flow](/instructions/control) — `call`, `call_indirect`
