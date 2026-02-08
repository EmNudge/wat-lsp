;; Tables and Indirect Calls
;; Demonstrates function tables, indirect calls, and table operations

(module
  ;; Define function types for table
  (type $unary_op (func (param i32) (result i32)))
  (type $binary_op (func (param i32 i32) (result i32)))
  (type $void_fn (func))
  (type $predicate (func (param i32) (result i32)))

  ;; Declare tables
  (table $unary_ops 4 funcref)
  (table $binary_ops 4 funcref)
  (table $dynamic 10 10 funcref)

  ;; Element segments to initialize tables
  (elem $unary_init (table $unary_ops) (i32.const 0) func $double $square $negate $abs_val)
  (elem $binary_init (table $binary_ops) (i32.const 0) func $add $sub $mul $max_val)

  ;; Memory for storing results
  (memory 1)

  ;; Global operation counter
  (global $op_count (mut i32) (i32.const 0))

  ;; Unary operations
  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func $square (param $x i32) (result i32)
    (i32.mul (local.get $x) (local.get $x)))

  (func $negate (param $x i32) (result i32)
    (i32.sub (i32.const 0) (local.get $x)))

  (func $abs_val (param $x i32) (result i32)
    (if (result i32) (i32.lt_s (local.get $x) (i32.const 0))
      (then (i32.sub (i32.const 0) (local.get $x)))
      (else (local.get $x))))

  ;; Binary operations
  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  (func $sub (param $a i32) (param $b i32) (result i32)
    (i32.sub (local.get $a) (local.get $b)))

  (func $mul (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b)))

  (func $max_val (param $a i32) (param $b i32) (result i32)
    (if (result i32) (i32.gt_s (local.get $a) (local.get $b))
      (then (local.get $a))
      (else (local.get $b))))

  ;; Call unary operation by index
  (func $call_unary (param $op_idx i32) (param $x i32) (result i32)
    (global.set $op_count
      (i32.add (global.get $op_count) (i32.const 1)))
    (call_indirect $unary_ops (type $unary_op)
      (local.get $x)
      (local.get $op_idx)))

  ;; Call binary operation by index
  (func $call_binary (param $op_idx i32) (param $a i32) (param $b i32) (result i32)
    (global.set $op_count
      (i32.add (global.get $op_count) (i32.const 1)))
    (call_indirect $binary_ops (type $binary_op)
      (local.get $a)
      (local.get $b)
      (local.get $op_idx)))

  ;; Apply unary operation to array in memory
  (func $map_array (param $op_idx i32) (param $offset i32) (param $len i32)
    (local $i i32)
    (local $addr i32)
    (local $val i32)
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $addr
          (i32.add (local.get $offset)
            (i32.shl (local.get $i) (i32.const 2))))
        (local.set $val (i32.load (local.get $addr)))
        (local.set $val
          (call_indirect $unary_ops (type $unary_op)
            (local.get $val)
            (local.get $op_idx)))
        (i32.store (local.get $addr) (local.get $val))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))

  ;; Reduce array with binary operation
  (func $reduce_array (param $op_idx i32) (param $offset i32) (param $len i32) (param $init i32) (result i32)
    (local $i i32)
    (local $acc i32)
    (local $val i32)
    (local.set $acc (local.get $init))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $val
          (i32.load
            (i32.add (local.get $offset)
              (i32.shl (local.get $i) (i32.const 2)))))
        (local.set $acc
          (call_indirect $binary_ops (type $binary_op)
            (local.get $acc)
            (local.get $val)
            (local.get $op_idx)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $acc))

  ;; Get current table size
  (func $get_table_size (result i32)
    (table.size $dynamic))

  ;; Grow the dynamic table
  (func $grow_table (param $delta i32) (result i32)
    (table.grow $dynamic (ref.null func) (local.get $delta)))

  ;; Set function in dynamic table
  (func $set_table_entry (param $idx i32) (param $fn_idx i32)
    ;; This would need to use table.set with a ref from table.get
    ;; For simplicity, we demonstrate table.fill
    (nop))

  ;; Fill table entries with null
  (func $clear_table (param $start i32) (param $len i32)
    (table.fill $dynamic
      (local.get $start)
      (ref.null func)
      (local.get $len)))

  ;; Copy between tables
  (func $copy_ops (param $dst_start i32) (param $src_start i32) (param $len i32)
    (table.copy $dynamic $unary_ops
      (local.get $dst_start)
      (local.get $src_start)
      (local.get $len)))

  ;; Check if table entry is null
  (func $is_entry_null (param $idx i32) (result i32)
    (ref.is_null (table.get $dynamic (local.get $idx))))

  ;; Get operation count
  (func $get_op_count (result i32)
    (global.get $op_count))

  ;; Compose two unary operations
  (func $compose_unary (param $op1 i32) (param $op2 i32) (param $x i32) (result i32)
    (call_indirect $unary_ops (type $unary_op)
      (call_indirect $unary_ops (type $unary_op)
        (local.get $x)
        (local.get $op1))
      (local.get $op2)))

  ;; Exports
  (export "call_unary" (func $call_unary))
  (export "call_binary" (func $call_binary))
  (export "map_array" (func $map_array))
  (export "reduce_array" (func $reduce_array))
  (export "get_table_size" (func $get_table_size))
  (export "grow_table" (func $grow_table))
  (export "get_op_count" (func $get_op_count))
  (export "compose_unary" (func $compose_unary))
  (export "memory" (memory 0)))
