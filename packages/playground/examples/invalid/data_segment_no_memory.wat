;; Invalid: active data segment without a memory
;; An active data segment requires at least one memory to be defined

(module
  (func (export "test"))

  ;; Error: unknown memory 0
  (data (offset (i32.const 0)) "hello"))
