;; Imported functions may declare a full typeuse: an optional (type idx)
;; reference plus explicit params/results, or params/results alone.
;; Regression test: these previously produced syntax ERROR nodes.
(module
  (type $binop (func (param i32 i32) (result i32)))
  (import "env" "add" (func $add (type $binop) (param i32 i32) (result i32)))
  (import "env" "sub" (func $sub (param i32 i32) (result i32)))
  (import "env" "id"  (func $id (type $binop)))
  (func (result i32)
    i32.const 1
    i32.const 2
    call $add))
