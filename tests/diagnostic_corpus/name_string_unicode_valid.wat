;; Valid Unicode in WAT *name strings* (quoted export/import names) must not
;; produce false diagnostics on the browser-reachable path (tree-sitter syntax
;; diagnostics + semantic diagnostics). Unlike identifiers ($...), which are
;; restricted to ASCII idchars, name strings are UTF-8 and may contain arbitrary
;; valid Unicode, including multi-byte characters, combining marks, and escapes.
;;
;; Regression test: pins that neither the syntax pass nor the semantic pass flags
;; valid Unicode name strings. wasm-tools `validate --features all` accepts this
;; module.
(module
  ;; Unicode in both the import module and field names (imports come first)
  (import "wåsî" "función_🚀" (func))

  ;; multi-byte characters directly in the name string
  (func (export "café_ñ_🎉_日本語"))

  ;; \u{...} escapes in a name string
  (func (export "\u{1F389}\u{00e9}"))

  ;; Unicode export on a global
  (global (export "π_value") i32 (i32.const 0)))
