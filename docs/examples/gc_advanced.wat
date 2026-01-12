;; Advanced GC Features
;; Demonstrates rec types, sub types, casts, and more complex GC patterns

(module
  ;; === Recursive Type Definitions ===

  ;; Recursive type group for tree structure
  (rec
    (type $tree (sub (struct
      (field $value i32)
      (field $left (ref null $tree))
      (field $right (ref null $tree)))))

    (type $leaf (sub final $tree (struct
      (field $value i32)
      (field $left (ref null $tree))
      (field $right (ref null $tree))))))

  ;; Recursive type for linked list
  (rec
    (type $list_node (struct
      (field $head i32)
      (field $tail (ref null $list_node)))))

  ;; === Type Hierarchy ===

  ;; Base shape type
  (type $shape (sub (struct
    (field $x f64)
    (field $y f64))))

  ;; Circle extends shape
  (type $circle (sub $shape (struct
    (field $x f64)
    (field $y f64)
    (field $radius f64))))

  ;; Rectangle extends shape
  (type $rectangle (sub $shape (struct
    (field $x f64)
    (field $y f64)
    (field $width f64)
    (field $height f64))))

  ;; Final square type (cannot be extended)
  (type $square (sub final $rectangle (struct
    (field $x f64)
    (field $y f64)
    (field $width f64)
    (field $height f64))))

  ;; === Array Types ===

  ;; Immutable array of i32
  (type $const_i32_array (array i32))

  ;; Mutable array of f64
  (type $f64_array (array (mut f64)))

  ;; Array of struct references
  (type $shape_array (array (mut (ref null $shape))))

  ;; Packed arrays (i8, i16)
  (type $byte_array (array (mut i8)))
  (type $short_array (array (mut i16)))

  ;; === Tree Operations ===

  ;; Create a tree node
  (func $make_tree (param $val i32) (param $left (ref null $tree)) (param $right (ref null $tree)) (result (ref $tree))
    (struct.new $tree
      (local.get $val)
      (local.get $left)
      (local.get $right)))

  ;; Create a leaf node
  (func $make_leaf (param $val i32) (result (ref $leaf))
    (struct.new $leaf
      (local.get $val)
      (ref.null $tree)
      (ref.null $tree)))

  ;; Get tree value
  (func $tree_value (param $t (ref $tree)) (result i32)
    (struct.get $tree $value (local.get $t)))

  ;; Sum all values in tree (recursive)
  (func $tree_sum (param $t (ref null $tree)) (result i32)
    (if (result i32) (ref.is_null (local.get $t))
      (then (i32.const 0))
      (else
        (i32.add
          (struct.get $tree $value (ref.as_non_null (local.get $t)))
          (i32.add
            (call $tree_sum (struct.get $tree $left (ref.as_non_null (local.get $t))))
            (call $tree_sum (struct.get $tree $right (ref.as_non_null (local.get $t)))))))))

  ;; Count nodes in tree
  (func $tree_size (param $t (ref null $tree)) (result i32)
    (if (result i32) (ref.is_null (local.get $t))
      (then (i32.const 0))
      (else
        (i32.add
          (i32.const 1)
          (i32.add
            (call $tree_size (struct.get $tree $left (ref.as_non_null (local.get $t))))
            (call $tree_size (struct.get $tree $right (ref.as_non_null (local.get $t)))))))))

  ;; Tree depth
  (func $tree_depth (param $t (ref null $tree)) (result i32)
    (local $left_depth i32)
    (local $right_depth i32)
    (if (result i32) (ref.is_null (local.get $t))
      (then (i32.const 0))
      (else
        (local.set $left_depth
          (call $tree_depth (struct.get $tree $left (ref.as_non_null (local.get $t)))))
        (local.set $right_depth
          (call $tree_depth (struct.get $tree $right (ref.as_non_null (local.get $t)))))
        (i32.add
          (i32.const 1)
          (if (result i32) (i32.gt_s (local.get $left_depth) (local.get $right_depth))
            (then (local.get $left_depth))
            (else (local.get $right_depth)))))))

  ;; === Shape Operations ===

  ;; Create a circle
  (func $make_circle (param $x f64) (param $y f64) (param $r f64) (result (ref $circle))
    (struct.new $circle (local.get $x) (local.get $y) (local.get $r)))

  ;; Create a rectangle
  (func $make_rectangle (param $x f64) (param $y f64) (param $w f64) (param $h f64) (result (ref $rectangle))
    (struct.new $rectangle (local.get $x) (local.get $y) (local.get $w) (local.get $h)))

  ;; Create a square
  (func $make_square (param $x f64) (param $y f64) (param $side f64) (result (ref $square))
    (struct.new $square (local.get $x) (local.get $y) (local.get $side) (local.get $side)))

  ;; Get shape position
  (func $shape_position (param $s (ref $shape)) (result f64 f64)
    (struct.get $shape $x (local.get $s))
    (struct.get $shape $y (local.get $s)))

  ;; Move shape
  (func $move_shape (param $s (ref $shape)) (param $dx f64) (param $dy f64)
    (struct.set $shape $x (local.get $s)
      (f64.add (struct.get $shape $x (local.get $s)) (local.get $dx)))
    (struct.set $shape $y (local.get $s)
      (f64.add (struct.get $shape $y (local.get $s)) (local.get $dy))))

  ;; Circle area
  (func $circle_area (param $c (ref $circle)) (result f64)
    (f64.mul
      (f64.const 3.141592653589793)
      (f64.mul
        (struct.get $circle $radius (local.get $c))
        (struct.get $circle $radius (local.get $c)))))

  ;; Rectangle area
  (func $rectangle_area (param $r (ref $rectangle)) (result f64)
    (f64.mul
      (struct.get $rectangle $width (local.get $r))
      (struct.get $rectangle $height (local.get $r))))

  ;; === Type Casts and Tests ===

  ;; Check if shape is a circle
  (func $is_circle (param $s (ref $shape)) (result i32)
    (ref.test (ref $circle) (local.get $s)))

  ;; Check if shape is a rectangle
  (func $is_rectangle (param $s (ref $shape)) (result i32)
    (ref.test (ref $rectangle) (local.get $s)))

  ;; Cast shape to circle (returns null if not a circle)
  (func $as_circle (param $s (ref $shape)) (result (ref null $circle))
    (ref.cast (ref null $circle) (local.get $s)))

  ;; Cast shape to rectangle (returns null if not a rectangle)
  (func $as_rectangle (param $s (ref $shape)) (result (ref null $rectangle))
    (ref.cast (ref null $rectangle) (local.get $s)))

  ;; Calculate area for any shape
  (func $shape_area (param $s (ref $shape)) (result f64)
    (if (result f64) (ref.test (ref $circle) (local.get $s))
      (then
        (call $circle_area (ref.cast (ref $circle) (local.get $s))))
      (else
        (if (result f64) (ref.test (ref $rectangle) (local.get $s))
          (then
            (call $rectangle_area (ref.cast (ref $rectangle) (local.get $s))))
          (else
            (f64.const 0))))))

  ;; === Array Operations ===

  ;; Create array with initial value
  (func $create_f64_array (param $len i32) (param $init f64) (result (ref $f64_array))
    (array.new $f64_array (local.get $init) (local.get $len)))

  ;; Create array from data
  (func $create_byte_array (param $len i32) (result (ref $byte_array))
    (array.new_default $byte_array (local.get $len)))

  ;; Array get/set for f64
  (func $array_get_f64 (param $arr (ref $f64_array)) (param $idx i32) (result f64)
    (array.get $f64_array (local.get $arr) (local.get $idx)))

  (func $array_set_f64 (param $arr (ref $f64_array)) (param $idx i32) (param $val f64)
    (array.set $f64_array (local.get $arr) (local.get $idx) (local.get $val)))

  ;; Packed array operations (sign/zero extend)
  (func $get_byte_s (param $arr (ref $byte_array)) (param $idx i32) (result i32)
    (array.get_s $byte_array (local.get $arr) (local.get $idx)))

  (func $get_byte_u (param $arr (ref $byte_array)) (param $idx i32) (result i32)
    (array.get_u $byte_array (local.get $arr) (local.get $idx)))

  (func $set_byte (param $arr (ref $byte_array)) (param $idx i32) (param $val i32)
    (array.set $byte_array (local.get $arr) (local.get $idx) (local.get $val)))

  ;; Sum f64 array
  (func $sum_f64_array (param $arr (ref $f64_array)) (result f64)
    (local $i i32)
    (local $sum f64)
    (local $len i32)
    (local.set $len (array.len (local.get $arr)))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $sum
          (f64.add
            (local.get $sum)
            (array.get $f64_array (local.get $arr) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)))
    (local.get $sum))

  ;; Copy array segment
  (func $array_copy_f64 (param $dst (ref $f64_array)) (param $dst_idx i32)
                        (param $src (ref $f64_array)) (param $src_idx i32)
                        (param $len i32)
    (array.copy $f64_array $f64_array
      (local.get $dst) (local.get $dst_idx)
      (local.get $src) (local.get $src_idx)
      (local.get $len)))

  ;; Fill array with value
  (func $array_fill_f64 (param $arr (ref $f64_array)) (param $start i32) (param $val f64) (param $len i32)
    (array.fill $f64_array
      (local.get $arr) (local.get $start) (local.get $val) (local.get $len)))

  ;; === Linked List Operations ===

  ;; Create list node
  (func $cons (param $head i32) (param $tail (ref null $list_node)) (result (ref $list_node))
    (struct.new $list_node (local.get $head) (local.get $tail)))

  ;; List length
  (func $list_length (param $list (ref null $list_node)) (result i32)
    (if (result i32) (ref.is_null (local.get $list))
      (then (i32.const 0))
      (else
        (i32.add
          (i32.const 1)
          (call $list_length
            (struct.get $list_node $tail (ref.as_non_null (local.get $list))))))))

  ;; List sum
  (func $list_sum (param $list (ref null $list_node)) (result i32)
    (if (result i32) (ref.is_null (local.get $list))
      (then (i32.const 0))
      (else
        (i32.add
          (struct.get $list_node $head (ref.as_non_null (local.get $list)))
          (call $list_sum
            (struct.get $list_node $tail (ref.as_non_null (local.get $list))))))))

  ;; Reverse list
  (func $list_reverse (param $list (ref null $list_node)) (result (ref null $list_node))
    (local $result (ref null $list_node))
    (local $current (ref null $list_node))
    (local.set $current (local.get $list))
    (block $done
      (loop $loop
        (br_if $done (ref.is_null (local.get $current)))
        (local.set $result
          (struct.new $list_node
            (struct.get $list_node $head (ref.as_non_null (local.get $current)))
            (local.get $result)))
        (local.set $current
          (struct.get $list_node $tail (ref.as_non_null (local.get $current))))
        (br $loop)))
    (local.get $result))

  ;; Exports
  (export "make_tree" (func $make_tree))
  (export "make_leaf" (func $make_leaf))
  (export "tree_sum" (func $tree_sum))
  (export "tree_size" (func $tree_size))
  (export "tree_depth" (func $tree_depth))
  (export "make_circle" (func $make_circle))
  (export "make_rectangle" (func $make_rectangle))
  (export "make_square" (func $make_square))
  (export "shape_area" (func $shape_area))
  (export "is_circle" (func $is_circle))
  (export "is_rectangle" (func $is_rectangle))
  (export "create_f64_array" (func $create_f64_array))
  (export "sum_f64_array" (func $sum_f64_array))
  (export "cons" (func $cons))
  (export "list_length" (func $list_length))
  (export "list_sum" (func $list_sum))
  (export "list_reverse" (func $list_reverse)))
