;; memory.copy between memories with differing index types.
;; The address operands must be typed from each memory independently: the
;; destination address takes the destination memory's index type and the source
;; address takes the source memory's index type. The count `n` is i64 only when
;; both memories are i64, otherwise i32.
;;
;; Regression test: memory.copy previously derived a single address type from the
;; first memory operand and applied it to all three stack operands, producing
;; false "type mismatch" errors for valid cross-memory copies.
(module
  (memory $m64 i64 1)
  (memory $m32 1)

  ;; dst = memory64 (i64 addr), src = memory32 (i32 addr), n = i32
  (func $copy_64_from_32
    i64.const 0   ;; dst addr (i64)
    i32.const 0   ;; src addr (i32)
    i32.const 0   ;; n       (i32)
    memory.copy $m64 $m32)

  ;; dst = memory32 (i32 addr), src = memory64 (i64 addr), n = i32
  (func $copy_32_from_64
    i32.const 0   ;; dst addr (i32)
    i64.const 0   ;; src addr (i64)
    i32.const 0   ;; n       (i32)
    memory.copy $m32 $m64)

  ;; both memory64: all three operands are i64
  (func $copy_64_from_64
    i64.const 0   ;; dst addr (i64)
    i64.const 0   ;; src addr (i64)
    i64.const 0   ;; n       (i64)
    memory.copy $m64 $m64))
