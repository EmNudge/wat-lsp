;; Comprehensive SIMD instruction test file
;; This file tests all SIMD instructions to catch grammar regressions
;; Run with: cargo run --bin wat-check -- tests/fixtures/simd_comprehensive.wat

(module
  (memory 1)

  ;; ============================================================================
  ;; v128 load/store basic
  ;; ============================================================================
  (func $test_v128_load_store (local $v v128)
    ;; Basic load/store
    (local.set $v (v128.load (i32.const 0)))
    (v128.store (i32.const 0) (local.get $v))

    ;; With offset and align
    (local.set $v (v128.load offset=16 align=16 (i32.const 0)))
    (v128.store offset=32 align=16 (i32.const 0) (local.get $v))
  )

  ;; ============================================================================
  ;; v128 extending loads (8x8, 16x4, 32x2)
  ;; ============================================================================
  (func $test_v128_extending_loads (result v128)
    (v128.load8x8_s (i32.const 0))
  )
  (func $test_v128_extending_loads_u (result v128)
    (v128.load8x8_u (i32.const 0))
  )
  (func $test_v128_load16x4_s (result v128)
    (v128.load16x4_s (i32.const 0))
  )
  (func $test_v128_load16x4_u (result v128)
    (v128.load16x4_u (i32.const 0))
  )
  (func $test_v128_load32x2_s (result v128)
    (v128.load32x2_s (i32.const 0))
  )
  (func $test_v128_load32x2_u (result v128)
    (v128.load32x2_u (i32.const 0))
  )

  ;; ============================================================================
  ;; v128 splat loads
  ;; ============================================================================
  (func $test_v128_splat_loads (result v128)
    (v128.load8_splat (i32.const 0))
  )
  (func $test_v128_load16_splat (result v128)
    (v128.load16_splat (i32.const 0))
  )
  (func $test_v128_load32_splat (result v128)
    (v128.load32_splat (i32.const 0))
  )
  (func $test_v128_load64_splat (result v128)
    (v128.load64_splat (i32.const 0))
  )

  ;; ============================================================================
  ;; v128 zero-extending loads (IMPORTANT: these were previously missing!)
  ;; ============================================================================
  (func $test_v128_load32_zero (result v128)
    (v128.load32_zero (i32.const 0))
  )
  (func $test_v128_load64_zero (result v128)
    (v128.load64_zero (i32.const 0))
  )
  (func $test_v128_load64_zero_with_offset (result v128)
    (v128.load64_zero offset=8 (i32.const 0))
  )
  (func $test_v128_load32_zero_with_align (result v128)
    (v128.load32_zero align=4 (i32.const 0))
  )

  ;; ============================================================================
  ;; v128 lane loads (IMPORTANT: these were previously missing!)
  ;; ============================================================================
  (func $test_v128_load8_lane (param $v v128) (result v128)
    (v128.load8_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_load16_lane (param $v v128) (result v128)
    (v128.load16_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_load32_lane (param $v v128) (result v128)
    (v128.load32_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_load64_lane (param $v v128) (result v128)
    (v128.load64_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_load64_lane_with_offset (param $v v128) (result v128)
    (v128.load64_lane offset=8 1 (i32.const 0) (local.get $v))
  )

  ;; ============================================================================
  ;; v128 lane stores (IMPORTANT: these were previously missing!)
  ;; ============================================================================
  (func $test_v128_store8_lane (param $v v128)
    (v128.store8_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_store16_lane (param $v v128)
    (v128.store16_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_store32_lane (param $v v128)
    (v128.store32_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_store64_lane (param $v v128)
    (v128.store64_lane 0 (i32.const 0) (local.get $v))
  )
  (func $test_v128_store64_lane_with_offset (param $v v128)
    (v128.store64_lane offset=8 1 (i32.const 0) (local.get $v))
  )

  ;; ============================================================================
  ;; v128.const
  ;; ============================================================================
  (func $test_v128_const (result v128)
    (v128.const i32x4 1 2 3 4)
  )
  (func $test_v128_const_f32x4 (result v128)
    (v128.const f32x4 1.0 2.0 3.0 4.0)
  )
  (func $test_v128_const_i8x16 (result v128)
    (v128.const i8x16 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15)
  )
  (func $test_v128_const_i64x2 (result v128)
    (v128.const i64x2 100 200)
  )
  (func $test_v128_const_f64x2 (result v128)
    (v128.const f64x2 1.5 2.5)
  )

  ;; ============================================================================
  ;; i8x16 shuffle
  ;; ============================================================================
  (func $test_i8x16_shuffle (param $a v128) (param $b v128) (result v128)
    (i8x16.shuffle 0 1 2 3 4 5 6 7 16 17 18 19 20 21 22 23
                   (local.get $a) (local.get $b))
  )

  ;; ============================================================================
  ;; extract_lane / replace_lane
  ;; ============================================================================
  (func $test_i8x16_extract_lane_s (param $v v128) (result i32)
    (i8x16.extract_lane_s 0 (local.get $v))
  )
  (func $test_i8x16_extract_lane_u (param $v v128) (result i32)
    (i8x16.extract_lane_u 15 (local.get $v))
  )
  (func $test_i16x8_extract_lane_s (param $v v128) (result i32)
    (i16x8.extract_lane_s 0 (local.get $v))
  )
  (func $test_i16x8_extract_lane_u (param $v v128) (result i32)
    (i16x8.extract_lane_u 7 (local.get $v))
  )
  (func $test_i32x4_extract_lane (param $v v128) (result i32)
    (i32x4.extract_lane 0 (local.get $v))
  )
  (func $test_i64x2_extract_lane (param $v v128) (result i64)
    (i64x2.extract_lane 1 (local.get $v))
  )
  (func $test_f32x4_extract_lane (param $v v128) (result f32)
    (f32x4.extract_lane 2 (local.get $v))
  )
  (func $test_f64x2_extract_lane (param $v v128) (result f64)
    (f64x2.extract_lane 0 (local.get $v))
  )

  (func $test_i8x16_replace_lane (param $v v128) (param $val i32) (result v128)
    (i8x16.replace_lane 5 (local.get $v) (local.get $val))
  )
  (func $test_i32x4_replace_lane (param $v v128) (param $val i32) (result v128)
    (i32x4.replace_lane 3 (local.get $v) (local.get $val))
  )
  (func $test_f32x4_replace_lane (param $v v128) (param $val f32) (result v128)
    (f32x4.replace_lane 1 (local.get $v) (local.get $val))
  )

  ;; ============================================================================
  ;; Splat operations
  ;; ============================================================================
  (func $test_i8x16_splat (param $val i32) (result v128)
    (i8x16.splat (local.get $val))
  )
  (func $test_i16x8_splat (param $val i32) (result v128)
    (i16x8.splat (local.get $val))
  )
  (func $test_i32x4_splat (param $val i32) (result v128)
    (i32x4.splat (local.get $val))
  )
  (func $test_i64x2_splat (param $val i64) (result v128)
    (i64x2.splat (local.get $val))
  )
  (func $test_f32x4_splat (param $val f32) (result v128)
    (f32x4.splat (local.get $val))
  )
  (func $test_f64x2_splat (param $val f64) (result v128)
    (f64x2.splat (local.get $val))
  )

  ;; ============================================================================
  ;; i32x4 arithmetic
  ;; ============================================================================
  (func $test_i32x4_add (param $a v128) (param $b v128) (result v128)
    (i32x4.add (local.get $a) (local.get $b))
  )
  (func $test_i32x4_sub (param $a v128) (param $b v128) (result v128)
    (i32x4.sub (local.get $a) (local.get $b))
  )
  (func $test_i32x4_mul (param $a v128) (param $b v128) (result v128)
    (i32x4.mul (local.get $a) (local.get $b))
  )

  ;; ============================================================================
  ;; f32x4 arithmetic
  ;; ============================================================================
  (func $test_f32x4_add (param $a v128) (param $b v128) (result v128)
    (f32x4.add (local.get $a) (local.get $b))
  )
  (func $test_f32x4_sub (param $a v128) (param $b v128) (result v128)
    (f32x4.sub (local.get $a) (local.get $b))
  )
  (func $test_f32x4_mul (param $a v128) (param $b v128) (result v128)
    (f32x4.mul (local.get $a) (local.get $b))
  )
  (func $test_f32x4_div (param $a v128) (param $b v128) (result v128)
    (f32x4.div (local.get $a) (local.get $b))
  )
  (func $test_f32x4_sqrt (param $v v128) (result v128)
    (f32x4.sqrt (local.get $v))
  )
  (func $test_f32x4_neg (param $v v128) (result v128)
    (f32x4.neg (local.get $v))
  )
  (func $test_f32x4_abs (param $v v128) (result v128)
    (f32x4.abs (local.get $v))
  )

  ;; ============================================================================
  ;; v128 bitwise operations
  ;; ============================================================================
  (func $test_v128_and (param $a v128) (param $b v128) (result v128)
    (v128.and (local.get $a) (local.get $b))
  )
  (func $test_v128_or (param $a v128) (param $b v128) (result v128)
    (v128.or (local.get $a) (local.get $b))
  )
  (func $test_v128_xor (param $a v128) (param $b v128) (result v128)
    (v128.xor (local.get $a) (local.get $b))
  )
  (func $test_v128_not (param $v v128) (result v128)
    (v128.not (local.get $v))
  )
  (func $test_v128_andnot (param $a v128) (param $b v128) (result v128)
    (v128.andnot (local.get $a) (local.get $b))
  )
  (func $test_v128_bitselect (param $a v128) (param $b v128) (param $c v128) (result v128)
    (v128.bitselect (local.get $a) (local.get $b) (local.get $c))
  )
  (func $test_v128_any_true (param $v v128) (result i32)
    (v128.any_true (local.get $v))
  )

  ;; ============================================================================
  ;; Comparison operations
  ;; ============================================================================
  (func $test_i32x4_eq (param $a v128) (param $b v128) (result v128)
    (i32x4.eq (local.get $a) (local.get $b))
  )
  (func $test_i32x4_ne (param $a v128) (param $b v128) (result v128)
    (i32x4.ne (local.get $a) (local.get $b))
  )
  (func $test_i32x4_lt_s (param $a v128) (param $b v128) (result v128)
    (i32x4.lt_s (local.get $a) (local.get $b))
  )
  (func $test_f32x4_eq (param $a v128) (param $b v128) (result v128)
    (f32x4.eq (local.get $a) (local.get $b))
  )
  (func $test_f32x4_lt (param $a v128) (param $b v128) (result v128)
    (f32x4.lt (local.get $a) (local.get $b))
  )

  ;; ============================================================================
  ;; Shift operations
  ;; ============================================================================
  (func $test_i32x4_shl (param $v v128) (param $shift i32) (result v128)
    (i32x4.shl (local.get $v) (local.get $shift))
  )
  (func $test_i32x4_shr_s (param $v v128) (param $shift i32) (result v128)
    (i32x4.shr_s (local.get $v) (local.get $shift))
  )
  (func $test_i32x4_shr_u (param $v v128) (param $shift i32) (result v128)
    (i32x4.shr_u (local.get $v) (local.get $shift))
  )

  ;; ============================================================================
  ;; Bitmask operations
  ;; ============================================================================
  (func $test_i8x16_bitmask (param $v v128) (result i32)
    (i8x16.bitmask (local.get $v))
  )
  (func $test_i32x4_bitmask (param $v v128) (result i32)
    (i32x4.bitmask (local.get $v))
  )

  ;; ============================================================================
  ;; All true operations
  ;; ============================================================================
  (func $test_i8x16_all_true (param $v v128) (result i32)
    (i8x16.all_true (local.get $v))
  )
  (func $test_i32x4_all_true (param $v v128) (result i32)
    (i32x4.all_true (local.get $v))
  )

  ;; ============================================================================
  ;; Swizzle
  ;; ============================================================================
  (func $test_i8x16_swizzle (param $v v128) (param $idx v128) (result v128)
    (i8x16.swizzle (local.get $v) (local.get $idx))
  )

  ;; ============================================================================
  ;; Relaxed SIMD (proposal)
  ;; ============================================================================
  (func $test_i8x16_relaxed_swizzle (param $v v128) (param $idx v128) (result v128)
    (i8x16.relaxed_swizzle (local.get $v) (local.get $idx))
  )
  (func $test_f32x4_relaxed_min (param $a v128) (param $b v128) (result v128)
    (f32x4.relaxed_min (local.get $a) (local.get $b))
  )
  (func $test_f32x4_relaxed_max (param $a v128) (param $b v128) (result v128)
    (f32x4.relaxed_max (local.get $a) (local.get $b))
  )
)
