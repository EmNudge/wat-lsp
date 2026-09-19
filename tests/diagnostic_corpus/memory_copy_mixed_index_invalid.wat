;; Invalid counterpart to memory_copy_mixed_index_valid.wat.
;; The destination memory is memory64, so its address operand must be i64, but an
;; i32 is supplied. Fixing the valid cross-memory case must not lose the ability
;; to catch a genuinely wrong address type.
(module
  (memory $m64 i64 1)
  (memory $m32 1)

  (func $bad_dst_addr
    i32.const 0   ;; dst addr: WRONG — memory64 requires i64
    i32.const 0   ;; src addr (i32, memory32)
    i32.const 0   ;; n
    memory.copy $m64 $m32))
