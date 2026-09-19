;; ref.null extern produces externref, which is unrelated to funcref.
(module
  (func (result funcref)
    ref.null extern))
