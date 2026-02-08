;; Multi-Value Returns and Bulk Memory Operations
;; Demonstrates functions returning multiple values and memory operations

(module
  (memory $mem 2)

  ;; Data segment for initialization
  (data $hello (i32.const 0) "Hello, WebAssembly!")
  (data $numbers (i32.const 100) "\01\00\00\00\02\00\00\00\03\00\00\00\04\00\00\00")

  ;; Return multiple values: quotient and remainder
  (func $divmod (param $a i32) (param $b i32) (result i32 i32)
    (i32.div_s (local.get $a) (local.get $b))
    (i32.rem_s (local.get $a) (local.get $b)))

  ;; Return min and max of two numbers
  (func $minmax (param $a i32) (param $b i32) (result i32 i32)
    (if (result i32 i32) (i32.lt_s (local.get $a) (local.get $b))
      (then
        (local.get $a)
        (local.get $b))
      (else
        (local.get $b)
        (local.get $a))))

  ;; Swap two values
  (func $swap (param $a i32) (param $b i32) (result i32 i32)
    (local.get $b)
    (local.get $a))

  ;; Return three values: sum, product, difference
  (func $arithmetic_triple (param $a i32) (param $b i32) (result i32 i32 i32)
    (i32.add (local.get $a) (local.get $b))
    (i32.mul (local.get $a) (local.get $b))
    (i32.sub (local.get $a) (local.get $b)))

  ;; Find bounds of array in memory
  (func $array_bounds (param $offset i32) (param $len i32) (result i32 i32)
    (local $i i32)
    (local $min i32)
    (local $max i32)
    (local $val i32)
    ;; Initialize min/max with first element
    (local.set $min (i32.load (local.get $offset)))
    (local.set $max (local.get $min))
    (local.set $i (i32.const 1))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $val
          (i32.load
            (i32.add (local.get $offset)
              (i32.shl (local.get $i) (i32.const 2)))))
        (if (i32.lt_s (local.get $val) (local.get $min))
          (then (local.set $min (local.get $val))))
        (if (i32.gt_s (local.get $val) (local.get $max))
          (then (local.set $max (local.get $val))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $min)
    (local.get $max))

  ;; Consume multi-value return
  (func $use_divmod (param $a i32) (param $b i32) (result i32)
    (local $quot i32)
    (local $rem i32)
    (call $divmod (local.get $a) (local.get $b))
    (local.set $rem)
    (local.set $quot)
    ;; Return quot * 10 + rem
    (i32.add
      (i32.mul (local.get $quot) (i32.const 10))
      (local.get $rem)))

  ;; Block with multi-value
  (func $block_multi (param $x i32) (result i32 i32)
    (block (result i32 i32)
      (local.get $x)
      (i32.mul (local.get $x) (i32.const 2))))

  ;; Loop with multi-value accumulator
  (func $fib_pair (param $n i32) (result i32 i32)
    (local $a i32)
    (local $b i32)
    (local $i i32)
    (local $temp i32)
    (local.set $a (i32.const 0))
    (local.set $b (i32.const 1))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (local.set $temp (local.get $b))
        (local.set $b (i32.add (local.get $a) (local.get $b)))
        (local.set $a (local.get $temp))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $a)
    (local.get $b))

  ;; === Bulk Memory Operations ===

  ;; Copy memory region
  (func $memcpy (param $dst i32) (param $src i32) (param $len i32)
    (memory.copy (local.get $dst) (local.get $src) (local.get $len)))

  ;; Fill memory with byte value
  (func $memset (param $dst i32) (param $val i32) (param $len i32)
    (memory.fill (local.get $dst) (local.get $val) (local.get $len)))

  ;; Initialize memory from data segment
  (func $init_hello (param $dst i32)
    (memory.init $hello (local.get $dst) (i32.const 0) (i32.const 19)))

  ;; Drop data segment (after initialization)
  (func $drop_hello
    (data.drop $hello))

  ;; Get memory size in pages
  (func $get_mem_pages (result i32)
    (memory.size))

  ;; Grow memory by specified pages
  (func $grow_mem (param $pages i32) (result i32)
    (memory.grow (local.get $pages)))

  ;; Compare two memory regions
  (func $memcmp (param $a i32) (param $b i32) (param $len i32) (result i32)
    (local $i i32)
    (local $va i32)
    (local $vb i32)
    (block $done (result i32)
      (loop $loop
        (br_if $done (i32.const 0) (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $va (i32.load8_u (i32.add (local.get $a) (local.get $i))))
        (local.set $vb (i32.load8_u (i32.add (local.get $b) (local.get $i))))
        (if (i32.ne (local.get $va) (local.get $vb))
          (then
            (br $done (i32.sub (local.get $va) (local.get $vb)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))
      (i32.const 0)))

  ;; Copy overlapping regions safely
  (func $memmove (param $dst i32) (param $src i32) (param $len i32)
    ;; memory.copy handles overlapping regions correctly
    (memory.copy (local.get $dst) (local.get $src) (local.get $len)))

  ;; Zero out a region
  (func $zero_region (param $offset i32) (param $len i32)
    (memory.fill (local.get $offset) (i32.const 0) (local.get $len)))

  ;; Compute checksum of memory region
  (func $checksum (param $offset i32) (param $len i32) (result i32)
    (local $i i32)
    (local $sum i32)
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $sum
          (i32.add
            (local.get $sum)
            (i32.load8_u (i32.add (local.get $offset) (local.get $i)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $sum))

  ;; Return statistics: sum, count, average*100 (as int)
  (func $array_stats (param $offset i32) (param $len i32) (result i32 i32 i32)
    (local $i i32)
    (local $sum i32)
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $sum
          (i32.add
            (local.get $sum)
            (i32.load
              (i32.add (local.get $offset)
                (i32.shl (local.get $i) (i32.const 2))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $sum)
    (local.get $len)
    (i32.div_s
      (i32.mul (local.get $sum) (i32.const 100))
      (local.get $len)))

  ;; Exports
  (export "divmod" (func $divmod))
  (export "minmax" (func $minmax))
  (export "swap" (func $swap))
  (export "arithmetic_triple" (func $arithmetic_triple))
  (export "array_bounds" (func $array_bounds))
  (export "fib_pair" (func $fib_pair))
  (export "memcpy" (func $memcpy))
  (export "memset" (func $memset))
  (export "memcmp" (func $memcmp))
  (export "checksum" (func $checksum))
  (export "array_stats" (func $array_stats))
  (export "memory" (memory $mem)))
