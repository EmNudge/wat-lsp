---
title: 'Text Format'
description: 'WAT lexical syntax, literals, identifiers, and syntactic sugar.'
---

WAT (WebAssembly Text Format) is the human-readable form of WebAssembly. This page covers the lexical building blocks and syntactic sugar that make WAT modules easier to read and write.

## Comments

Line comments start with `;;` and run to the end of the line. Block comments use `(; ;)` and can nest.

```wat
(module
  ;; This is a line comment
  (func $add (param $a i32) (param $b i32) (result i32)
    (; Block comments can span multiple lines. ;)
    (i32.add (local.get $a) (local.get $b)))
)
```

## Identifiers

Names in WAT start with `$` followed by one or more printable ASCII characters (letters, digits, and the punctuation `!`, `#`, `$`, `%`, `&`, `'`, `*`, `+`, `-`, `.`, `/`, `:`, `<`, `=`, `>`, `?`, `@`, `\\`, `^`, `_`, `` ` ``, `|`, `~`).

```wat-snippet
$x
$my_func
$a.b.c
$loop/count
$env.log
```

Identifiers are optional — you can refer to functions, locals, labels, etc. by numeric index instead. Names exist purely for readability and are stripped during compilation to binary.

## Numeric literals

Integers can be written in decimal or hexadecimal, with optional sign prefix and underscore separators.

```wat-snippet
42          ;; decimal
-1          ;; signed
0xFF        ;; hexadecimal
1_000_000   ;; underscores for readability
0x00_FF     ;; also works in hex
```

```wat
(module
  (func (export "constants") (result i32)
    (i32.add (i32.const 1_000) (i32.const 0xFF)))
)
```

The `i32.const` and `i64.const` instructions accept both signed and unsigned values within the type's range.

## Float literals

Floats support decimal notation, scientific notation, hexadecimal float notation, and special IEEE 754 values.

```wat-snippet
1.5         ;; decimal float
1.5e10      ;; scientific notation
0x1.0p10    ;; hex float: 1.0 * 2^10 = 1024.0
inf         ;; positive infinity
-inf        ;; negative infinity
nan         ;; canonical NaN
nan:0x1     ;; NaN with custom payload
```

```wat
(module
  (func (export "specials") (result f64)
    (f64.add (f64.const 1.5e2) (f64.const 0x1.0p3)))
)
```

## String literals

Strings appear in data segments, import/export names, and custom sections. They are enclosed in double quotes and support escape sequences.

| Escape  | Meaning                                              |
| ------- | ---------------------------------------------------- |
| `\t`    | tab (0x09)                                           |
| `\n`    | newline (0x0A)                                       |
| `\r`    | carriage return (0x0D)                               |
| `\"`    | literal double quote                                 |
| `\\`    | literal backslash                                    |
| `\hh`   | raw byte by two hex digits, e.g. `\00`, `\7F`        |
| `\u{N}` | Unicode code point (UTF-8 encoded), e.g. `\u{1F600}` |

```wat
(module
  (memory 1)
  (data (i32.const 0) "hello\n")
  (data (i32.const 6) "tab\there\00")
  (data (i32.const 15) "\u{2603}")       ;; snowman U+2603
)
```

## Folded (S-expression) syntax

WAT supports two equivalent syntactic styles. The **flat** (stack) style mirrors the binary format directly. The **folded** (S-expression) style lets you nest instructions inside parentheses so operands read left-to-right.

Flat form:

```wat
(module
  (func (export "flat") (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add
    i32.const 10
    i32.mul)
)
```

Folded form (equivalent):

```wat
(module
  (func (export "folded") (param $a i32) (param $b i32) (result i32)
    (i32.mul
      (i32.add (local.get $a) (local.get $b))
      (i32.const 10)))
)
```

Both compile to identical binary output. Folded syntax is usually easier to read because it makes the data flow explicit, but flat syntax can be clearer for long linear instruction sequences.

## Inline imports and exports

Import and export declarations can be written inline on `func`, `table`, `memory`, and `global` definitions instead of as separate top-level entries.

Full form:

```wat
(module
  (import "env" "log" (func $log (param i32)))
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
  (export "add" (func $add))
)
```

Inline form (equivalent):

```wat
(module
  (func $log (import "env" "log") (param i32))
  (func $add (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)
```

The same sugar applies to other definitions:

```wat
(module
  (memory (export "mem") 1)                        ;; inline export
  (global $g (import "env" "val") i32)             ;; inline import
  (table (export "tbl") 1 funcref)                 ;; inline export
)
```

## Inline type declarations

When a function's signature is written directly with `param` and `result`, the toolchain infers the type. You only need an explicit `(type $t)` reference when sharing a named type across multiple uses.

Explicit type:

```wat
(module
  (type $binop (func (param i32) (param i32) (result i32)))
  (func $add (type $binop)
    (i32.add (local.get 0) (local.get 1)))
)
```

Inline params/results (equivalent for simple cases):

```wat
(module
  (func $add (param i32) (param i32) (result i32)
    (i32.add (local.get 0) (local.get 1)))
)
```

Explicit type references are required for `call_indirect`, where the runtime checks the callee's signature against the expected type.

## Inline data and element segments

Memory and table definitions can include inline data or element content. The toolchain expands them into separate segment declarations.

Inline data segment — declares a one-page memory and fills it starting at offset 0:

```wat
(module
  (memory (data "hello"))
  (func (export "load") (result i32)
    (i32.load8_u (i32.const 0)))  ;; returns 104 ('h')
)
```

Inline element segment — declares a table pre-filled with function references:

```wat
(module
  (type $sig (func (result i32)))
  (func $one (result i32) (i32.const 1))
  (func $two (result i32) (i32.const 2))
  (table funcref (elem $one $two))
  (func (export "call_first") (result i32)
    (call_indirect (type $sig) (i32.const 0)))
)
```

These shorthands are convenient for small modules and tests. For larger data payloads, separate `(data ...)` segments with explicit offsets give more control.

## Instruction Reference

- [Module Structure](/instructions/module) — `module`, `func`, `import`, `export`, `memory`, `table`, `global`, `type`, `data`, `elem`
