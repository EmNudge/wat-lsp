;; Valid: blocks with complex control flow that always terminates
(module
  ;; All paths through if terminate with return
  (func $if_terminates (param $cond i32) (result i32)
    (if (result i32) (local.get $cond)
      (then (return (i32.const 1)))
      (else (return (i32.const 0)))))

  ;; Block with br that always branches - the i32.const after is dead code
  (func $block_br_terminates (result i32)
    (block $exit (result i32)
      (br $exit (i32.const 42))
      (i32.const 0)))  ;; never reached, but provides fallthrough value

  ;; Loop that always returns (doesn't fall through)
  (func $loop_terminates (result i32)
    (loop $again (result i32)
      (return (i32.const 99))))
)
