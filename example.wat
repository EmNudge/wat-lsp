(module
  ;; Global counter variable
  (global $counter (mut i32) (i32.const 0))

  ;; Memory for storing data
  (memory $mem 1)

  ;; Add two numbers function
  (func $add (param $a i32) (param $b i32) (result i32)
    (local $result i32)

    ;; Add the parameters
    (local.set $result
      (i32.add
        (local.get $a)
        (local.get $b)))

    ;; Return the result
    (local.get $result))

  ;; Increment counter function
  (func $increment (result i32)
    (global.set $counter
      (i32.add
        (global.get $counter)
        (i32.const 1)))
    (global.get $counter))

  ;; Factorial function with loop
  (func $factorial (param $n i32) (result i32)
    (local $result i32)
    (local $i i32)

    (local.set $result (i32.const 1))
    (local.set $i (i32.const 1))

    (block $break
      (loop $continue
        ;; Check if i > n
        (br_if $break
          (i32.gt_s (local.get $i) (local.get $n)))

        ;; result = result * i
        (local.set $result
          (i32.mul
            (local.get $result)
            (local.get $i)))

        ;; i++
        (local.set $i
          (i32.add (local.get $i) (i32.const 1)))

        ;; Continue loop
        (br $continue)))

    (local.get $result))

  ;; Export functions
  (export "add" (func $add))
  (export "increment" (func $increment))
  (export "factorial" (func $factorial))
  (export "memory" (memory $mem)))
