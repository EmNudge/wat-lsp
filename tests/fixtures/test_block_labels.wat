(module
  (func $test
    (block $exit
      (loop $continue
        i32.const 1
        br_if $exit
        br $continue))
    i32.const 0
    br_if $exit))
