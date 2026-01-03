;; Strings and Data Segments
;; Demonstrates data segments, string handling, and memory manipulation

(module
  (memory $mem 1)

  ;; === Active Data Segments (initialized at load time) ===

  ;; String constants at fixed offsets
  (data (i32.const 0) "Hello, WebAssembly!\00")
  (data (i32.const 32) "Error: Division by zero\00")
  (data (i32.const 64) "Error: Out of bounds\00")
  (data (i32.const 96) "Success\00")

  ;; Numeric data
  (data (i32.const 128) "\01\00\00\00\02\00\00\00\03\00\00\00\04\00\00\00\05\00\00\00")

  ;; Lookup table (ASCII to uppercase offset)
  (data (i32.const 256)
    "\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"  ;; 0-15
    "\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"  ;; 16-31
    "\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"  ;; 32-47
    "\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"  ;; 48-63
    "\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"  ;; 64-79
    "\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"  ;; 80-95
    "\00\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0"  ;; 96-111 (a-o -> A-O: -32)
    "\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0\e0\00\00\00\00\00") ;; 112-127 (p-z -> P-Z)

  ;; === Passive Data Segments (for memory.init) ===

  (data $greeting "Welcome to WASM!\00")
  (data $numbers "\0a\14\1e\28\32")  ;; 10, 20, 30, 40, 50
  (data $hex_digits "0123456789ABCDEF")

  ;; === String Functions ===

  ;; Get string length (null-terminated)
  (func $strlen (param $ptr i32) (result i32)
    (local $len i32)
    (block $done
      (loop $loop
        (br_if $done (i32.eqz (i32.load8_u (i32.add (local.get $ptr) (local.get $len)))))
        (local.set $len (i32.add (local.get $len) (i32.const 1)))
        (br $loop)))
    (local.get $len))

  ;; Copy string to destination
  (func $strcpy (param $dst i32) (param $src i32) (result i32)
    (local $i i32)
    (local $c i32)
    (block $done
      (loop $loop
        (local.set $c (i32.load8_u (i32.add (local.get $src) (local.get $i))))
        (i32.store8
          (i32.add (local.get $dst) (local.get $i))
          (local.get $c))
        (br_if $done (i32.eqz (local.get $c)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $dst))

  ;; Concatenate strings
  (func $strcat (param $dst i32) (param $src i32) (result i32)
    (local $dst_len i32)
    (local.set $dst_len (call $strlen (local.get $dst)))
    (drop (call $strcpy
      (i32.add (local.get $dst) (local.get $dst_len))
      (local.get $src)))
    (local.get $dst))

  ;; Compare strings (returns 0 if equal, <0 or >0 otherwise)
  (func $strcmp (param $s1 i32) (param $s2 i32) (result i32)
    (local $i i32)
    (local $c1 i32)
    (local $c2 i32)
    (block $done (result i32)
      (loop $loop
        (local.set $c1 (i32.load8_u (i32.add (local.get $s1) (local.get $i))))
        (local.set $c2 (i32.load8_u (i32.add (local.get $s2) (local.get $i))))
        ;; If different, return difference
        (br_if $done
          (i32.sub (local.get $c1) (local.get $c2))
          (i32.ne (local.get $c1) (local.get $c2)))
        ;; If both null, strings are equal
        (br_if $done (i32.const 0) (i32.eqz (local.get $c1)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))
      (i32.const 0)))

  ;; Find character in string (returns position or -1)
  (func $strchr (param $str i32) (param $char i32) (result i32)
    (local $i i32)
    (local $c i32)
    (block $not_found (result i32)
      (block $found (result i32)
        (loop $loop
          (local.set $c (i32.load8_u (i32.add (local.get $str) (local.get $i))))
          (br_if $found (local.get $i) (i32.eq (local.get $c) (local.get $char)))
          (br_if $not_found (i32.const -1) (i32.eqz (local.get $c)))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (br $loop))
        (i32.const -1))
      (i32.const -1)))

  ;; Convert string to uppercase (in place)
  (func $strupr (param $str i32)
    (local $i i32)
    (local $c i32)
    (block $done
      (loop $loop
        (local.set $c (i32.load8_u (i32.add (local.get $str) (local.get $i))))
        (br_if $done (i32.eqz (local.get $c)))
        ;; If lowercase letter (a-z)
        (if (i32.and
              (i32.ge_u (local.get $c) (i32.const 97))
              (i32.le_u (local.get $c) (i32.const 122)))
          (then
            (i32.store8
              (i32.add (local.get $str) (local.get $i))
              (i32.sub (local.get $c) (i32.const 32)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))

  ;; Convert string to lowercase (in place)
  (func $strlwr (param $str i32)
    (local $i i32)
    (local $c i32)
    (block $done
      (loop $loop
        (local.set $c (i32.load8_u (i32.add (local.get $str) (local.get $i))))
        (br_if $done (i32.eqz (local.get $c)))
        ;; If uppercase letter (A-Z)
        (if (i32.and
              (i32.ge_u (local.get $c) (i32.const 65))
              (i32.le_u (local.get $c) (i32.const 90)))
          (then
            (i32.store8
              (i32.add (local.get $str) (local.get $i))
              (i32.add (local.get $c) (i32.const 32)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))

  ;; Reverse string in place
  (func $strrev (param $str i32)
    (local $len i32)
    (local $i i32)
    (local $j i32)
    (local $tmp i32)
    (local.set $len (call $strlen (local.get $str)))
    (local.set $j (i32.sub (local.get $len) (i32.const 1)))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_s (local.get $i) (local.get $j)))
        ;; Swap characters
        (local.set $tmp (i32.load8_u (i32.add (local.get $str) (local.get $i))))
        (i32.store8
          (i32.add (local.get $str) (local.get $i))
          (i32.load8_u (i32.add (local.get $str) (local.get $j))))
        (i32.store8
          (i32.add (local.get $str) (local.get $j))
          (local.get $tmp))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $j (i32.sub (local.get $j) (i32.const 1)))
        (br $loop))))

  ;; === Number to String Conversion ===

  ;; Convert i32 to decimal string
  (func $itoa (param $num i32) (param $buf i32) (result i32)
    (local $i i32)
    (local $is_neg i32)
    (local $start i32)
    (local.set $start (local.get $buf))
    ;; Handle negative numbers
    (if (i32.lt_s (local.get $num) (i32.const 0))
      (then
        (local.set $is_neg (i32.const 1))
        (local.set $num (i32.sub (i32.const 0) (local.get $num)))))
    ;; Handle zero
    (if (i32.eqz (local.get $num))
      (then
        (i32.store8 (local.get $buf) (i32.const 48))  ;; '0'
        (i32.store8 (i32.add (local.get $buf) (i32.const 1)) (i32.const 0))
        (return (local.get $buf))))
    ;; Generate digits in reverse
    (block $done
      (loop $loop
        (br_if $done (i32.eqz (local.get $num)))
        (i32.store8
          (i32.add (local.get $buf) (local.get $i))
          (i32.add (i32.const 48) (i32.rem_u (local.get $num) (i32.const 10))))
        (local.set $num (i32.div_u (local.get $num) (i32.const 10)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    ;; Add negative sign if needed
    (if (local.get $is_neg)
      (then
        (i32.store8 (i32.add (local.get $buf) (local.get $i)) (i32.const 45))  ;; '-'
        (local.set $i (i32.add (local.get $i) (i32.const 1)))))
    ;; Null terminate
    (i32.store8 (i32.add (local.get $buf) (local.get $i)) (i32.const 0))
    ;; Reverse the string
    (call $strrev (local.get $buf))
    (local.get $start))

  ;; Convert i32 to hex string
  (func $itoa_hex (param $num i32) (param $buf i32) (result i32)
    (local $i i32)
    (local $digit i32)
    (local $start i32)
    (local.set $start (local.get $buf))
    ;; Handle zero
    (if (i32.eqz (local.get $num))
      (then
        (i32.store8 (local.get $buf) (i32.const 48))
        (i32.store8 (i32.add (local.get $buf) (i32.const 1)) (i32.const 0))
        (return (local.get $buf))))
    ;; Generate hex digits in reverse
    (block $done
      (loop $loop
        (br_if $done (i32.eqz (local.get $num)))
        (local.set $digit (i32.and (local.get $num) (i32.const 15)))
        ;; Use hex digits lookup from data segment
        (i32.store8
          (i32.add (local.get $buf) (local.get $i))
          (i32.load8_u (i32.add (i32.const 512) (local.get $digit))))  ;; Assuming hex_digits at 512
        (local.set $num (i32.shr_u (local.get $num) (i32.const 4)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    ;; Null terminate and reverse
    (i32.store8 (i32.add (local.get $buf) (local.get $i)) (i32.const 0))
    (call $strrev (local.get $buf))
    (local.get $start))

  ;; === String to Number Conversion ===

  ;; Parse decimal integer from string
  (func $atoi (param $str i32) (result i32)
    (local $result i32)
    (local $sign i32)
    (local $c i32)
    (local $i i32)
    (local.set $sign (i32.const 1))
    ;; Skip whitespace
    (block $ws_done
      (loop $ws_loop
        (local.set $c (i32.load8_u (i32.add (local.get $str) (local.get $i))))
        (br_if $ws_done (i32.ne (local.get $c) (i32.const 32)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $ws_loop)))
    ;; Check for sign
    (local.set $c (i32.load8_u (i32.add (local.get $str) (local.get $i))))
    (if (i32.eq (local.get $c) (i32.const 45))  ;; '-'
      (then
        (local.set $sign (i32.const -1))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))))
    (if (i32.eq (local.get $c) (i32.const 43))  ;; '+'
      (then
        (local.set $i (i32.add (local.get $i) (i32.const 1)))))
    ;; Parse digits
    (block $done
      (loop $loop
        (local.set $c (i32.load8_u (i32.add (local.get $str) (local.get $i))))
        ;; Check if digit
        (br_if $done (i32.lt_u (local.get $c) (i32.const 48)))
        (br_if $done (i32.gt_u (local.get $c) (i32.const 57)))
        (local.set $result
          (i32.add
            (i32.mul (local.get $result) (i32.const 10))
            (i32.sub (local.get $c) (i32.const 48))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (i32.mul (local.get $result) (local.get $sign)))

  ;; === Data Segment Operations ===

  ;; Initialize greeting to specified offset
  (func $init_greeting (param $dst i32)
    (memory.init $greeting (local.get $dst) (i32.const 0) (i32.const 17)))

  ;; Initialize numbers array
  (func $init_numbers (param $dst i32)
    (memory.init $numbers (local.get $dst) (i32.const 0) (i32.const 5)))

  ;; Initialize hex digits lookup
  (func $init_hex_table (param $dst i32)
    (memory.init $hex_digits (local.get $dst) (i32.const 0) (i32.const 16)))

  ;; Drop greeting segment (can no longer use memory.init on it)
  (func $drop_greeting
    (data.drop $greeting))

  ;; === Utility Functions ===

  ;; Get pointer to hello string
  (func $get_hello (result i32)
    (i32.const 0))

  ;; Get pointer to error messages
  (func $get_error_div_zero (result i32)
    (i32.const 32))

  (func $get_error_bounds (result i32)
    (i32.const 64))

  (func $get_success (result i32)
    (i32.const 96))

  ;; Get number from pre-initialized array
  (func $get_number (param $idx i32) (result i32)
    (i32.load
      (i32.add
        (i32.const 128)
        (i32.shl (local.get $idx) (i32.const 2)))))

  ;; Exports
  (export "strlen" (func $strlen))
  (export "strcpy" (func $strcpy))
  (export "strcat" (func $strcat))
  (export "strcmp" (func $strcmp))
  (export "strchr" (func $strchr))
  (export "strupr" (func $strupr))
  (export "strlwr" (func $strlwr))
  (export "strrev" (func $strrev))
  (export "itoa" (func $itoa))
  (export "itoa_hex" (func $itoa_hex))
  (export "atoi" (func $atoi))
  (export "init_greeting" (func $init_greeting))
  (export "get_hello" (func $get_hello))
  (export "get_number" (func $get_number))
  (export "memory" (memory $mem)))
