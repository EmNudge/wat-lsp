;; SIMD in Linear (Unfolded) Style
;; Demonstrates v128 operations using stack-based programming,
;; as opposed to the folded/S-expression style in simd.wat.

(module
  (memory 1)

  ;; Add two i32x4 vectors
  (func $add_i32x4 (export "add_i32x4")
        (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    i32x4.add)

  ;; Negate an f32x4 vector (unary op)
  (func $neg_f32x4 (export "neg_f32x4") (param $a v128) (result v128)
    local.get $a
    f32x4.neg)

  ;; Broadcast a scalar i32 to all four lanes
  (func $splat_i32 (export "splat_i32") (param $val i32) (result v128)
    local.get $val
    i32x4.splat)

  ;; Bitwise AND of two v128 values
  (func $bitwise_and (export "bitwise_and")
        (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    v128.and)

  ;; Bitwise NOT of a v128 value
  (func $bitwise_not (export "bitwise_not") (param $a v128) (result v128)
    local.get $a
    v128.not)

  ;; Bitselect: choose bits from $a or $b based on mask $m
  ;; For each bit: if mask bit is 1 pick $a, else pick $b
  (func $bitselect (export "bitselect")
        (param $a v128) (param $b v128) (param $m v128) (result v128)
    local.get $a
    local.get $b
    local.get $m
    v128.bitselect)

  ;; Check if any lane is non-zero
  (func $any_nonzero (export "any_nonzero") (param $v v128) (result i32)
    local.get $v
    v128.any_true)

  ;; Check if all i32 lanes are non-zero
  (func $all_nonzero_i32 (export "all_nonzero_i32") (param $v v128) (result i32)
    local.get $v
    i32x4.all_true)

  ;; Extract the sign bits of each i32 lane as a bitmask
  (func $bitmask_i32x4 (export "bitmask_i32x4") (param $v v128) (result i32)
    local.get $v
    i32x4.bitmask)

  ;; Shift each i32 lane left by a scalar amount
  (func $shl_i32x4 (export "shl_i32x4")
        (param $v v128) (param $shift i32) (result v128)
    local.get $v
    local.get $shift
    i32x4.shl)

  ;; Load a v128 from memory and store a modified copy back
  (func $load_add_store (export "load_add_store")
        (param $offset i32) (param $addend v128)
    local.get $offset
    v128.load          ;; load 16 bytes from memory
    local.get $addend
    i32x4.add          ;; add element-wise
    local.set $addend  ;; reuse param as temp

    local.get $offset
    local.get $addend
    v128.store)        ;; write back

  ;; Convert i32x4 to f32x4 (signed)
  (func $convert_to_float (export "convert_to_float")
        (param $v v128) (result v128)
    local.get $v
    f32x4.convert_i32x4_s)

  ;; Absolute value of f32x4
  (func $abs_f32x4 (export "abs_f32x4") (param $v v128) (result v128)
    local.get $v
    f32x4.abs)

  ;; Element-wise i32x4 comparison, returning a mask
  (func $compare_eq (export "compare_eq")
        (param $a v128) (param $b v128) (result v128)
    local.get $a
    local.get $b
    i32x4.eq)

  ;; Horizontal sum of an i32x4 vector (extract each lane, add scalars)
  (func $horizontal_sum (export "horizontal_sum") (param $v v128) (result i32)
    (local $a i32)
    (local $b i32)

    local.get $v
    i32x4.extract_lane 0
    local.set $a

    local.get $v
    i32x4.extract_lane 1
    local.get $a
    i32.add
    local.set $a

    local.get $v
    i32x4.extract_lane 2
    local.get $a
    i32.add
    local.set $a

    local.get $v
    i32x4.extract_lane 3
    local.get $a
    i32.add)
)
