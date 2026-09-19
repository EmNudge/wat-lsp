;; A valid module and an assert_malformed the diagnostic pipeline should catch.
(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add))
(assert_malformed
  (module quote "(func (result i32) i32.add)")
  "type mismatch")
