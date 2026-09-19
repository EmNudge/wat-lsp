;; On a memory64 module the atomic address must be i64. Passing an i32 address
;; to an atomic rmw is a type error the semantic pass must report.
(module
  (memory i64 1 1 shared)
  (func $rmw (param $addr i32) (param $val i32) (result i32)
    local.get $addr
    local.get $val
    i32.atomic.rmw.add))
