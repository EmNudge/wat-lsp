;; Typed Function References Proposal
;; Demonstrates call_ref, typed function references, and ref.func with types

(module
  ;; Define function types
  (type $unary (func (param i32) (result i32)))
  (type $binary (func (param i32 i32) (result i32)))
  (type $predicate (func (param i32) (result i32)))
  (type $consumer (func (param i32)))
  (type $producer (func (result i32)))
  (type $void (func))

  ;; Sub-typing with final types
  (type $base (sub (func (param i32) (result i32))))
  (type $derived (sub final $base (func (param i32) (result i32))))

  ;; Table of typed function references
  (table $unary_fns 4 (ref null $unary))
  (table $binary_fns 4 (ref null $binary))

  ;; Global typed function references
  (global $current_unary (mut (ref null $unary)) (ref.null $unary))
  (global $current_binary (mut (ref null $binary)) (ref.null $binary))

  ;; Memory for state
  (memory 1)
  (global $state (mut i32) (i32.const 0))

  ;; === Basic Functions ===

  (func $double (type $unary)
    (i32.mul (local.get 0) (i32.const 2)))

  (func $square (type $unary)
    (i32.mul (local.get 0) (local.get 0)))

  (func $increment (type $unary)
    (i32.add (local.get 0) (i32.const 1)))

  (func $negate (type $unary)
    (i32.sub (i32.const 0) (local.get 0)))

  (func $add (type $binary)
    (i32.add (local.get 0) (local.get 1)))

  (func $multiply (type $binary)
    (i32.mul (local.get 0) (local.get 1)))

  (func $subtract (type $binary)
    (i32.sub (local.get 0) (local.get 1)))

  (func $max (type $binary)
    (if (result i32) (i32.gt_s (local.get 0) (local.get 1))
      (then (local.get 0))
      (else (local.get 1))))

  ;; Initialize tables with typed refs
  (elem (table $unary_fns) (i32.const 0)
    (ref $unary) (ref.func $double) (ref.func $square) (ref.func $increment) (ref.func $negate))
  (elem (table $binary_fns) (i32.const 0)
    (ref $binary) (ref.func $add) (ref.func $multiply) (ref.func $subtract) (ref.func $max))

  ;; === call_ref Usage ===

  ;; Call a unary function reference directly
  (func $apply_unary (param $fn (ref $unary)) (param $x i32) (result i32)
    (call_ref $unary (local.get $x) (local.get $fn)))

  ;; Call a binary function reference
  (func $apply_binary (param $fn (ref $binary)) (param $a i32) (param $b i32) (result i32)
    (call_ref $binary (local.get $a) (local.get $b) (local.get $fn)))

  ;; Get a typed function ref from table and call it
  (func $call_unary_from_table (param $idx i32) (param $x i32) (result i32)
    (call_ref $unary
      (local.get $x)
      (ref.as_non_null (table.get $unary_fns (local.get $idx)))))

  ;; Get a typed function ref from table and call it (binary)
  (func $call_binary_from_table (param $idx i32) (param $a i32) (param $b i32) (result i32)
    (call_ref $binary
      (local.get $a)
      (local.get $b)
      (ref.as_non_null (table.get $binary_fns (local.get $idx)))))

  ;; === Higher-Order Functions ===

  ;; Map: apply function to each element in memory array
  (func $map (param $fn (ref $unary)) (param $offset i32) (param $len i32)
    (local $i i32)
    (local $addr i32)
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $addr
          (i32.add (local.get $offset) (i32.shl (local.get $i) (i32.const 2))))
        (i32.store
          (local.get $addr)
          (call_ref $unary
            (i32.load (local.get $addr))
            (local.get $fn)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))

  ;; Reduce/fold: combine all elements with binary function
  (func $reduce (param $fn (ref $binary)) (param $offset i32) (param $len i32) (param $init i32) (result i32)
    (local $i i32)
    (local $acc i32)
    (local.set $acc (local.get $init))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $acc
          (call_ref $binary
            (local.get $acc)
            (i32.load
              (i32.add (local.get $offset) (i32.shl (local.get $i) (i32.const 2))))
            (local.get $fn)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $acc))

  ;; Compose two unary functions
  (func $compose (param $f (ref $unary)) (param $g (ref $unary)) (param $x i32) (result i32)
    (call_ref $unary
      (call_ref $unary (local.get $x) (local.get $g))
      (local.get $f)))

  ;; Apply function n times
  (func $iterate (param $fn (ref $unary)) (param $x i32) (param $n i32) (result i32)
    (local $result i32)
    (local $i i32)
    (local.set $result (local.get $x))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (local.set $result
          (call_ref $unary (local.get $result) (local.get $fn)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $result))

  ;; === Global Function Reference Management ===

  ;; Set current unary function
  (func $set_unary (param $idx i32)
    (global.set $current_unary (table.get $unary_fns (local.get $idx))))

  ;; Call current unary function
  (func $call_current_unary (param $x i32) (result i32)
    (if (result i32) (ref.is_null (global.get $current_unary))
      (then (local.get $x))
      (else
        (call_ref $unary
          (local.get $x)
          (ref.as_non_null (global.get $current_unary))))))

  ;; Set current binary function
  (func $set_binary (param $idx i32)
    (global.set $current_binary (table.get $binary_fns (local.get $idx))))

  ;; === Return Function References ===

  ;; Return a unary function reference based on selector
  (func $get_unary (param $selector i32) (result (ref $unary))
    (if (result (ref $unary)) (i32.eq (local.get $selector) (i32.const 0))
      (then (ref.func $double))
      (else
        (if (result (ref $unary)) (i32.eq (local.get $selector) (i32.const 1))
          (then (ref.func $square))
          (else
            (if (result (ref $unary)) (i32.eq (local.get $selector) (i32.const 2))
              (then (ref.func $increment))
              (else (ref.func $negate))))))))

  ;; Return a binary function reference
  (func $get_binary (param $selector i32) (result (ref $binary))
    (if (result (ref $binary)) (i32.eq (local.get $selector) (i32.const 0))
      (then (ref.func $add))
      (else
        (if (result (ref $binary)) (i32.eq (local.get $selector) (i32.const 1))
          (then (ref.func $multiply))
          (else (ref.func $subtract))))))

  ;; === Predicate Functions ===

  (func $is_positive (type $predicate)
    (i32.gt_s (local.get 0) (i32.const 0)))

  (func $is_even (type $predicate)
    (i32.eqz (i32.and (local.get 0) (i32.const 1))))

  (func $is_zero (type $predicate)
    (i32.eqz (local.get 0)))

  ;; Filter: count elements matching predicate
  (func $count_if (param $pred (ref $predicate)) (param $offset i32) (param $len i32) (result i32)
    (local $i i32)
    (local $count i32)
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (call_ref $predicate
              (i32.load
                (i32.add (local.get $offset) (i32.shl (local.get $i) (i32.const 2))))
              (local.get $pred))
          (then
            (local.set $count (i32.add (local.get $count) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $count))

  ;; Find first element matching predicate, return index (-1 if not found)
  (func $find_if (param $pred (ref $predicate)) (param $offset i32) (param $len i32) (result i32)
    (local $i i32)
    (block $found (result i32)
      (loop $loop
        (br_if $found (i32.const -1) (i32.ge_u (local.get $i) (local.get $len)))
        (if (call_ref $predicate
              (i32.load
                (i32.add (local.get $offset) (i32.shl (local.get $i) (i32.const 2))))
              (local.get $pred))
          (then
            (br $found (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))
      (i32.const -1)))

  ;; Exports
  (export "apply_unary" (func $apply_unary))
  (export "apply_binary" (func $apply_binary))
  (export "call_unary_from_table" (func $call_unary_from_table))
  (export "call_binary_from_table" (func $call_binary_from_table))
  (export "map" (func $map))
  (export "reduce" (func $reduce))
  (export "compose" (func $compose))
  (export "iterate" (func $iterate))
  (export "get_unary" (func $get_unary))
  (export "get_binary" (func $get_binary))
  (export "count_if" (func $count_if))
  (export "find_if" (func $find_if))
  (export "memory" (memory 0)))
