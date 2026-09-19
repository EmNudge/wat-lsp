;; Paired negative-case fixtures for the wast-runner conformance gate.
;;
;; These pin the negative-case *scoring* contract that issue #292 calls out:
;; `assert_invalid` and `assert_malformed` must not both "pass on any error".
;; A validation failure and a grammar/parse failure are different signals.

;; --- assert_invalid ---------------------------------------------------------

;; Genuine validation failure: the module parses fine but fails type checking
;; (result declared i32, body pushes i64). This is exactly what assert_invalid
;; expects, so it must PASS.
(assert_invalid
  (module (func (result i32) (i64.const 0)))
  "type mismatch")

;; A perfectly valid module wrapped in assert_invalid: our pipeline reports no
;; error at all, so the expected validation failure did not occur. This must
;; FAIL (it would be a false negative to pass it).
(assert_invalid
  (module (func (result i32) (i32.const 0)))
  "type mismatch")

;; --- assert_malformed -------------------------------------------------------

;; Malformed quoted text (unterminated module). A parse/lex-level rejection
;; satisfies assert_malformed, so it must PASS.
(assert_malformed
  (module quote "(func (result i32)")
  "unexpected end")
