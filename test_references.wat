(module
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add)

  (func $main
    i32.const 5
    i32.const 3
    call $add
    drop

    i32.const 10
    i32.const 20
    call $add
    drop)
)
