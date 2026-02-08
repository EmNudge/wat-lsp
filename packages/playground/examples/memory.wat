;; Memory Operations
;; Demonstrates linear memory load/store and manipulation

(module
  ;; Define 1 page (64KB) of memory and export it
  (memory (export "memory") 1)

  ;; Store an i32 at a given offset
  (func (export "store") (param $offset i32) (param $value i32)
    (i32.store
      (local.get $offset)
      (local.get $value)))

  ;; Load an i32 from a given offset
  (func (export "load") (param $offset i32) (result i32)
    (i32.load (local.get $offset)))

  ;; Store a byte
  (func (export "store_byte") (param $offset i32) (param $value i32)
    (i32.store8
      (local.get $offset)
      (local.get $value)))

  ;; Load a byte
  (func (export "load_byte") (param $offset i32) (result i32)
    (i32.load8_u (local.get $offset)))

  ;; Fill memory with a value
  (func (export "fill") (param $start i32) (param $value i32) (param $count i32)
    (local $i i32)
    (local.set $i (i32.const 0))

    (block $done
      (loop $continue
        (br_if $done (i32.ge_u (local.get $i) (local.get $count)))

        (i32.store8
          (i32.add (local.get $start) (local.get $i))
          (local.get $value))

        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue))))

  ;; Sum bytes in a range
  (func (export "sum_bytes") (param $start i32) (param $count i32) (result i32)
    (local $sum i32)
    (local $i i32)

    (local.set $sum (i32.const 0))
    (local.set $i (i32.const 0))

    (block $done
      (loop $continue
        (br_if $done (i32.ge_u (local.get $i) (local.get $count)))

        (local.set $sum
          (i32.add
            (local.get $sum)
            (i32.load8_u
              (i32.add (local.get $start) (local.get $i)))))

        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue)))

    (local.get $sum))

  ;; Get current memory size in pages
  (func (export "mem_size") (result i32)
    (memory.size))

  ;; Grow memory and return previous size (-1 on failure)
  (func (export "mem_grow") (param $pages i32) (result i32)
    (memory.grow (local.get $pages))))
