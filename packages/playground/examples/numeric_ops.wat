;; Comprehensive Numeric Operations
;; Demonstrates all numeric types and operations

(module
  (memory 1)

  ;; === i32 Operations ===

  ;; Arithmetic
  (func $i32_add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  (func $i32_sub (param $a i32) (param $b i32) (result i32)
    (i32.sub (local.get $a) (local.get $b)))

  (func $i32_mul (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b)))

  (func $i32_div_s (param $a i32) (param $b i32) (result i32)
    (i32.div_s (local.get $a) (local.get $b)))

  (func $i32_div_u (param $a i32) (param $b i32) (result i32)
    (i32.div_u (local.get $a) (local.get $b)))

  (func $i32_rem_s (param $a i32) (param $b i32) (result i32)
    (i32.rem_s (local.get $a) (local.get $b)))

  (func $i32_rem_u (param $a i32) (param $b i32) (result i32)
    (i32.rem_u (local.get $a) (local.get $b)))

  ;; Bitwise
  (func $i32_and (param $a i32) (param $b i32) (result i32)
    (i32.and (local.get $a) (local.get $b)))

  (func $i32_or (param $a i32) (param $b i32) (result i32)
    (i32.or (local.get $a) (local.get $b)))

  (func $i32_xor (param $a i32) (param $b i32) (result i32)
    (i32.xor (local.get $a) (local.get $b)))

  (func $i32_shl (param $a i32) (param $b i32) (result i32)
    (i32.shl (local.get $a) (local.get $b)))

  (func $i32_shr_s (param $a i32) (param $b i32) (result i32)
    (i32.shr_s (local.get $a) (local.get $b)))

  (func $i32_shr_u (param $a i32) (param $b i32) (result i32)
    (i32.shr_u (local.get $a) (local.get $b)))

  (func $i32_rotl (param $a i32) (param $b i32) (result i32)
    (i32.rotl (local.get $a) (local.get $b)))

  (func $i32_rotr (param $a i32) (param $b i32) (result i32)
    (i32.rotr (local.get $a) (local.get $b)))

  ;; Unary
  (func $i32_clz (param $a i32) (result i32)
    (i32.clz (local.get $a)))

  (func $i32_ctz (param $a i32) (result i32)
    (i32.ctz (local.get $a)))

  (func $i32_popcnt (param $a i32) (result i32)
    (i32.popcnt (local.get $a)))

  (func $i32_eqz (param $a i32) (result i32)
    (i32.eqz (local.get $a)))

  ;; Comparison
  (func $i32_eq (param $a i32) (param $b i32) (result i32)
    (i32.eq (local.get $a) (local.get $b)))

  (func $i32_ne (param $a i32) (param $b i32) (result i32)
    (i32.ne (local.get $a) (local.get $b)))

  (func $i32_lt_s (param $a i32) (param $b i32) (result i32)
    (i32.lt_s (local.get $a) (local.get $b)))

  (func $i32_lt_u (param $a i32) (param $b i32) (result i32)
    (i32.lt_u (local.get $a) (local.get $b)))

  (func $i32_gt_s (param $a i32) (param $b i32) (result i32)
    (i32.gt_s (local.get $a) (local.get $b)))

  (func $i32_gt_u (param $a i32) (param $b i32) (result i32)
    (i32.gt_u (local.get $a) (local.get $b)))

  (func $i32_le_s (param $a i32) (param $b i32) (result i32)
    (i32.le_s (local.get $a) (local.get $b)))

  (func $i32_le_u (param $a i32) (param $b i32) (result i32)
    (i32.le_u (local.get $a) (local.get $b)))

  (func $i32_ge_s (param $a i32) (param $b i32) (result i32)
    (i32.ge_s (local.get $a) (local.get $b)))

  (func $i32_ge_u (param $a i32) (param $b i32) (result i32)
    (i32.ge_u (local.get $a) (local.get $b)))

  ;; Extend operations
  (func $i32_extend8_s (param $a i32) (result i32)
    (i32.extend8_s (local.get $a)))

  (func $i32_extend16_s (param $a i32) (result i32)
    (i32.extend16_s (local.get $a)))

  ;; === i64 Operations ===

  (func $i64_add (param $a i64) (param $b i64) (result i64)
    (i64.add (local.get $a) (local.get $b)))

  (func $i64_sub (param $a i64) (param $b i64) (result i64)
    (i64.sub (local.get $a) (local.get $b)))

  (func $i64_mul (param $a i64) (param $b i64) (result i64)
    (i64.mul (local.get $a) (local.get $b)))

  (func $i64_div_s (param $a i64) (param $b i64) (result i64)
    (i64.div_s (local.get $a) (local.get $b)))

  (func $i64_clz (param $a i64) (result i64)
    (i64.clz (local.get $a)))

  (func $i64_eqz (param $a i64) (result i32)
    (i64.eqz (local.get $a)))

  (func $i64_extend_i32_s (param $a i32) (result i64)
    (i64.extend_i32_s (local.get $a)))

  (func $i64_extend_i32_u (param $a i32) (result i64)
    (i64.extend_i32_u (local.get $a)))

  (func $i32_wrap_i64 (param $a i64) (result i32)
    (i32.wrap_i64 (local.get $a)))

  ;; === f32 Operations ===

  (func $f32_add (param $a f32) (param $b f32) (result f32)
    (f32.add (local.get $a) (local.get $b)))

  (func $f32_sub (param $a f32) (param $b f32) (result f32)
    (f32.sub (local.get $a) (local.get $b)))

  (func $f32_mul (param $a f32) (param $b f32) (result f32)
    (f32.mul (local.get $a) (local.get $b)))

  (func $f32_div (param $a f32) (param $b f32) (result f32)
    (f32.div (local.get $a) (local.get $b)))

  (func $f32_abs (param $a f32) (result f32)
    (f32.abs (local.get $a)))

  (func $f32_neg (param $a f32) (result f32)
    (f32.neg (local.get $a)))

  (func $f32_ceil (param $a f32) (result f32)
    (f32.ceil (local.get $a)))

  (func $f32_floor (param $a f32) (result f32)
    (f32.floor (local.get $a)))

  (func $f32_trunc (param $a f32) (result f32)
    (f32.trunc (local.get $a)))

  (func $f32_nearest (param $a f32) (result f32)
    (f32.nearest (local.get $a)))

  (func $f32_sqrt (param $a f32) (result f32)
    (f32.sqrt (local.get $a)))

  (func $f32_min (param $a f32) (param $b f32) (result f32)
    (f32.min (local.get $a) (local.get $b)))

  (func $f32_max (param $a f32) (param $b f32) (result f32)
    (f32.max (local.get $a) (local.get $b)))

  (func $f32_copysign (param $a f32) (param $b f32) (result f32)
    (f32.copysign (local.get $a) (local.get $b)))

  ;; Comparison
  (func $f32_eq (param $a f32) (param $b f32) (result i32)
    (f32.eq (local.get $a) (local.get $b)))

  (func $f32_ne (param $a f32) (param $b f32) (result i32)
    (f32.ne (local.get $a) (local.get $b)))

  (func $f32_lt (param $a f32) (param $b f32) (result i32)
    (f32.lt (local.get $a) (local.get $b)))

  (func $f32_gt (param $a f32) (param $b f32) (result i32)
    (f32.gt (local.get $a) (local.get $b)))

  (func $f32_le (param $a f32) (param $b f32) (result i32)
    (f32.le (local.get $a) (local.get $b)))

  (func $f32_ge (param $a f32) (param $b f32) (result i32)
    (f32.ge (local.get $a) (local.get $b)))

  ;; === f64 Operations ===

  (func $f64_add (param $a f64) (param $b f64) (result f64)
    (f64.add (local.get $a) (local.get $b)))

  (func $f64_sub (param $a f64) (param $b f64) (result f64)
    (f64.sub (local.get $a) (local.get $b)))

  (func $f64_mul (param $a f64) (param $b f64) (result f64)
    (f64.mul (local.get $a) (local.get $b)))

  (func $f64_div (param $a f64) (param $b f64) (result f64)
    (f64.div (local.get $a) (local.get $b)))

  (func $f64_abs (param $a f64) (result f64)
    (f64.abs (local.get $a)))

  (func $f64_neg (param $a f64) (result f64)
    (f64.neg (local.get $a)))

  (func $f64_sqrt (param $a f64) (result f64)
    (f64.sqrt (local.get $a)))

  (func $f64_ceil (param $a f64) (result f64)
    (f64.ceil (local.get $a)))

  (func $f64_floor (param $a f64) (result f64)
    (f64.floor (local.get $a)))

  ;; === Conversions ===

  (func $i32_trunc_f32_s (param $a f32) (result i32)
    (i32.trunc_f32_s (local.get $a)))

  (func $i32_trunc_f32_u (param $a f32) (result i32)
    (i32.trunc_f32_u (local.get $a)))

  (func $i32_trunc_f64_s (param $a f64) (result i32)
    (i32.trunc_f64_s (local.get $a)))

  (func $i32_trunc_f64_u (param $a f64) (result i32)
    (i32.trunc_f64_u (local.get $a)))

  (func $i64_trunc_f32_s (param $a f32) (result i64)
    (i64.trunc_f32_s (local.get $a)))

  (func $i64_trunc_f64_s (param $a f64) (result i64)
    (i64.trunc_f64_s (local.get $a)))

  ;; Saturating truncation (non-trapping)
  (func $i32_trunc_sat_f32_s (param $a f32) (result i32)
    (i32.trunc_sat_f32_s (local.get $a)))

  (func $i32_trunc_sat_f32_u (param $a f32) (result i32)
    (i32.trunc_sat_f32_u (local.get $a)))

  (func $i32_trunc_sat_f64_s (param $a f64) (result i32)
    (i32.trunc_sat_f64_s (local.get $a)))

  (func $i64_trunc_sat_f64_s (param $a f64) (result i64)
    (i64.trunc_sat_f64_s (local.get $a)))

  ;; Float conversions
  (func $f32_convert_i32_s (param $a i32) (result f32)
    (f32.convert_i32_s (local.get $a)))

  (func $f32_convert_i32_u (param $a i32) (result f32)
    (f32.convert_i32_u (local.get $a)))

  (func $f64_convert_i32_s (param $a i32) (result f64)
    (f64.convert_i32_s (local.get $a)))

  (func $f64_convert_i64_s (param $a i64) (result f64)
    (f64.convert_i64_s (local.get $a)))

  (func $f32_demote_f64 (param $a f64) (result f32)
    (f32.demote_f64 (local.get $a)))

  (func $f64_promote_f32 (param $a f32) (result f64)
    (f64.promote_f32 (local.get $a)))

  ;; Reinterpret
  (func $i32_reinterpret_f32 (param $a f32) (result i32)
    (i32.reinterpret_f32 (local.get $a)))

  (func $f32_reinterpret_i32 (param $a i32) (result f32)
    (f32.reinterpret_i32 (local.get $a)))

  (func $i64_reinterpret_f64 (param $a f64) (result i64)
    (i64.reinterpret_f64 (local.get $a)))

  (func $f64_reinterpret_i64 (param $a i64) (result f64)
    (f64.reinterpret_i64 (local.get $a)))

  ;; === Memory Load/Store Operations ===

  ;; i32 loads
  (func $i32_load (param $addr i32) (result i32)
    (i32.load (local.get $addr)))

  (func $i32_load8_s (param $addr i32) (result i32)
    (i32.load8_s (local.get $addr)))

  (func $i32_load8_u (param $addr i32) (result i32)
    (i32.load8_u (local.get $addr)))

  (func $i32_load16_s (param $addr i32) (result i32)
    (i32.load16_s (local.get $addr)))

  (func $i32_load16_u (param $addr i32) (result i32)
    (i32.load16_u (local.get $addr)))

  ;; i32 stores
  (func $i32_store (param $addr i32) (param $val i32)
    (i32.store (local.get $addr) (local.get $val)))

  (func $i32_store8 (param $addr i32) (param $val i32)
    (i32.store8 (local.get $addr) (local.get $val)))

  (func $i32_store16 (param $addr i32) (param $val i32)
    (i32.store16 (local.get $addr) (local.get $val)))

  ;; i64 loads
  (func $i64_load (param $addr i32) (result i64)
    (i64.load (local.get $addr)))

  (func $i64_load32_s (param $addr i32) (result i64)
    (i64.load32_s (local.get $addr)))

  (func $i64_load32_u (param $addr i32) (result i64)
    (i64.load32_u (local.get $addr)))

  ;; Float loads/stores
  (func $f32_load (param $addr i32) (result f32)
    (f32.load (local.get $addr)))

  (func $f32_store (param $addr i32) (param $val f32)
    (f32.store (local.get $addr) (local.get $val)))

  (func $f64_load (param $addr i32) (result f64)
    (f64.load (local.get $addr)))

  (func $f64_store (param $addr i32) (param $val f64)
    (f64.store (local.get $addr) (local.get $val)))

  ;; Load with offset and alignment
  (func $aligned_load (param $base i32) (result i32)
    (i32.load offset=16 align=4 (local.get $base)))

  ;; === Practical Examples ===

  ;; Calculate hypotenuse
  (func $hypot (param $a f64) (param $b f64) (result f64)
    (f64.sqrt
      (f64.add
        (f64.mul (local.get $a) (local.get $a))
        (f64.mul (local.get $b) (local.get $b)))))

  ;; Lerp (linear interpolation)
  (func $lerp (param $a f64) (param $b f64) (param $t f64) (result f64)
    (f64.add
      (local.get $a)
      (f64.mul
        (f64.sub (local.get $b) (local.get $a))
        (local.get $t))))

  ;; Clamp f64
  (func $clamp_f64 (param $val f64) (param $min f64) (param $max f64) (result f64)
    (f64.min
      (local.get $max)
      (f64.max (local.get $min) (local.get $val))))

  ;; Bit manipulation: count set bits in range
  (func $count_bits_in_mask (param $val i32) (param $mask i32) (result i32)
    (i32.popcnt (i32.and (local.get $val) (local.get $mask))))

  ;; Exports
  (export "i32_add" (func $i32_add))
  (export "i32_clz" (func $i32_clz))
  (export "i32_popcnt" (func $i32_popcnt))
  (export "f64_sqrt" (func $f64_sqrt))
  (export "hypot" (func $hypot))
  (export "lerp" (func $lerp))
  (export "clamp_f64" (func $clamp_f64))
  (export "memory" (memory 0)))
