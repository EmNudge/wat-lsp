;; GC Proposal - Structs and Arrays
;; Demonstrates struct types, array types, and reference manipulation

(module
  ;; Define a point struct type
  (type $point (struct
    (field $x (mut f64))
    (field $y (mut f64))))

  ;; Define a 3D point that extends the base point concept
  (type $point3d (struct
    (field $x (mut f64))
    (field $y (mut f64))
    (field $z (mut f64))))

  ;; Define a color struct
  (type $color (struct
    (field $r i32)
    (field $g i32)
    (field $b i32)
    (field $a i32)))

  ;; Define an array of i32 values
  (type $int_array (array (mut i32)))

  ;; Define an array of point references
  (type $point_array (array (mut (ref null $point))))

  ;; Define a linked list node
  (type $node (struct
    (field $value i32)
    (field $next (ref null $node))))

  ;; Create a new point
  (func $new_point (param $x f64) (param $y f64) (result (ref $point))
    (struct.new $point
      (local.get $x)
      (local.get $y)))

  ;; Get the X coordinate of a point
  (func $get_x (param $p (ref $point)) (result f64)
    (struct.get $point $x (local.get $p)))

  ;; Get the Y coordinate of a point
  (func $get_y (param $p (ref $point)) (result f64)
    (struct.get $point $y (local.get $p)))

  ;; Set the X coordinate of a point
  (func $set_x (param $p (ref $point)) (param $val f64)
    (struct.set $point $x (local.get $p) (local.get $val)))

  ;; Calculate distance from origin
  (func $distance_from_origin (param $p (ref $point)) (result f64)
    (f64.sqrt
      (f64.add
        (f64.mul
          (struct.get $point $x (local.get $p))
          (struct.get $point $x (local.get $p)))
        (f64.mul
          (struct.get $point $y (local.get $p))
          (struct.get $point $y (local.get $p))))))

  ;; Create an array of integers with given length
  (func $new_int_array (param $len i32) (result (ref $int_array))
    (array.new $int_array
      (i32.const 0)
      (local.get $len)))

  ;; Get element from int array
  (func $get_int (param $arr (ref $int_array)) (param $idx i32) (result i32)
    (array.get $int_array (local.get $arr) (local.get $idx)))

  ;; Set element in int array
  (func $set_int (param $arr (ref $int_array)) (param $idx i32) (param $val i32)
    (array.set $int_array (local.get $arr) (local.get $idx) (local.get $val)))

  ;; Get array length
  (func $array_len (param $arr (ref $int_array)) (result i32)
    (array.len (local.get $arr)))

  ;; Create a linked list node
  (func $new_node (param $value i32) (param $next (ref null $node)) (result (ref $node))
    (struct.new $node
      (local.get $value)
      (local.get $next)))

  ;; Sum all values in a linked list
  (func $sum_list (param $head (ref null $node)) (result i32)
    (local $sum i32)
    (local $current (ref null $node))
    (local.set $current (local.get $head))
    (block $done
      (loop $loop
        (br_if $done (ref.is_null (local.get $current)))
        (local.set $sum
          (i32.add
            (local.get $sum)
            (struct.get $node $value (ref.as_non_null (local.get $current)))))
        (local.set $current
          (struct.get $node $next (ref.as_non_null (local.get $current))))
        (br $loop)))
    (local.get $sum))

  ;; Test if two points are equal
  (func $points_equal (param $a (ref $point)) (param $b (ref $point)) (result i32)
    (i32.and
      (f64.eq
        (struct.get $point $x (local.get $a))
        (struct.get $point $x (local.get $b)))
      (f64.eq
        (struct.get $point $y (local.get $a))
        (struct.get $point $y (local.get $b)))))

  ;; Create point using struct.new_default (all fields zeroed)
  (func $new_origin (result (ref $point))
    (struct.new_default $point))

  ;; Exports
  (export "new_point" (func $new_point))
  (export "get_x" (func $get_x))
  (export "get_y" (func $get_y))
  (export "distance_from_origin" (func $distance_from_origin))
  (export "new_int_array" (func $new_int_array))
  (export "sum_list" (func $sum_list)))
