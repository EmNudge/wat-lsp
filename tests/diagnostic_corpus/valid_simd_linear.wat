;; Valid SIMD in linear style - should produce no diagnostics
;; Regression test for SIMD stack arity inference
(module
  (func $add (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    i32x4.add)

  (func $neg (param $a v128) (result v128)
    local.get $a
    f32x4.neg)

  (func $splat (param $x i32) (result v128)
    local.get $x
    i32x4.splat)

  (func $bitwise (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    v128.and)

  (func $not (param $a v128) (result v128)
    local.get $a
    v128.not)

  (func $bitselect (param $a v128) (param $b v128) (param $m v128) (result v128)
    local.get $a
    local.get $b
    local.get $m
    v128.bitselect)
)
