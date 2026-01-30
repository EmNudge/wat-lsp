;; Table64 - 64-bit Table Indexing
;; Demonstrates tables with 64-bit address types for large table support

(module
  ;; Define function types
  (type $callback (func (param i64) (result i64)))
  (type $void (func))

  ;; Regular 32-bit indexed table (default)
  (table $small_table 10 funcref)

  ;; 64-bit indexed table - can have more than 2^32 entries
  (table $large_table i64 100 1000 funcref)

  ;; Another 64-bit table with externref
  (table $extern_table i64 50 externref)

  ;; Initialize 32-bit table with element segment
  (elem (table $small_table) (i32.const 0) func $identity)

  ;; Functions for the table
  (func $identity (param $x i64) (result i64)
    (local.get $x))

  (func $double (param $x i64) (result i64)
    (i64.mul (local.get $x) (i64.const 2)))

  ;; Get table size - returns i64 for table64
  (func $get_large_table_size (export "get_large_table_size") (result i64)
    (table.size $large_table))

  ;; Get small table size - returns i32 for regular table
  (func $get_small_table_size (export "get_small_table_size") (result i32)
    (table.size $small_table))

  ;; Grow 64-bit table - delta is i64, returns i64
  (func $grow_large_table (export "grow_large_table") (param $delta i64) (result i64)
    (table.grow $large_table (ref.null func) (local.get $delta)))

  ;; Get from 64-bit table - index is i64
  (func $get_from_large (export "get_from_large") (param $idx i64) (result funcref)
    (table.get $large_table (local.get $idx)))

  ;; Set in 64-bit table - index is i64
  (func $set_in_large (export "set_in_large") (param $idx i64) (param $val funcref)
    (table.set $large_table (local.get $idx) (local.get $val)))

  ;; Fill 64-bit table - index and count are i64
  (func $fill_large (export "fill_large") (param $start i64) (param $count i64)
    (table.fill $large_table
      (local.get $start)
      (ref.null func)
      (local.get $count)))

  ;; Copy within 64-bit table - all indices are i64
  (func $copy_within_large (export "copy_within_large")
    (param $dst i64) (param $src i64) (param $count i64)
    (table.copy $large_table $large_table
      (local.get $dst)
      (local.get $src)
      (local.get $count)))

  ;; Call indirect with 64-bit index
  (func $call_by_index64 (export "call_by_index64") (param $idx i64) (param $arg i64) (result i64)
    (call_indirect $large_table (type $callback)
      (local.get $arg)
      (local.get $idx)))

  ;; Mixed usage: copy from 32-bit table to 64-bit table
  ;; Note: source index is i32, dest index is i64
  (func $copy_small_to_large (export "copy_small_to_large")
    (param $dst i64) (param $src i32) (param $count i32)
    (table.copy $large_table $small_table
      (local.get $dst)
      (local.get $src)
      (local.get $count)))
)
