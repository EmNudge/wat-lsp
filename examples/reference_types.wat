;; Reference Types Proposal
;; Demonstrates externref, funcref, and reference operations

(module
  ;; Table of external references (JavaScript objects, etc.)
  (table $extern_table 10 externref)

  ;; Table of function references
  (table $func_table 5 funcref)

  ;; Global holding an external reference
  (global $current_extern (mut externref) (ref.null extern))

  ;; Global holding a function reference
  (global $callback (mut funcref) (ref.null func))

  ;; Type definitions
  (type $i32_to_i32 (func (param i32) (result i32)))
  (type $void_fn (func))
  (type $binary_fn (func (param i32 i32) (result i32)))

  ;; Store an external reference in the table
  (func $store_extern (param $idx i32) (param $ref externref)
    (table.set $extern_table (local.get $idx) (local.get $ref)))

  ;; Load an external reference from the table
  (func $load_extern (param $idx i32) (result externref)
    (table.get $extern_table (local.get $idx)))

  ;; Check if table slot is null
  (func $is_null (param $idx i32) (result i32)
    (ref.is_null (table.get $extern_table (local.get $idx))))

  ;; Set the global extern ref
  (func $set_current (param $ref externref)
    (global.set $current_extern (local.get $ref)))

  ;; Get the global extern ref
  (func $get_current (result externref)
    (global.get $current_extern))

  ;; Clear (set to null) a table slot
  (func $clear_slot (param $idx i32)
    (table.set $extern_table (local.get $idx) (ref.null extern)))

  ;; Copy reference between slots
  (func $copy_ref (param $from i32) (param $to i32)
    (table.set $extern_table
      (local.get $to)
      (table.get $extern_table (local.get $from))))

  ;; Get table size
  (func $table_size (result i32)
    (table.size $extern_table))

  ;; Grow table
  (func $grow_table (param $delta i32) (result i32)
    (table.grow $extern_table (ref.null extern) (local.get $delta)))

  ;; Fill table with null
  (func $clear_range (param $start i32) (param $len i32)
    (table.fill $extern_table
      (local.get $start)
      (ref.null extern)
      (local.get $len)))

  ;; === Function References ===

  ;; Sample functions to store in table
  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func $triple (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 3)))

  (func $square (param $x i32) (result i32)
    (i32.mul (local.get $x) (local.get $x)))

  (func $noop)

  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  ;; Initialize function table
  (elem $fn_init (table $func_table) (i32.const 0)
    func $double $triple $square)

  ;; Call function from table indirectly
  (func $call_from_table (param $idx i32) (param $arg i32) (result i32)
    (call_indirect $func_table (type $i32_to_i32)
      (local.get $arg)
      (local.get $idx)))

  ;; Set callback function
  (func $set_callback (param $fn funcref)
    (global.set $callback (local.get $fn)))

  ;; Check if callback is set
  (func $has_callback (result i32)
    (i32.eqz (ref.is_null (global.get $callback))))

  ;; Call the callback if set, return default otherwise
  (func $invoke_callback (param $arg i32) (param $default i32) (result i32)
    (if (result i32) (ref.is_null (global.get $callback))
      (then (local.get $default))
      (else
        (call_indirect $func_table (type $i32_to_i32)
          (local.get $arg)
          ;; Need to store callback in table first for call_indirect
          ;; For simplicity, just return arg * 2 as placeholder
          (i32.const 0)))))

  ;; Return a function reference
  (func $get_double_fn (result funcref)
    (ref.func $double))

  ;; Return a function reference for triple
  (func $get_triple_fn (result funcref)
    (ref.func $triple))

  ;; Store function ref in table at given index
  (func $store_fn (param $idx i32) (param $fn funcref)
    (table.set $func_table (local.get $idx) (local.get $fn)))

  ;; Check if function slot is null
  (func $is_fn_null (param $idx i32) (result i32)
    (ref.is_null (table.get $func_table (local.get $idx))))

  ;; Apply function to value N times
  (func $apply_n_times (param $fn_idx i32) (param $x i32) (param $n i32) (result i32)
    (local $i i32)
    (local $result i32)
    (local.set $result (local.get $x))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (local.set $result
          (call_indirect $func_table (type $i32_to_i32)
            (local.get $result)
            (local.get $fn_idx)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $result))

  ;; Count non-null entries in extern table
  (func $count_entries (result i32)
    (local $i i32)
    (local $count i32)
    (local $size i32)
    (local.set $size (table.size $extern_table))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $size)))
        (if (i32.eqz (ref.is_null (table.get $extern_table (local.get $i))))
          (then
            (local.set $count (i32.add (local.get $count) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $count))

  ;; Select between two functions based on condition
  (func $select_fn (param $cond i32) (result funcref)
    (if (result funcref) (local.get $cond)
      (then (ref.func $double))
      (else (ref.func $triple))))

  ;; Exports
  (export "store_extern" (func $store_extern))
  (export "load_extern" (func $load_extern))
  (export "is_null" (func $is_null))
  (export "set_current" (func $set_current))
  (export "get_current" (func $get_current))
  (export "table_size" (func $table_size))
  (export "grow_table" (func $grow_table))
  (export "call_from_table" (func $call_from_table))
  (export "get_double_fn" (func $get_double_fn))
  (export "apply_n_times" (func $apply_n_times))
  (export "count_entries" (func $count_entries))
  (export "extern_table" (table $extern_table))
  (export "func_table" (table $func_table)))
