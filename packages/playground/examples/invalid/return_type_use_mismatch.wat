;; Return type mismatch when using (type $t) syntax
;; Functions that reference a type definition via (type $t) instead of inline
;; (result ...) clauses are now validated against the referenced type's signature.
;;
;; Expected error: Type mismatch: function returns i32 but declared as i64

(module
  ;; Define a function type that takes no params and returns i64
  (type $returns_i64 (func (result i64)))

  ;; This function references the type but returns the wrong type
  (func $wrong_return (type $returns_i64)
    i32.const 42  ;; ERROR: returns i32 but type $returns_i64 declares i64
  )

  ;; For comparison: this function correctly satisfies the type
  (func $correct_return (type $returns_i64)
    i64.const 42  ;; OK: returns i64 as declared
  )

  ;; Also works with numeric type indices
  (type $binary_op (func (param i32 i32) (result i32)))

  (func $bad_binary (type $binary_op)
    i64.const 1  ;; ERROR: returns i64 but type $binary_op declares i32
  )
)
