;; Invalid: global.get of mutable global in constant expression
;; Constant expressions may only reference immutable imported globals

(module
  (global $g (mut i32) (i32.const 0))

  ;; Error: global.get of mutable global in const expr
  (global $h i32 (global.get $g)))
