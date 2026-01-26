(module
  ;; Real FFT (r2c) - f32 (Single Precision) Version
  ;;
  ;; Optimized with in-place post-processing to eliminate buffer copy.
  ;;
  ;; Algorithm:
  ;; 1. N real values are treated as N/2 complex: z[k] = x[2k] + i*x[2k+1]
  ;; 2. Run N/2-point Stockham FFT (result in primary buffer)
  ;; 3. In-place post-process: for k=1 to N/4, compute and store X[k], X[N/2-k]
  ;;
  ;; Memory layout (f32 version):
  ;;   0 - 32767:        Real input / complex output buffer
  ;;   32768 - 65535:    Secondary buffer (Stockham ping-pong)
  ;;   65536 - 98303:    Complex FFT twiddles (for N/2-point FFT)
  ;;   98304+:           Post-processing twiddles W_N^k for k=0..N/2

  ;; Stockham FFT imports - provided by fft_stockham_f32 module
  (import "stockham" "precompute_twiddles" (func $precompute_twiddles (param i32)))
  (import "stockham" "fft_stockham" (func $fft_stockham (param i32)))

  ;; Memory (3 pages = 192KB)
  (memory (export "memory") 3)

  ;; Post-processing twiddle offset (after complex FFT twiddles)
  (global $RFFT_TWIDDLE_OFFSET i32 (i32.const 98304))

  ;; Constants
  (global $PI f32 (f32.const 3.1415927))
  (global $HALF_PI f32 (f32.const 1.5707964))

  ;; Inline sin for twiddle precomputation
  (func $sin (param $x f32) (result f32)
    (local $x2 f32)
    (local $term f32)
    (local $sum f32)

    (if (f32.lt (local.get $x) (f32.neg (global.get $PI)))
      (then (local.set $x (f32.add (local.get $x) (f32.mul (f32.const 2.0) (global.get $PI))))))
    (if (f32.gt (local.get $x) (global.get $PI))
      (then (local.set $x (f32.sub (local.get $x) (f32.mul (f32.const 2.0) (global.get $PI))))))
    (if (f32.gt (local.get $x) (global.get $HALF_PI))
      (then (local.set $x (f32.sub (global.get $PI) (local.get $x)))))
    (if (f32.lt (local.get $x) (f32.neg (global.get $HALF_PI)))
      (then (local.set $x (f32.sub (f32.neg (global.get $PI)) (local.get $x)))))

    (local.set $x2 (f32.mul (local.get $x) (local.get $x)))
    (local.set $sum (local.get $x))
    (local.set $term (local.get $x))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -6.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -20.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -42.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -72.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -110.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.get $sum)
  )

  ;; Inline cos for twiddle precomputation
  (func $cos (param $x f32) (result f32)
    (local $x2 f32)
    (local $term f32)
    (local $sum f32)
    (local $sign f32)

    (if (f32.lt (local.get $x) (f32.neg (global.get $PI)))
      (then (local.set $x (f32.add (local.get $x) (f32.mul (f32.const 2.0) (global.get $PI))))))
    (if (f32.gt (local.get $x) (global.get $PI))
      (then (local.set $x (f32.sub (local.get $x) (f32.mul (f32.const 2.0) (global.get $PI))))))
    (local.set $sign (f32.const 1.0))
    (if (f32.gt (local.get $x) (global.get $HALF_PI))
      (then
        (local.set $x (f32.sub (global.get $PI) (local.get $x)))
        (local.set $sign (f32.const -1.0))))
    (if (f32.lt (local.get $x) (f32.neg (global.get $HALF_PI)))
      (then
        (local.set $x (f32.add (global.get $PI) (local.get $x)))
        (local.set $sign (f32.const -1.0))))

    (local.set $x2 (f32.mul (local.get $x) (local.get $x)))
    (local.set $sum (f32.const 1.0))
    (local.set $term (f32.const 1.0))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -2.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -12.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -30.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -56.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (local.set $term (f32.mul (local.get $term) (f32.div (local.get $x2) (f32.const -90.0))))
    (local.set $sum (f32.add (local.get $sum) (local.get $term)))
    (f32.mul (local.get $sum) (local.get $sign))
  )

  ;; Precompute twiddles for real FFT of size N
  (func $precompute_rfft_twiddles (export "precompute_rfft_twiddles") (param $n i32)
    (local $n2 i32)
    (local $k i32)
    (local $angle f32)
    (local $addr i32)
    (local $neg_two_pi_over_n f32)

    ;; n2 = n / 2
    (local.set $n2 (i32.shr_u (local.get $n) (i32.const 1)))

    ;; Precompute N/2-point FFT twiddles
    (call $precompute_twiddles (local.get $n2))

    ;; Precompute post-processing twiddles W_N^k for k = 0 to N/2
    (local.set $neg_two_pi_over_n
      (f32.div
        (f32.mul (f32.const -2.0) (global.get $PI))
        (f32.convert_i32_u (local.get $n))))

    (local.set $k (i32.const 0))
    (block $done
      (loop $loop
        (br_if $done (i32.gt_u (local.get $k) (local.get $n2)))

        (local.set $angle
          (f32.mul (f32.convert_i32_u (local.get $k)) (local.get $neg_two_pi_over_n)))

        ;; Store at RFFT_TWIDDLE_OFFSET + k * 8 bytes (each f32 complex is 8 bytes)
        (local.set $addr (i32.add (global.get $RFFT_TWIDDLE_OFFSET)
                                  (i32.shl (local.get $k) (i32.const 3))))

        (f32.store (local.get $addr) (call $cos (local.get $angle)))
        (f32.store (i32.add (local.get $addr) (i32.const 4)) (call $sin (local.get $angle)))

        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (br $loop)
      )
    )
  )

  ;; In-place Real FFT: N real inputs -> N/2+1 complex outputs
  ;; Input: N real values at offset 0 (float32, 4 bytes each)
  ;; Output: N/2+1 complex values at offset 0 (interleaved re,im, 8 bytes each)
  (func $rfft (export "rfft") (param $n i32)
    (local $n2 i32)
    (local $k i32)
    (local $k_end i32)
    (local $n2_minus_k i32)

    ;; Z[k] and Z[N/2-k] values
    (local $zk_re f32)
    (local $zk_im f32)
    (local $zn2k_re f32)
    (local $zn2k_im f32)

    ;; Twiddles
    (local $wk_re f32)
    (local $wk_im f32)
    (local $wn2k_re f32)
    (local $wn2k_im f32)

    ;; Intermediates for X[k]
    (local $sum_re f32)
    (local $sum_im f32)
    (local $diff_re f32)
    (local $diff_im f32)
    (local $wd_re f32)
    (local $wd_im f32)

    ;; Intermediates for X[N/2-k]
    (local $sum2_re f32)
    (local $sum2_im f32)
    (local $diff2_re f32)
    (local $diff2_im f32)
    (local $wd2_re f32)
    (local $wd2_im f32)

    ;; Results
    (local $xk_re f32)
    (local $xk_im f32)
    (local $xn2k_re f32)
    (local $xn2k_im f32)

    ;; Addresses
    (local $addr_k i32)
    (local $addr_n2k i32)

    ;; Z[0] for DC and Nyquist
    (local $z0_re f32)
    (local $z0_im f32)

    ;; n2 = n / 2
    (local.set $n2 (i32.shr_u (local.get $n) (i32.const 1)))

    ;; Step 1: Input already packed as z[k] = x[2k] + i*x[2k+1]
    ;; Step 2: Run N/2-point Stockham FFT
    (call $fft_stockham (local.get $n2))

    ;; Step 3: In-place post-processing

    ;; Save Z[0] before it gets overwritten
    (local.set $z0_re (f32.load (i32.const 0)))
    (local.set $z0_im (f32.load (i32.const 4)))

    ;; First, store DC (X[0]) - this overwrites Z[0] but we've saved it
    ;; X[0] = (Z[0].re + Z[0].im, 0)
    (f32.store (i32.const 0) (f32.add (local.get $z0_re) (local.get $z0_im)))
    (f32.store (i32.const 4) (f32.const 0.0))

    ;; Store Nyquist (X[N/2]) - position n2*8
    ;; X[N/2] = (Z[0].re - Z[0].im, 0)
    (local.set $addr_k (i32.shl (local.get $n2) (i32.const 3)))
    (f32.store (local.get $addr_k) (f32.sub (local.get $z0_re) (local.get $z0_im)))
    (f32.store (i32.add (local.get $addr_k) (i32.const 4)) (f32.const 0.0))

    ;; Process k from 1 to n2/2 (exclusive)
    ;; For each k, compute both X[k] and X[N/2-k] from Z[k] and Z[N/2-k]
    (local.set $k_end (i32.shr_u (local.get $n2) (i32.const 1)))
    (local.set $k (i32.const 1))

    (block $done_main
      (loop $main_loop
        (br_if $done_main (i32.ge_u (local.get $k) (local.get $k_end)))

        (local.set $n2_minus_k (i32.sub (local.get $n2) (local.get $k)))
        (local.set $addr_k (i32.shl (local.get $k) (i32.const 3)))
        (local.set $addr_n2k (i32.shl (local.get $n2_minus_k) (i32.const 3)))

        ;; Load Z[k]
        (local.set $zk_re (f32.load (local.get $addr_k)))
        (local.set $zk_im (f32.load (i32.add (local.get $addr_k) (i32.const 4))))

        ;; Load Z[N/2-k]
        (local.set $zn2k_re (f32.load (local.get $addr_n2k)))
        (local.set $zn2k_im (f32.load (i32.add (local.get $addr_n2k) (i32.const 4))))

        ;; Load twiddle W_N^k
        (local.set $wk_re (f32.load (i32.add (global.get $RFFT_TWIDDLE_OFFSET)
                                            (i32.shl (local.get $k) (i32.const 3)))))
        (local.set $wk_im (f32.load (i32.add (global.get $RFFT_TWIDDLE_OFFSET)
                                            (i32.add (i32.shl (local.get $k) (i32.const 3)) (i32.const 4)))))

        ;; Load twiddle W_N^{N/2-k}
        (local.set $wn2k_re (f32.load (i32.add (global.get $RFFT_TWIDDLE_OFFSET)
                                              (i32.shl (local.get $n2_minus_k) (i32.const 3)))))
        (local.set $wn2k_im (f32.load (i32.add (global.get $RFFT_TWIDDLE_OFFSET)
                                              (i32.add (i32.shl (local.get $n2_minus_k) (i32.const 3)) (i32.const 4)))))

        ;; === Compute X[k] ===
        ;; A = Z[k] + conj(Z[N/2-k])
        (local.set $sum_re (f32.add (local.get $zk_re) (local.get $zn2k_re)))
        (local.set $sum_im (f32.sub (local.get $zk_im) (local.get $zn2k_im)))

        ;; B = Z[k] - conj(Z[N/2-k])
        (local.set $diff_re (f32.sub (local.get $zk_re) (local.get $zn2k_re)))
        (local.set $diff_im (f32.add (local.get $zk_im) (local.get $zn2k_im)))

        ;; -i*W*B
        (local.set $wd_re (f32.add
          (f32.mul (local.get $wk_im) (local.get $diff_re))
          (f32.mul (local.get $wk_re) (local.get $diff_im))))
        (local.set $wd_im (f32.sub
          (f32.mul (local.get $wk_im) (local.get $diff_im))
          (f32.mul (local.get $wk_re) (local.get $diff_re))))

        ;; X[k] = 0.5*(A + (-i*W*B))
        (local.set $xk_re (f32.mul (f32.const 0.5)
          (f32.add (local.get $sum_re) (local.get $wd_re))))
        (local.set $xk_im (f32.mul (f32.const 0.5)
          (f32.add (local.get $sum_im) (local.get $wd_im))))

        ;; === Compute X[N/2-k] ===
        ;; A' = Z[N/2-k] + conj(Z[k])
        (local.set $sum2_re (f32.add (local.get $zn2k_re) (local.get $zk_re)))
        (local.set $sum2_im (f32.sub (local.get $zn2k_im) (local.get $zk_im)))

        ;; B' = Z[N/2-k] - conj(Z[k])
        (local.set $diff2_re (f32.sub (local.get $zn2k_re) (local.get $zk_re)))
        (local.set $diff2_im (f32.add (local.get $zn2k_im) (local.get $zk_im)))

        ;; -i*W'*B' using W_{N/2-k}
        (local.set $wd2_re (f32.add
          (f32.mul (local.get $wn2k_im) (local.get $diff2_re))
          (f32.mul (local.get $wn2k_re) (local.get $diff2_im))))
        (local.set $wd2_im (f32.sub
          (f32.mul (local.get $wn2k_im) (local.get $diff2_im))
          (f32.mul (local.get $wn2k_re) (local.get $diff2_re))))

        ;; X[N/2-k] = 0.5*(A' + (-i*W'*B'))
        (local.set $xn2k_re (f32.mul (f32.const 0.5)
          (f32.add (local.get $sum2_re) (local.get $wd2_re))))
        (local.set $xn2k_im (f32.mul (f32.const 0.5)
          (f32.add (local.get $sum2_im) (local.get $wd2_im))))

        ;; Store X[k] and X[N/2-k] in-place
        (f32.store (local.get $addr_k) (local.get $xk_re))
        (f32.store (i32.add (local.get $addr_k) (i32.const 4)) (local.get $xk_im))
        (f32.store (local.get $addr_n2k) (local.get $xn2k_re))
        (f32.store (i32.add (local.get $addr_n2k) (i32.const 4)) (local.get $xn2k_im))

        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (br $main_loop)
      )
    )

    ;; Handle middle element when N/2 is even (k = N/4)
    (if (i32.and (i32.eqz (i32.and (local.get $n2) (i32.const 1)))
                 (i32.gt_u (local.get $n2) (i32.const 2)))
      (then
        (local.set $addr_k (i32.shl (local.get $k_end) (i32.const 3)))

        ;; Load Z[N/4]
        (local.set $zk_re (f32.load (local.get $addr_k)))
        (local.set $zk_im (f32.load (i32.add (local.get $addr_k) (i32.const 4))))

        ;; Load W_{N/4}
        (local.set $wk_re (f32.load (i32.add (global.get $RFFT_TWIDDLE_OFFSET)
                                            (i32.shl (local.get $k_end) (i32.const 3)))))
        (local.set $wk_im (f32.load (i32.add (global.get $RFFT_TWIDDLE_OFFSET)
                                            (i32.add (i32.shl (local.get $k_end) (i32.const 3)) (i32.const 4)))))

        ;; For k = N/4, Z[N/2-k] = Z[k]
        ;; A = Z[k] + conj(Z[k]) = (2*zk_re, 0)
        ;; B = Z[k] - conj(Z[k]) = (0, 2*zk_im)
        (local.set $sum_re (f32.mul (f32.const 2.0) (local.get $zk_re)))
        (local.set $sum_im (f32.const 0.0))
        (local.set $diff_re (f32.const 0.0))
        (local.set $diff_im (f32.mul (f32.const 2.0) (local.get $zk_im)))

        ;; -i*W*B
        (local.set $wd_re (f32.add
          (f32.mul (local.get $wk_im) (local.get $diff_re))
          (f32.mul (local.get $wk_re) (local.get $diff_im))))
        (local.set $wd_im (f32.sub
          (f32.mul (local.get $wk_im) (local.get $diff_im))
          (f32.mul (local.get $wk_re) (local.get $diff_re))))

        ;; X[N/4] = 0.5*(A + (-i*W*B))
        (local.set $xk_re (f32.mul (f32.const 0.5)
          (f32.add (local.get $sum_re) (local.get $wd_re))))
        (local.set $xk_im (f32.mul (f32.const 0.5)
          (f32.add (local.get $sum_im) (local.get $wd_im))))

        ;; Store X[N/4]
        (f32.store (local.get $addr_k) (local.get $xk_re))
        (f32.store (i32.add (local.get $addr_k) (i32.const 4)) (local.get $xk_im))
      )
    )
  )
) ;; end module
