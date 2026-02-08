;; Invalid: undefined label reference
;; The label $nowhere is never defined

(module
  (func (export "test")
    (block $here
      (br $nowhere))))
