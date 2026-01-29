;; Multi-memory test fixture
;; Tests the multi-memory proposal features

(module
  ;; Multiple memory definitions
  (memory $mem0 1)
  (memory $mem1 2 4)
  (memory $mem2 1 1)

  ;; Import a memory
  (import "env" "external_mem" (memory $imported 1))

  ;; Export memories
  (export "memory0" (memory $mem0))
  (export "memory1" (memory $mem1))

  ;; Data segments targeting different memories
  (data $d0 (memory $mem0) (i32.const 0) "hello")
  (data $d1 (memory $mem1) (i32.const 0) "world")
  (data $d2 (memory 2) (i32.const 0) "test")

  ;; Function using multi-memory instructions
  (func $multi_memory_ops (export "multi_memory_ops")
    ;; memory.size with explicit indices
    (drop (memory.size $mem0))
    (drop (memory.size $mem1))
    (drop (memory.size 0))
    (drop (memory.size 1))

    ;; memory.grow with explicit indices
    (drop (memory.grow $mem0 (i32.const 1)))
    (drop (memory.grow $mem1 (i32.const 1)))
    (drop (memory.grow 0 (i32.const 1)))
    (drop (memory.grow 1 (i32.const 1)))

    ;; memory.fill with explicit indices
    (memory.fill $mem0
      (i32.const 0)    ;; destination
      (i32.const 0)    ;; value
      (i32.const 10))  ;; count
    (memory.fill 1
      (i32.const 0)
      (i32.const 0)
      (i32.const 10))

    ;; memory.copy between memories
    (memory.copy $mem0 $mem1
      (i32.const 0)    ;; dest offset
      (i32.const 0)    ;; src offset
      (i32.const 10))  ;; length
    (memory.copy 0 1
      (i32.const 0)
      (i32.const 0)
      (i32.const 10))

    ;; memory.init with memory index
    (memory.init $mem0 $d0
      (i32.const 0)
      (i32.const 0)
      (i32.const 5))
    (memory.init 1 $d1
      (i32.const 0)
      (i32.const 0)
      (i32.const 5))
  )

  ;; Function with load/store to different memories
  (func $load_store_multi (param $addr i32) (result i32)
    ;; Load from memory 0
    (i32.store $mem0 (local.get $addr) (i32.const 42))
    ;; Load from memory 1
    (i32.store $mem1 (local.get $addr) (i32.const 43))
    ;; Load from memory 0
    (i32.load $mem0 (local.get $addr))
  )

  ;; Function with offset and align on different memories
  (func $load_with_offset (param $addr i32) (result i32)
    (i32.load $mem0 offset=4 align=4 (local.get $addr))
  )
)
