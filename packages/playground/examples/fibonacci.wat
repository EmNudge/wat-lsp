;; Fibonacci
;; Compute Fibonacci numbers using recursion and iteration

(module
  ;; Recursive Fibonacci (slow for large n)
  (func $fib (export "fib") (param $n i32) (result i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then
        (local.get $n))
      (else
        (i32.add
          (call $fib (i32.sub (local.get $n) (i32.const 1)))
          (call $fib (i32.sub (local.get $n) (i32.const 2)))))))

  ;; Iterative Fibonacci (fast)
  (func $fib_fast (export "fib_fast") (param $n i32) (result i32)
    (local $a i32)
    (local $b i32)
    (local $temp i32)
    (local $i i32)

    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then
        (local.get $n))
      (else
        (local.set $a (i32.const 0))
        (local.set $b (i32.const 1))
        (local.set $i (i32.const 2))

        (block $done
          (loop $continue
            (br_if $done (i32.gt_s (local.get $i) (local.get $n)))

            ;; temp = a + b
            (local.set $temp
              (i32.add (local.get $a) (local.get $b)))

            ;; a = b
            (local.set $a (local.get $b))

            ;; b = temp
            (local.set $b (local.get $temp))

            ;; i++
            (local.set $i
              (i32.add (local.get $i) (i32.const 1)))

            (br $continue)))

        (local.get $b)))))
