---
title: 'Reference Types (externref)'
description: 'Hold references to host objects and use function references.'
---

This extension lets Wasm store references to host objects (`externref`) and introduces more flexible function references.

See the [Reference Types](/reference-types/) page for fundamentals. Use with tables and `call_indirect` for dynamic dispatch.

## Instruction Reference

- [Reference Instructions](/instructions/reference) — `ref.null`, `ref.func`, `ref.is_null`
- [Table Instructions](/instructions/table) — `table.get`, `table.set`
- [Type Names](/instructions/types) — `funcref`, `externref`
