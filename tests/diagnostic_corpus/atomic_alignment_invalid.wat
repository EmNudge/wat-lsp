;; Atomic accesses require natural alignment: the align= value must equal the
;; access width exactly. Over-alignment (align=8 on a 4-byte access) and
;; under-alignment (align=2 on a 4-byte access) are both errors.
(module
  (memory 1 1 shared)

  (func $too_large (param $a i32) (result i32)
    local.get $a
    i32.atomic.load align=8)

  (func $too_small (param $a i32) (result i32)
    local.get $a
    i32.atomic.load align=2))
