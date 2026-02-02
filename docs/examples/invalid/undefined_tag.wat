;; Invalid: undefined tag reference in catch clause
;; The tag $missing is never defined

(module
  (func (export "test")
    (try
      (do (nop))
      (catch $missing (drop)))))
