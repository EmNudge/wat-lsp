---
title: 'call & call_indirect'
description: Direct calls to known functions and indirect calls via tables.
---

`call` invokes a function by index or name. `call_indirect` dispatches through a table and type-checks the signature at runtime.

```wat
(module
  (type $t0 (func (param i32) (result i32)))

  (func $double (type $t0) (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func (export "use_call") (param $n i32) (result i32)
    (call $double (local.get $n)))

  (table 1 funcref)
  (elem (i32.const 0) $double)

  (func (export "use_call_indirect") (param $n i32) (result i32)
    (call_indirect (type $t0) (local.get $n) (i32.const 0)))
)
```

- For `call_indirect`, the last argument is the table element index. Preceding arguments are passed to the function.
- The function at that index must match the declared `(type ...)`, or the call traps.

## Instruction Reference

- [Control Flow Instructions](/instructions/control) — `call`, `call_indirect`, `call_ref`
- [Table Instructions](/instructions/table) — `table.get`, `table.set`, `table.grow`
