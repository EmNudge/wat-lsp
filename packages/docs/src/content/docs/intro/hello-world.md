---
title: Hello World
---

Wasm modules can't do anything on their own — they need a host to run them. This page shows a few small WAT programs and how to call them from JavaScript.

## A minimal module

Every WAT file starts with a `(module ...)` wrapper. Here's the simplest possible module — it does nothing:

```wat
(module)
```

## Returning a value

A function that takes no arguments and returns the number 42:

```wat
(module
  (func (export "hello") (result i32)
    (i32.const 42))
)
```

`(export "hello")` makes the function available to the host under the name `"hello"`. `(result i32)` declares a 32-bit integer return value. `(i32.const 42)` pushes 42 onto the stack, which becomes the return value.

To run this from JavaScript:

```js
const response = await fetch('hello.wasm');
const { instance } = await WebAssembly.instantiateStreaming(response);
console.log(instance.exports.hello()); // 42
```

## Adding two numbers

A function that takes parameters and computes a result:

```wat
(module
  (func (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)
```

`(param $a i32)` declares a parameter named `$a`. `local.get` reads it from the local scope. `i32.add` pops two values off the stack and pushes their sum.

```js
console.log(instance.exports.add(3, 4)); // 7
```

## Calling an import

Wasm modules can call functions provided by the host. Here the module imports a `log` function and calls it with the result of a computation:

```wat
(module
  (import "env" "log" (func $log (param i32)))
  (func (export "run") (param $a i32) (param $b i32)
    (call $log (i32.add (local.get $a) (local.get $b))))
)
```

The host must supply the import at instantiation time:

```js
const importObject = {
  env: { log: (value) => console.log('result:', value) },
};
const response = await fetch('imports.wasm');
const { instance } = await WebAssembly.instantiateStreaming(response, importObject);
instance.exports.run(10, 20); // logs "result: 30"
```

## Compiling and running

To go from `.wat` to `.wasm`, you can use [wat2wasm](https://github.com/WebAssembly/wabt) from the WebAssembly Binary Toolkit or [wasm-tools](https://github.com/bytecodealliance/wasm-tools) from the Bytecode Alliance:

```sh
wat2wasm hello.wat -o hello.wasm
# or
wasm-tools parse hello.wat -o hello.wasm
```

You can also try modules instantly in the browser at [wat2wasm demo](https://webassembly.github.io/wabt/demo/wat2wasm/).

## Keep learning

For a hands-on tutorial, check out [watlings](https://github.com/EmNudge/watlings/) — a set of small exercises where you learn WAT by fixing and completing programs, similar to rustlings.
