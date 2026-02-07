---
title: 'Subtyping'
description: 'The WebAssembly type hierarchy and subtyping rules.'
---

The GC proposal introduces a type hierarchy that enables runtime casts, polymorphism, and safe downcasting. Subtyping determines when one type can be used where another is expected.

## Heap type hierarchy

Every heap type fits into one of four independent hierarchies:

- `any` > `eq` > `i31`, `struct`, `array` > `none`
- `func` > `nofunc`
- `exn` > `noexn`
- `extern` > `noextern`

The bottom types (`none`, `nofunc`, `noexn`, `noextern`) are uninhabited -- no value has these types. They exist so that `ref.null none` is a valid null reference assignable to any nullable reference in the `any` hierarchy.

```wat
(module
  ;; A parameter accepting any eq-compatible reference
  (func $identity (param $v (ref null eq)) (result (ref null eq))
    (local.get $v))
)
```

## Reference type matching

A non-nullable reference `(ref $t)` is a subtype of its nullable counterpart `(ref null $t)`, but not vice versa. This means you can pass a non-null reference where a nullable one is expected, but you cannot pass a nullable reference where a non-null one is required.

```wat
(module
  (type $point (struct (field $x f32) (field $y f32)))

  ;; Accepts non-nullable -- callers must provide a live reference
  (func $use_point (param $p (ref $point))
    (drop (struct.get $point $x (local.get $p))))

  ;; Accepts nullable -- callers may pass ref.null
  (func $maybe_point (param $p (ref null $point))
    (drop (ref.is_null (local.get $p))))
)
```

A call to `$use_point` satisfies the stricter contract. A `(ref $point)` value can also be passed to `$maybe_point` because `(ref $point)` is a subtype of `(ref null $point)`.

## Struct subtyping

A struct subtype must repeat all parent fields (in order and with compatible types) and may append new ones. This is called width subtyping. Use the `sub` keyword to declare a subtype relationship.

```wat
(module
  (type $shape (sub (struct
    (field $x f32)
    (field $y f32))))

  (type $circle (sub $shape (struct
    (field $x f32)
    (field $y f32)
    (field $radius f32))))

  (type $rect (sub $shape (struct
    (field $x f32)
    (field $y f32)
    (field $w f32)
    (field $h f32))))
)
```

A `(ref $circle)` can be used wherever `(ref $shape)` is expected. The extra `$radius` field is simply invisible to code that only knows about `$shape`.

## Field variance

Immutable fields are **covariant**: if `$sub` is a subtype of `$super`, then an immutable field of type `(ref $sub)` is compatible with one of type `(ref $super)`.

Mutable fields are **invariant**: the field type must match exactly, because the field can be both read and written.

```wat-snippet
(type $animal (sub (struct
  (field $name (ref $string)))))       ;; immutable -- covariant

(type $pet (sub $animal (struct
  (field $name (ref $nonempty_str))))) ;; OK: $nonempty_str <: $string

(type $cell (sub (struct
  (field $value (mut (ref $animal)))))) ;; mutable -- invariant

;; Invalid: cannot narrow a mutable field
;; (type $pet_cell (sub $cell (struct
;;   (field $value (mut (ref $pet))))))  ;; NOT allowed
```

## The `final` keyword

By default, types declared with `sub` are open for further subtyping. Adding `final` seals a type so no other type can extend it.

```wat
(module
  (type $base (sub (struct
    (field $id i32))))

  (type $leaf (sub final $base (struct
    (field $id i32)
    (field $data f64))))

  ;; Error: $leaf is final, cannot extend
  ;; (type $invalid (sub $leaf (struct
  ;;   (field $id i32)
  ;;   (field $data f64)
  ;;   (field $extra i32))))
)
```

Types not wrapped in `sub` at all are implicitly `sub final` -- they cannot be extended.

## Instruction Reference

- [GC Types](/instructions/gc-types) -- `sub`, `final`, `rec`, `struct`, `array`, `field`, `ref`, `null`
- [GC Casts](/instructions/gc-casts) -- `ref.test`, `ref.cast`, `br_on_cast`, `ref.eq`
