;; Valid relaxed SIMD — should produce no diagnostics
(module
  (func $madd (param $a v128) (param $b v128) (param $c v128) (result v128)
    local.get $a
    local.get $b
    local.get $c
    f32x4.relaxed_madd)

  (func $swizzle (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    i8x16.relaxed_swizzle)

  (func $trunc (param $a v128) (result v128)
    local.get $a
    i32x4.relaxed_trunc_f32x4_s)

  (func $laneselect (param $a v128) (param $b v128) (param $m v128) (result v128)
    local.get $a
    local.get $b
    local.get $m
    i32x4.relaxed_laneselect)
)
