(module
  ;; Define functions
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add)

  (func $sub (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.sub)

  (func $noop)

  ;; Define a global
  (global $counter (mut i32) (i32.const 0))

  ;; Define a table and memory
  (table $tbl 4 funcref)
  (memory $mem 1)

  ;; Exports referencing defined symbols
  (export "add" (func $add))
  (export "sub" (func $sub))
  (export "counter" (global $counter))
  (export "table" (table $tbl))
  (export "memory" (memory $mem))

  ;; Elem segment populating the table with function references
  (elem (table $tbl) (i32.const 0) func $add $sub $noop)

  ;; Data segment writing to memory
  (data (memory $mem) (i32.const 0) "hello, world!")
)
