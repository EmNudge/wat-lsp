;; call_ref / return_call_ref with (type $t) annotation
;; The Typed Function References proposal allows both forms:
;;   call_ref $type         - bare index
;;   call_ref (type $type)  - type annotation
;; Both are equivalent. The annotation form mirrors call_indirect (type $t).

(module
  (type $unary (func (param i32) (result i32)))
  (type $binary (func (param i32 i32) (result i32)))

  ;; --- Basic functions ---

  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))

  (func $add (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))

  ;; --- call_ref with (type ...) annotation (s-expression style) ---

  (func $apply_unary (param $fn (ref $unary)) (param $x i32) (result i32)
    (call_ref (type $unary) (local.get $x) (local.get $fn)))

  (func $apply_binary (param $fn (ref $binary)) (param $a i32) (param $b i32) (result i32)
    (call_ref (type $binary) (local.get $a) (local.get $b) (local.get $fn)))

  ;; --- call_ref with (type ...) annotation (linear/stack style) ---

  (func $apply_unary_linear (param $fn (ref $unary)) (param $x i32) (result i32)
    local.get $x
    local.get $fn
    call_ref (type $unary))

  (func $apply_binary_linear (param $fn (ref $binary)) (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    local.get $fn
    call_ref (type $binary))

  ;; --- return_call_ref with (type ...) annotation (tail call) ---

  (func $tail_apply (param $fn (ref $unary)) (param $x i32) (result i32)
    local.get $x
    local.get $fn
    return_call_ref (type $unary))

  ;; --- Mixing both forms in the same module ---

  (func $double_then_add (param $x i32) (param $y i32) (result i32)
    ;; bare form producing i32, then annotation form consuming it
    (call_ref (type $binary)
      (call_ref $unary (local.get $x) (ref.func $double))
      (local.get $y)
      (ref.func $add)))

  ;; --- Numeric index works with annotation form ---

  (func $call_by_index (param $fn (ref $unary)) (param $x i32) (result i32)
    (call_ref (type 0) (local.get $x) (local.get $fn)))

  ;; Exports
  (export "apply_unary" (func $apply_unary))
  (export "apply_binary" (func $apply_binary))
  (export "apply_unary_linear" (func $apply_unary_linear))
  (export "apply_binary_linear" (func $apply_binary_linear))
  (export "tail_apply" (func $tail_apply))
  (export "double_then_add" (func $double_then_add)))
