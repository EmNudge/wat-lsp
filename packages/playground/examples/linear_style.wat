;; Linear (Unfolded) Style
;; Demonstrates stack-based programming where values are pushed
;; to the stack before operations consume them.
;; This is an alternative to the more common folded/S-expression style.

(module
  ;; Helper function: add two numbers
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add)

  ;; Helper function: multiply two numbers
  (func $mul (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.mul)

  ;; Compute (a + b) * (c + d) using purely linear style
  (func $compute (export "compute")
        (param $a i32) (param $b i32) (param $c i32) (param $d i32)
        (result i32)
    ;; Push a and b, then call $add
    local.get $a
    local.get $b
    call $add

    ;; Push c and d, then call $add
    local.get $c
    local.get $d
    call $add

    ;; Multiply the two results (both are on the stack)
    call $mul)

  ;; Same computation mixing folded and unfolded styles
  (func $compute_mixed (export "compute_mixed")
        (param $a i32) (param $b i32) (param $c i32) (param $d i32)
        (result i32)
    ;; First addition in folded style, result goes on stack
    (call $add (local.get $a) (local.get $b))

    ;; Second addition in unfolded style
    local.get $c
    local.get $d
    call $add

    ;; Multiply both results
    call $mul)

  ;; Factorial using linear style with a loop
  (func $factorial (export "factorial") (param $n i32) (result i32)
    (local $result i32)
    (local $i i32)

    ;; Initialize result = 1
    i32.const 1
    local.set $result

    ;; Initialize i = 2 (we'll multiply from 2 to n)
    i32.const 2
    local.set $i

    ;; Loop while i <= n
    block $done
      loop $continue
        ;; Check if i > n, if so exit
        local.get $i
        local.get $n
        i32.gt_s
        br_if $done

        ;; result = result * i
        local.get $result
        local.get $i
        i32.mul
        local.set $result

        ;; i = i + 1
        local.get $i
        i32.const 1
        i32.add
        local.set $i

        ;; Continue loop
        br $continue
      end
    end

    ;; Return result
    local.get $result)

  ;; Demonstrate global access in linear style
  (global $counter (mut i32) (i32.const 0))

  (func $increment (export "increment") (result i32)
    ;; Load current counter value
    global.get $counter

    ;; Add 1 to it
    i32.const 1
    i32.add

    ;; Store back (but also keep on stack for return)
    ;; Need to duplicate: load, add, tee to local, store, get local
    ;; Actually simpler: load, add 1, set, load again
    global.set $counter
    global.get $counter)

  ;; Memory operations in linear style
  (memory $mem 1)

  (func $store_and_load (export "store_and_load") (param $addr i32) (param $value i32) (result i32)
    ;; Store value at address
    local.get $addr
    local.get $value
    i32.store

    ;; Load and return it
    local.get $addr
    i32.load)

  ;; Conditional logic in linear style
  (func $max (export "max") (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    local.get $a
    local.get $b
    i32.gt_s
    select)

  ;; Demonstrating br_if with stack values
  (func $sum_to_n (export "sum_to_n") (param $n i32) (result i32)
    (local $sum i32)
    (local $i i32)

    i32.const 0
    local.set $sum

    i32.const 1
    local.set $i

    block $done
      loop $continue
        ;; if i > n, break
        local.get $i
        local.get $n
        i32.gt_s
        br_if $done

        ;; sum += i
        local.get $sum
        local.get $i
        i32.add
        local.set $sum

        ;; i++
        local.get $i
        i32.const 1
        i32.add
        local.set $i

        br $continue
      end
    end

    local.get $sum)

  ;; Multiple return values (multi-value) in linear style
  (func $divmod (export "divmod") (param $a i32) (param $b i32) (result i32 i32)
    ;; Push quotient
    local.get $a
    local.get $b
    i32.div_s

    ;; Push remainder
    local.get $a
    local.get $b
    i32.rem_s)

  ;; Call a function that returns multiple values
  (func $use_divmod (export "use_divmod") (param $a i32) (param $b i32) (result i32)
    ;; Call divmod, get quotient and remainder on stack
    local.get $a
    local.get $b
    call $divmod

    ;; Add quotient + remainder
    i32.add)
)
