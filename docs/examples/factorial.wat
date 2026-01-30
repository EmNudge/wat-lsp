;; Factorial
;; Compute factorial using recursion and iteration

(module
  (func $factorial (export "factorial") (param $n i32) (result i32)
    ;; Base case: if n <= 1, return 1
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then
        (i32.const 1))
      (else
        ;; Recursive case: n * factorial(n - 1)
        (i32.mul
          (local.get $n)
          (call $factorial
            (i32.sub (local.get $n) (i32.const 1)))))))

  ;; Iterative version using a loop
  (func $factorial_iter (export "factorial_iter") (param $n i32) (result i32)
    (local $result i32)
    (local $i i32)

    (local.set $result (i32.const 1))
    (local.set $i (i32.const 1))

    (block $done
      (loop $continue
        ;; If i > n, exit loop
        (br_if $done (i32.gt_s (local.get $i) (local.get $n)))

        ;; result = result * i
        (local.set $result
          (i32.mul (local.get $result) (local.get $i)))

        ;; i++
        (local.set $i
          (i32.add (local.get $i) (i32.const 1)))

        ;; Continue loop
        (br $continue)))

    (local.get $result)))
