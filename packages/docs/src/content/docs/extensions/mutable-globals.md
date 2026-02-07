---
title: 'Import/Export of Mutable Globals'
description: 'Share and modify mutable globals between JS and Wasm.'
---

## Export a mutable global from Wasm

```wat
(module
  (global (export "counter") (mut i32) (i32.const 0))
)
```

From JS, read and write via `.value`:

```javascript
const { instance } = await WebAssembly.instantiate(wasmBytes);
console.log(instance.exports.counter.value); // 0
instance.exports.counter.value = 5;
console.log(instance.exports.counter.value); // 5
```

## Import a mutable global from JS

```wat
(module
  (import "env" "g" (global $g (mut i32)))
  (func (export "inc")
    (global.set $g (i32.add (global.get $g) (i32.const 1))))
)
```

Create a `WebAssembly.Global` on the JS side and pass it as an import:

```javascript
const g = new WebAssembly.Global({ value: 'i32', mutable: true }, 0);
const { instance } = await WebAssembly.instantiate(wasmBytes, { env: { g } });
instance.exports.inc();
console.log(g.value); // 1
```

## Instruction Reference

- [Local & Global Instructions](/instructions/local-global) — `global.get`, `global.set`
- [Module Structure](/instructions/module) — `global`, `import`, `export`
