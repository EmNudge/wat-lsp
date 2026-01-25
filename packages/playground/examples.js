// Example WAT programs for the playground
export const watExamples = {
  hello: `(module
  ;; A simple module that adds two numbers

  ;; Export the add function
  (func (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add
      (local.get $a)
      (local.get $b)))

  ;; Export a multiply function
  (func (export "multiply") (param $a i32) (param $b i32) (result i32)
    (i32.mul
      (local.get $a)
      (local.get $b)))

  ;; Export a function that returns a constant
  (func (export "answer") (result i32)
    (i32.const 42)))
`,

  factorial: `(module
  ;; Compute factorial using recursion

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
`,

  fibonacci: `(module
  ;; Compute Fibonacci numbers

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
`,

  memory: `(module
  ;; Memory operations example
  ;; Shows how to use linear memory

  ;; Define 1 page (64KB) of memory and export it
  (memory (export "memory") 1)

  ;; Store an i32 at a given offset
  (func (export "store") (param $offset i32) (param $value i32)
    (i32.store
      (local.get $offset)
      (local.get $value)))

  ;; Load an i32 from a given offset
  (func (export "load") (param $offset i32) (result i32)
    (i32.load (local.get $offset)))

  ;; Store a byte
  (func (export "store_byte") (param $offset i32) (param $value i32)
    (i32.store8
      (local.get $offset)
      (local.get $value)))

  ;; Load a byte
  (func (export "load_byte") (param $offset i32) (result i32)
    (i32.load8_u (local.get $offset)))

  ;; Fill memory with a value
  (func (export "fill") (param $start i32) (param $value i32) (param $count i32)
    (local $i i32)
    (local.set $i (i32.const 0))

    (block $done
      (loop $continue
        (br_if $done (i32.ge_u (local.get $i) (local.get $count)))

        (i32.store8
          (i32.add (local.get $start) (local.get $i))
          (local.get $value))

        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue))))

  ;; Sum bytes in a range
  (func (export "sum_bytes") (param $start i32) (param $count i32) (result i32)
    (local $sum i32)
    (local $i i32)

    (local.set $sum (i32.const 0))
    (local.set $i (i32.const 0))

    (block $done
      (loop $continue
        (br_if $done (i32.ge_u (local.get $i) (local.get $count)))

        (local.set $sum
          (i32.add
            (local.get $sum)
            (i32.load8_u
              (i32.add (local.get $start) (local.get $i)))))

        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue)))

    (local.get $sum))

  ;; Get current memory size in pages
  (func (export "mem_size") (result i32)
    (memory.size))

  ;; Grow memory and return previous size (-1 on failure)
  (func (export "mem_grow") (param $pages i32) (result i32)
    (memory.grow (local.get $pages))))
`,

  imports: `(module
  ;; Example showing imports and exports

  ;; Import a logging function from the host
  (import "env" "log" (func $log (param i32)))

  ;; Import a function to log floats
  (import "env" "logFloat" (func $logFloat (param f64)))

  ;; Import memory from host (optional - we can also define our own)
  ;; (import "env" "memory" (memory 1))

  ;; Define our own memory
  (memory (export "memory") 1)

  ;; Global counter
  (global $counter (mut i32) (i32.const 0))
  (export "counter" (global $counter))

  ;; Increment counter and log
  (func (export "increment") (result i32)
    (global.set $counter
      (i32.add (global.get $counter) (i32.const 1)))
    (call $log (global.get $counter))
    (global.get $counter))

  ;; Reset counter
  (func (export "reset")
    (global.set $counter (i32.const 0))
    (call $log (i32.const 0)))

  ;; Compute and log PI approximation using Leibniz formula
  (func (export "computePi") (param $iterations i32) (result f64)
    (local $sum f64)
    (local $i i32)
    (local $sign f64)

    (local.set $sum (f64.const 0))
    (local.set $sign (f64.const 1))
    (local.set $i (i32.const 0))

    (block $done
      (loop $continue
        (br_if $done (i32.ge_s (local.get $i) (local.get $iterations)))

        ;; sum += sign / (2*i + 1)
        (local.set $sum
          (f64.add
            (local.get $sum)
            (f64.div
              (local.get $sign)
              (f64.convert_i32_s
                (i32.add
                  (i32.mul (local.get $i) (i32.const 2))
                  (i32.const 1))))))

        ;; sign = -sign
        (local.set $sign (f64.neg (local.get $sign)))

        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue)))

    ;; PI = 4 * sum
    (local.set $sum (f64.mul (local.get $sum) (f64.const 4)))
    (call $logFloat (local.get $sum))
    (local.get $sum))

  ;; A simple add function
  (func (export "add") (param $a i32) (param $b i32) (result i32)
    (local $result i32)
    (local.set $result (i32.add (local.get $a) (local.get $b)))
    (call $log (local.get $result))
    (local.get $result)))
`,

  simd: `(module
  ;; SIMD (Single Instruction Multiple Data) Example
  ;; Demonstrates v128 operations for parallel computation

  (memory (export "memory") 1)

  ;; Basic v128 operations
  (func (export "add_vectors") (param $a v128) (param $b v128) (result v128)
    (i32x4.add (local.get $a) (local.get $b)))

  ;; Create a constant vector
  (func (export "make_vector") (result v128)
    (v128.const i32x4 1 2 3 4))

  ;; Splat a single value to all lanes
  (func (export "splat") (param $val i32) (result v128)
    (i32x4.splat (local.get $val)))

  ;; Extract a lane from a vector
  (func (export "get_lane_0") (param $v v128) (result i32)
    (i32x4.extract_lane 0 (local.get $v)))

  ;; Replace a lane in a vector
  (func (export "set_lane_0") (param $v v128) (param $val i32) (result v128)
    (i32x4.replace_lane 0 (local.get $v) (local.get $val)))

  ;; ============================================================
  ;; Lane load/store operations (load/store individual lanes)
  ;; These were recently added to the grammar!
  ;; ============================================================

  ;; Load a 64-bit value into a specific lane
  (func (export "load_into_lane") (param $v v128) (param $addr i32) (result v128)
    (v128.load64_lane 0 (local.get $addr) (local.get $v)))

  ;; Store a 64-bit value from a specific lane
  (func (export "store_from_lane") (param $v v128) (param $addr i32)
    (v128.store64_lane 0 (local.get $addr) (local.get $v)))

  ;; Load 32-bit lanes
  (func (export "load32_into_lane") (param $v v128) (param $addr i32) (result v128)
    (v128.load32_lane 0 (local.get $addr) (local.get $v)))

  ;; Store 32-bit lanes
  (func (export "store32_from_lane") (param $v v128) (param $addr i32)
    (v128.store32_lane 0 (local.get $addr) (local.get $v)))

  ;; ============================================================
  ;; Zero-extending loads (load partial and zero the rest)
  ;; These were also recently added!
  ;; ============================================================

  ;; Load 64 bits and zero-extend to v128 (upper 64 bits = 0)
  (func (export "load64_zero") (param $addr i32) (result v128)
    (v128.load64_zero (local.get $addr)))

  ;; Load 32 bits and zero-extend to v128 (upper 96 bits = 0)
  (func (export "load32_zero") (param $addr i32) (result v128)
    (v128.load32_zero (local.get $addr)))

  ;; ============================================================
  ;; Practical example: Sum array elements using SIMD
  ;; ============================================================

  ;; Sum 4 i32 values in parallel from memory
  (func (export "sum_simd") (param $addr i32) (result i32)
    (local $vec v128)
    ;; Load 4 i32 values
    (local.set $vec (v128.load (local.get $addr)))
    ;; Sum all lanes
    (i32.add
      (i32.add
        (i32x4.extract_lane 0 (local.get $vec))
        (i32x4.extract_lane 1 (local.get $vec)))
      (i32.add
        (i32x4.extract_lane 2 (local.get $vec))
        (i32x4.extract_lane 3 (local.get $vec))))))
`
};
