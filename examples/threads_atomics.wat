;; Threads and Atomics Proposal
;; Demonstrates shared memory, atomic operations, and wait/notify

(module
  ;; Shared memory for multi-threaded access
  (memory $shared_mem 1 4 shared)

  ;; === Atomic Load/Store Operations ===

  ;; Atomically load i32
  (func $atomic_load (param $addr i32) (result i32)
    (i32.atomic.load (local.get $addr)))

  ;; Atomically store i32
  (func $atomic_store (param $addr i32) (param $val i32)
    (i32.atomic.store (local.get $addr) (local.get $val)))

  ;; Atomically load i64
  (func $atomic_load64 (param $addr i32) (result i64)
    (i64.atomic.load (local.get $addr)))

  ;; Atomically store i64
  (func $atomic_store64 (param $addr i32) (param $val i64)
    (i64.atomic.store (local.get $addr) (local.get $val)))

  ;; === Atomic Read-Modify-Write Operations ===

  ;; Atomic add and return old value
  (func $atomic_add (param $addr i32) (param $val i32) (result i32)
    (i32.atomic.rmw.add (local.get $addr) (local.get $val)))

  ;; Atomic subtract and return old value
  (func $atomic_sub (param $addr i32) (param $val i32) (result i32)
    (i32.atomic.rmw.sub (local.get $addr) (local.get $val)))

  ;; Atomic AND and return old value
  (func $atomic_and (param $addr i32) (param $val i32) (result i32)
    (i32.atomic.rmw.and (local.get $addr) (local.get $val)))

  ;; Atomic OR and return old value
  (func $atomic_or (param $addr i32) (param $val i32) (result i32)
    (i32.atomic.rmw.or (local.get $addr) (local.get $val)))

  ;; Atomic XOR and return old value
  (func $atomic_xor (param $addr i32) (param $val i32) (result i32)
    (i32.atomic.rmw.xor (local.get $addr) (local.get $val)))

  ;; Atomic exchange (swap) and return old value
  (func $atomic_xchg (param $addr i32) (param $val i32) (result i32)
    (i32.atomic.rmw.xchg (local.get $addr) (local.get $val)))

  ;; === Compare and Exchange ===

  ;; Compare and exchange: if [addr] == expected, set to replacement
  ;; Returns old value
  (func $atomic_cmpxchg (param $addr i32) (param $expected i32) (param $replacement i32) (result i32)
    (i32.atomic.rmw.cmpxchg
      (local.get $addr)
      (local.get $expected)
      (local.get $replacement)))

  ;; 64-bit compare and exchange
  (func $atomic_cmpxchg64 (param $addr i32) (param $expected i64) (param $replacement i64) (result i64)
    (i64.atomic.rmw.cmpxchg
      (local.get $addr)
      (local.get $expected)
      (local.get $replacement)))

  ;; === Wait and Notify ===

  ;; Wait on address until value changes or timeout
  ;; Returns: 0 = ok (was woken), 1 = not equal, 2 = timed out
  (func $atomic_wait (param $addr i32) (param $expected i32) (param $timeout i64) (result i32)
    (memory.atomic.wait32
      (local.get $addr)
      (local.get $expected)
      (local.get $timeout)))

  ;; 64-bit wait
  (func $atomic_wait64 (param $addr i32) (param $expected i64) (param $timeout i64) (result i32)
    (memory.atomic.wait64
      (local.get $addr)
      (local.get $expected)
      (local.get $timeout)))

  ;; Notify waiters on address
  ;; Returns number of waiters woken
  (func $atomic_notify (param $addr i32) (param $count i32) (result i32)
    (memory.atomic.notify (local.get $addr) (local.get $count)))

  ;; === Atomic Fence ===

  ;; Memory fence for ordering
  (func $atomic_fence
    (atomic.fence))

  ;; === Practical Examples ===

  ;; Spinlock acquire (returns when lock acquired)
  (func $spinlock_acquire (param $lock_addr i32)
    (local $old i32)
    (block $acquired
      (loop $spin
        ;; Try to set lock from 0 to 1
        (local.set $old
          (i32.atomic.rmw.cmpxchg
            (local.get $lock_addr)
            (i32.const 0)
            (i32.const 1)))
        ;; If old value was 0, we acquired the lock
        (br_if $acquired (i32.eqz (local.get $old)))
        ;; Otherwise, spin and try again
        (br $spin))))

  ;; Spinlock release
  (func $spinlock_release (param $lock_addr i32)
    (i32.atomic.store (local.get $lock_addr) (i32.const 0)))

  ;; Mutex lock with wait (more efficient than spinlock)
  (func $mutex_lock (param $mutex_addr i32)
    (local $old i32)
    (block $acquired
      (loop $retry
        ;; Try to acquire: 0 (unlocked) -> 1 (locked)
        (local.set $old
          (i32.atomic.rmw.cmpxchg
            (local.get $mutex_addr)
            (i32.const 0)
            (i32.const 1)))
        (br_if $acquired (i32.eqz (local.get $old)))
        ;; Wait until value is 0 (with -1 timeout = infinite)
        (drop
          (memory.atomic.wait32
            (local.get $mutex_addr)
            (i32.const 1)
            (i64.const -1)))
        (br $retry))))

  ;; Mutex unlock with notify
  (func $mutex_unlock (param $mutex_addr i32)
    (i32.atomic.store (local.get $mutex_addr) (i32.const 0))
    ;; Wake one waiter
    (drop (memory.atomic.notify (local.get $mutex_addr) (i32.const 1))))

  ;; Atomic increment counter
  (func $increment_counter (param $counter_addr i32) (result i32)
    (i32.atomic.rmw.add (local.get $counter_addr) (i32.const 1)))

  ;; Atomic decrement counter
  (func $decrement_counter (param $counter_addr i32) (result i32)
    (i32.atomic.rmw.sub (local.get $counter_addr) (i32.const 1)))

  ;; Semaphore wait (decrement if > 0, otherwise wait)
  (func $semaphore_wait (param $sem_addr i32)
    (local $old i32)
    (block $acquired
      (loop $retry
        (local.set $old (i32.atomic.load (local.get $sem_addr)))
        ;; If semaphore is 0, wait
        (if (i32.eqz (local.get $old))
          (then
            (drop
              (memory.atomic.wait32
                (local.get $sem_addr)
                (i32.const 0)
                (i64.const -1)))
            (br $retry)))
        ;; Try to decrement
        (if (i32.eq
              (local.get $old)
              (i32.atomic.rmw.cmpxchg
                (local.get $sem_addr)
                (local.get $old)
                (i32.sub (local.get $old) (i32.const 1))))
          (then (br $acquired)))
        (br $retry))))

  ;; Semaphore signal (increment and notify)
  (func $semaphore_signal (param $sem_addr i32)
    (drop (i32.atomic.rmw.add (local.get $sem_addr) (i32.const 1)))
    (drop (memory.atomic.notify (local.get $sem_addr) (i32.const 1))))

  ;; Barrier wait (all threads must reach barrier)
  (func $barrier_wait (param $barrier_addr i32) (param $count_addr i32) (param $total i32)
    (local $arrived i32)
    ;; Increment arrival count
    (local.set $arrived
      (i32.add
        (i32.atomic.rmw.add (local.get $count_addr) (i32.const 1))
        (i32.const 1)))
    ;; If we're the last to arrive, notify all
    (if (i32.eq (local.get $arrived) (local.get $total))
      (then
        (i32.atomic.store (local.get $barrier_addr) (i32.const 1))
        (drop
          (memory.atomic.notify
            (local.get $barrier_addr)
            (i32.sub (local.get $total) (i32.const 1)))))
      (else
        ;; Wait for barrier to open
        (drop
          (memory.atomic.wait32
            (local.get $barrier_addr)
            (i32.const 0)
            (i64.const -1))))))

  ;; 8-bit and 16-bit atomic operations
  (func $atomic_load8 (param $addr i32) (result i32)
    (i32.atomic.load8_u (local.get $addr)))

  (func $atomic_store8 (param $addr i32) (param $val i32)
    (i32.atomic.store8 (local.get $addr) (local.get $val)))

  (func $atomic_load16 (param $addr i32) (result i32)
    (i32.atomic.load16_u (local.get $addr)))

  (func $atomic_store16 (param $addr i32) (param $val i32)
    (i32.atomic.store16 (local.get $addr) (local.get $val)))

  ;; Atomic fetch-and-add for 8-bit
  (func $atomic_add8 (param $addr i32) (param $val i32) (result i32)
    (i32.atomic.rmw8.add_u (local.get $addr) (local.get $val)))

  ;; Exports
  (export "atomic_load" (func $atomic_load))
  (export "atomic_store" (func $atomic_store))
  (export "atomic_add" (func $atomic_add))
  (export "atomic_cmpxchg" (func $atomic_cmpxchg))
  (export "atomic_wait" (func $atomic_wait))
  (export "atomic_notify" (func $atomic_notify))
  (export "spinlock_acquire" (func $spinlock_acquire))
  (export "spinlock_release" (func $spinlock_release))
  (export "mutex_lock" (func $mutex_lock))
  (export "mutex_unlock" (func $mutex_unlock))
  (export "increment_counter" (func $increment_counter))
  (export "semaphore_wait" (func $semaphore_wait))
  (export "semaphore_signal" (func $semaphore_signal))
  (export "memory" (memory $shared_mem)))
