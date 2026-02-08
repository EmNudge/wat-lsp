;; Tail call return type mismatch: callee returns f64 but caller returns i32
(module
  (func $callee (result f64)
    f64.const 1.0)
  (func $caller (result i32)
    return_call $callee))
