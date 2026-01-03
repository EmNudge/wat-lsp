;; SIMD (Single Instruction Multiple Data) Proposal
;; Demonstrates v128 operations for parallel computation

(module
  (memory 1)

  ;; Add two v128 vectors of i32x4
  (func $add_i32x4 (param $a v128) (param $b v128) (result v128)
    (i32x4.add (local.get $a) (local.get $b)))

  ;; Multiply two v128 vectors of f32x4
  (func $mul_f32x4 (param $a v128) (param $b v128) (result v128)
    (f32x4.mul (local.get $a) (local.get $b)))

  ;; Dot product of two f32x4 vectors
  (func $dot_f32x4 (param $a v128) (param $b v128) (result f32)
    (local $product v128)
    (local.set $product (f32x4.mul (local.get $a) (local.get $b)))
    ;; Sum all lanes
    (f32.add
      (f32.add
        (f32x4.extract_lane 0 (local.get $product))
        (f32x4.extract_lane 1 (local.get $product)))
      (f32.add
        (f32x4.extract_lane 2 (local.get $product))
        (f32x4.extract_lane 3 (local.get $product)))))

  ;; Create a v128 with all lanes set to same i32 value (splat)
  (func $splat_i32 (param $val i32) (result v128)
    (i32x4.splat (local.get $val)))

  ;; Create a v128 with all lanes set to same f32 value
  (func $splat_f32 (param $val f32) (result v128)
    (f32x4.splat (local.get $val)))

  ;; Load v128 from memory
  (func $load_v128 (param $offset i32) (result v128)
    (v128.load (local.get $offset)))

  ;; Store v128 to memory
  (func $store_v128 (param $offset i32) (param $val v128)
    (v128.store (local.get $offset) (local.get $val)))

  ;; Element-wise minimum of two f32x4 vectors
  (func $min_f32x4 (param $a v128) (param $b v128) (result v128)
    (f32x4.min (local.get $a) (local.get $b)))

  ;; Element-wise maximum of two f32x4 vectors
  (func $max_f32x4 (param $a v128) (param $b v128) (result v128)
    (f32x4.max (local.get $a) (local.get $b)))

  ;; Absolute value of f32x4
  (func $abs_f32x4 (param $a v128) (result v128)
    (f32x4.abs (local.get $a)))

  ;; Negate f32x4
  (func $neg_f32x4 (param $a v128) (result v128)
    (f32x4.neg (local.get $a)))

  ;; Square root of f32x4
  (func $sqrt_f32x4 (param $a v128) (result v128)
    (f32x4.sqrt (local.get $a)))

  ;; Bitwise AND of two v128
  (func $and_v128 (param $a v128) (param $b v128) (result v128)
    (v128.and (local.get $a) (local.get $b)))

  ;; Bitwise OR of two v128
  (func $or_v128 (param $a v128) (param $b v128) (result v128)
    (v128.or (local.get $a) (local.get $b)))

  ;; Bitwise XOR of two v128
  (func $xor_v128 (param $a v128) (param $b v128) (result v128)
    (v128.xor (local.get $a) (local.get $b)))

  ;; Bitwise NOT of v128
  (func $not_v128 (param $a v128) (result v128)
    (v128.not (local.get $a)))

  ;; Compare i32x4 for equality (returns mask)
  (func $eq_i32x4 (param $a v128) (param $b v128) (result v128)
    (i32x4.eq (local.get $a) (local.get $b)))

  ;; Compare i32x4 less than signed
  (func $lt_i32x4 (param $a v128) (param $b v128) (result v128)
    (i32x4.lt_s (local.get $a) (local.get $b)))

  ;; Shift left i32x4
  (func $shl_i32x4 (param $a v128) (param $shift i32) (result v128)
    (i32x4.shl (local.get $a) (local.get $shift)))

  ;; Shift right signed i32x4
  (func $shr_s_i32x4 (param $a v128) (param $shift i32) (result v128)
    (i32x4.shr_s (local.get $a) (local.get $shift)))

  ;; Extract lane from i32x4
  (func $extract_i32 (param $v v128) (param $lane i32) (result i32)
    ;; Note: lane must be immediate, so we use a switch
    (if (result i32) (i32.eq (local.get $lane) (i32.const 0))
      (then (i32x4.extract_lane 0 (local.get $v)))
      (else
        (if (result i32) (i32.eq (local.get $lane) (i32.const 1))
          (then (i32x4.extract_lane 1 (local.get $v)))
          (else
            (if (result i32) (i32.eq (local.get $lane) (i32.const 2))
              (then (i32x4.extract_lane 2 (local.get $v)))
              (else (i32x4.extract_lane 3 (local.get $v)))))))))

  ;; Replace lane in i32x4
  (func $replace_lane_0 (param $v v128) (param $val i32) (result v128)
    (i32x4.replace_lane 0 (local.get $v) (local.get $val)))

  ;; Shuffle bytes (swizzle)
  (func $reverse_i32x4 (param $v v128) (result v128)
    ;; Reverse the order of i32 lanes: 3,2,1,0
    (i8x16.shuffle 12 13 14 15 8 9 10 11 4 5 6 7 0 1 2 3
      (local.get $v) (local.get $v)))

  ;; Sum all i32 lanes
  (func $horizontal_sum_i32x4 (param $v v128) (result i32)
    (i32.add
      (i32.add
        (i32x4.extract_lane 0 (local.get $v))
        (i32x4.extract_lane 1 (local.get $v)))
      (i32.add
        (i32x4.extract_lane 2 (local.get $v))
        (i32x4.extract_lane 3 (local.get $v)))))

  ;; Process array of f32 with SIMD (add constant to each)
  (func $add_constant_to_array (param $offset i32) (param $len i32) (param $constant f32)
    (local $i i32)
    (local $vec v128)
    (local $const_vec v128)
    (local.set $const_vec (f32x4.splat (local.get $constant)))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        ;; Load 4 floats
        (local.set $vec
          (v128.load
            (i32.add (local.get $offset)
              (i32.shl (local.get $i) (i32.const 2)))))
        ;; Add constant
        (local.set $vec (f32x4.add (local.get $vec) (local.get $const_vec)))
        ;; Store back
        (v128.store
          (i32.add (local.get $offset)
            (i32.shl (local.get $i) (i32.const 2)))
          (local.get $vec))
        ;; Increment by 4 (process 4 floats at a time)
        (local.set $i (i32.add (local.get $i) (i32.const 4)))
        (br $loop))))

  ;; Convert i32x4 to f32x4
  (func $convert_i32x4_to_f32x4 (param $v v128) (result v128)
    (f32x4.convert_i32x4_s (local.get $v)))

  ;; Truncate f32x4 to i32x4 (saturating)
  (func $trunc_f32x4_to_i32x4 (param $v v128) (result v128)
    (i32x4.trunc_sat_f32x4_s (local.get $v)))

  ;; Check if any lane is true (non-zero)
  (func $any_true_i32x4 (param $v v128) (result i32)
    (v128.any_true (local.get $v)))

  ;; Check if all lanes are true
  (func $all_true_i32x4 (param $v v128) (result i32)
    (i32x4.all_true (local.get $v)))

  ;; Bitmask - extract sign bits from each lane
  (func $bitmask_i32x4 (param $v v128) (result i32)
    (i32x4.bitmask (local.get $v)))

  ;; Fused multiply-add for f32x4: a * b + c
  (func $fma_f32x4 (param $a v128) (param $b v128) (param $c v128) (result v128)
    (f32x4.add
      (f32x4.mul (local.get $a) (local.get $b))
      (local.get $c)))

  ;; i64x2 operations
  (func $add_i64x2 (param $a v128) (param $b v128) (result v128)
    (i64x2.add (local.get $a) (local.get $b)))

  ;; f64x2 operations
  (func $mul_f64x2 (param $a v128) (param $b v128) (result v128)
    (f64x2.mul (local.get $a) (local.get $b)))

  ;; Exports
  (export "add_i32x4" (func $add_i32x4))
  (export "mul_f32x4" (func $mul_f32x4))
  (export "dot_f32x4" (func $dot_f32x4))
  (export "splat_i32" (func $splat_i32))
  (export "horizontal_sum_i32x4" (func $horizontal_sum_i32x4))
  (export "add_constant_to_array" (func $add_constant_to_array))
  (export "memory" (memory 0)))
