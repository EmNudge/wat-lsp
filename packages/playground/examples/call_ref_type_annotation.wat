;; call_ref / return_call_ref with bare type index
;; Unlike call_indirect which uses (type $t), call_ref takes a bare type index:
;;   call_ref $type       - named type index
;;   call_ref 0           - numeric type index

(module
  (type $unary (func (param i32) (result i32)))
  (type $binary (func (param i32 i32) (result i32)))

  ;; --- Basic functions ---

  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  ;; --- call_ref with named type index (s-expression style) ---

  (func $apply_unary (param $fn (ref $unary)) (param $x i32) (result i32)
    (call_ref $unary (local.get $x) (local.get $fn)))

  (func $apply_binary (param $fn (ref $binary)) (param $a i32) (param $b i32) (result i32)
    (call_ref $binary (local.get $a) (local.get $b) (local.get $fn)))

  ;; --- call_ref with named type index (linear/stack style) ---

  (func $apply_unary_linear (param $fn (ref $unary)) (param $x i32) (result i32)
    local.get $x
    local.get $fn
    call_ref $unary)

  (func $apply_binary_linear (param $fn (ref $binary)) (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    local.get $fn
    call_ref $binary)

  ;; --- return_call_ref with bare type index (tail call) ---

  (func $tail_apply (param $fn (ref $unary)) (param $x i32) (result i32)
    local.get $x
    local.get $fn
    return_call_ref $unary)

  ;; --- Mixing named and numeric type indices ---

  (func $double_then_add (param $x i32) (param $y i32) (result i32)
    (call_ref $binary
      (call_ref $unary (local.get $x) (ref.func $double))
      (local.get $y)
      (ref.func $add)))

  ;; --- Numeric type index ---

  (func $call_by_index (param $fn (ref $unary)) (param $x i32) (result i32)
    (call_ref 0 (local.get $x) (local.get $fn)))

  ;; Exports
  (export "apply_unary" (func $apply_unary))
  (export "apply_binary" (func $apply_binary))
  (export "apply_unary_linear" (func $apply_unary_linear))
  (export "apply_binary_linear" (func $apply_binary_linear))
  (export "tail_apply" (func $tail_apply))
  (export "double_then_add" (func $double_then_add)))
