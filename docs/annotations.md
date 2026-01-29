# WebAssembly Annotation Documentation

This file contains documentation for WebAssembly annotations. It is parsed at build time to generate hover documentation.

Format:
```
## annotation_name
Description of the annotation.

Example:
\`\`\`wat
example code here
\`\`\`
---
```

## name
Custom name section annotation used for debugging and tooling.

Provides human-readable names for functions, locals, globals, and other WebAssembly constructs. Used by debuggers and source mapping tools.

Example:
```wat
(module (@name "my_module")
  (func $add (@name "addition")
    (param $x i32) (param $y i32) (result i32)
    (i32.add (local.get $x) (local.get $y))))
```
---

## producers
Records information about tools that produced or processed this module.

Contains producer metadata including:
- `language`: Source language (e.g., "Rust", "C++")
- `processed-by`: Tools that processed the module
- `sdk`: SDK used for compilation

Example:
```wat
(module
  (@producers
    (language "Rust" "1.70.0")
    (processed-by "rustc" "1.70.0")
    (sdk "wasm-bindgen" "0.2.87")))
```
---

## custom
Custom section annotation for embedding arbitrary data.

Allows embedding custom data sections in the WebAssembly binary. The section name is the first string, followed by content bytes.

Example:
```wat
(module
  (@custom "my_section" "arbitrary data here")
  (@custom "version" "1.0.0"))
```
---

## interface
Component model interface annotation.

Used in WebAssembly Component Model for defining interfaces and their functions. Part of the WASI (WebAssembly System Interface) ecosystem.

Example:
```wat
(@interface func (export "greet")
  (param "name" string)
  (result string))
```
---

## use
Component model import annotation.

References types or resources from other interfaces in the Component Model.

Example:
```wat
(@use (interface "wasi:filesystem/types")
  (type $descriptor))
```
---

## metadata.code.branch_hint
Branch hint metadata for optimization.

Provides hints to the WebAssembly engine about likely branch directions, allowing for better code generation and optimization.

Example:
```wat
(func $likely_true (param $cond i32) (result i32)
  (@metadata.code.branch_hint "\00")  ;; likely false
  (if (result i32) (local.get $cond)
    (then (i32.const 1))
    (else (i32.const 0))))
```
---

## dylink
Dynamic linking metadata.

Contains information needed for dynamic linking of WebAssembly modules, including memory and table requirements.

Example:
```wat
(module
  (@dylink
    (mem-info (memory 1 2 4))
    (needed "libfoo.so")))
```
---
