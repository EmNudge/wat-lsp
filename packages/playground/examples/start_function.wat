;; Start Function
;; Demonstrates the start directive for module initialization

(module
  ;; Shared memory for initialization state
  (memory (export "memory") 1)

  ;; Global flag set by the start function
  (global $initialized (mut i32) (i32.const 0))

  ;; Initialize memory with a magic number and set the flag.
  ;; This runs automatically when the module is instantiated.
  (func $init
    ;; Write a magic header: 0x57415400 ("WAT\0" in ASCII)
    (i32.store (i32.const 0) (i32.const 0x00544157))

    ;; Store a version number at offset 4
    (i32.store (i32.const 4) (i32.const 1))

    ;; Fill offsets 8..15 with a default value
    (call $fill_range (i32.const 8) (i32.const 0xFF) (i32.const 8))

    ;; Mark as initialized
    (global.set $initialized (i32.const 1)))

  ;; Fill a range of bytes in memory
  (func $fill_range (param $offset i32) (param $value i32) (param $len i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (block $done
      (loop $continue
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (i32.store8
          (i32.add (local.get $offset) (local.get $i))
          (local.get $value))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue))))

  ;; Declare the start function — $init runs at instantiation
  (start $init)

  ;; Check whether the module was initialized
  (func (export "is_initialized") (result i32)
    (global.get $initialized))

  ;; Read the magic header
  (func (export "magic") (result i32)
    (i32.load (i32.const 0)))

  ;; Read the version
  (func (export "version") (result i32)
    (i32.load (i32.const 4)))

  ;; Read a byte from memory
  (func (export "read_byte") (param $offset i32) (result i32)
    (i32.load8_u (local.get $offset))))
