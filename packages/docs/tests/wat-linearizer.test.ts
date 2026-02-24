import { describe, it, expect } from 'vitest';
import { foldedToLinear } from '../src/lib/wat-linearizer';

describe('foldedToLinear', () => {
  describe('simple instructions', () => {
    it('linearizes a simple binary op', () => {
      const input = `(module
  (func (result i32)
    (i32.add (i32.const 1) (i32.const 2)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (result i32)
    i32.const 1
    i32.const 2
    i32.add
  )
)`);
    });

    it('linearizes nested binary ops', () => {
      const input = `(module
  (func (param $a i32) (param $b i32) (result i32)
    (i32.add (i32.mul (local.get $a) (local.get $b)) (i32.const 1)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.mul
    i32.const 1
    i32.add
  )
)`);
    });
  });

  describe('block', () => {
    it('linearizes a block with label', () => {
      const input = `(module
  (func (param $n i32) (result i32)
    (block $out
      (br_if $out (i32.eqz (local.get $n)))
      (return (i32.const 42)))
    (i32.const 0))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (param $n i32) (result i32)
    block $out
      local.get $n
      i32.eqz
      br_if $out
      i32.const 42
      return
    end $out
    i32.const 0
  )
)`);
    });

    it('linearizes a block with result type', () => {
      const input = `(module
  (func (result i32)
    (block $b (result i32)
      (i32.const 42)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (result i32)
    block $b (result i32)
      i32.const 42
    end $b
  )
)`);
    });
  });

  describe('loop', () => {
    it('linearizes a loop with label', () => {
      const input = `(module
  (func (param $n i32) (result i32)
    (local $acc i32)
    (block $done
      (loop $again
        (br_if $done (i32.eqz (local.get $n)))
        (local.set $acc (i32.add (local.get $acc) (i32.const 1)))
        (local.set $n (i32.sub (local.get $n) (i32.const 1)))
        (br $again)))
    (local.get $acc))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (param $n i32) (result i32)
    (local $acc i32)
    block $done
      loop $again
        local.get $n
        i32.eqz
        br_if $done
        local.get $acc
        i32.const 1
        i32.add
        local.set $acc
        local.get $n
        i32.const 1
        i32.sub
        local.set $n
        br $again
      end $again
    end $done
    local.get $acc
  )
)`);
    });
  });

  describe('if/else', () => {
    it('linearizes if with result type and else', () => {
      const input = `(module
  (func (param $x i32) (result i32)
    (if (result i32) (i32.eqz (local.get $x))
      (then (i32.const 1))
      (else (i32.const 0))))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (param $x i32) (result i32)
    local.get $x
    i32.eqz
    if (result i32)
      i32.const 1
    else
      i32.const 0
    end
  )
)`);
    });

    it('linearizes if with label', () => {
      const input = `(module
  (func (param $x i32) (result i32)
    (if $lbl (result i32) (local.get $x)
      (then (i32.const 1))
      (else (i32.const 0))))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (param $x i32) (result i32)
    local.get $x
    if $lbl (result i32)
      i32.const 1
    else
      i32.const 0
    end $lbl
  )
)`);
    });
  });

  describe('instructions with immediates', () => {
    it('preserves offset and align on memory ops', () => {
      const input = `(module
  (memory 1)
  (func (param $addr i32) (param $val i32)
    (i32.store offset=4 align=4 (local.get $addr) (local.get $val)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (memory 1)
  (func (param $addr i32) (param $val i32)
    local.get $addr
    local.get $val
    i32.store offset=4 align=4
  )
)`);
    });
  });

  describe('call and call_indirect', () => {
    it('linearizes function calls', () => {
      const input = `(module
  (func $double (param $x i32) (result i32)
    (i32.mul (local.get $x) (i32.const 2)))
  (func (export "main") (param $n i32) (result i32)
    (call $double (local.get $n)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func $double (param $x i32) (result i32)
    local.get $x
    i32.const 2
    i32.mul
  )
  (func (export "main") (param $n i32) (result i32)
    local.get $n
    call $double
  )
)`);
    });
  });

  describe('already-linear code', () => {
    it('passes through linear code unchanged', () => {
      const input = `(module
  (func (result i32)
    i32.const 1
    i32.const 2
    i32.add)
)`;
      // No folded instructions — should preserve as-is
      const output = foldedToLinear(input);
      // The func body has no nested S-expressions, so it goes through exprToString
      expect(output).toBe(`(module
  (func (result i32) i32.const 1 i32.const 2 i32.add)
)`);
    });
  });

  describe('comments', () => {
    it('preserves line comments', () => {
      const input = `(module
  (func (result i32)
    ;; add two numbers
    (i32.add (i32.const 1) (i32.const 2)))
)`;
      const output = foldedToLinear(input);
      expect(output).toContain(';; add two numbers');
      expect(output).toContain('i32.const 1');
      expect(output).toContain('i32.add');
    });
  });

  describe('module structure preservation', () => {
    it('preserves non-func module elements', () => {
      const input = `(module
  (type $t (func (param i32) (result i32)))
  (memory 1)
  (global $g (mut i32) (i32.const 0))
  (func (param i32) (result i32)
    (i32.add (local.get 0) (i32.const 1)))
)`;
      const output = foldedToLinear(input);
      expect(output).toContain('(type $t (func (param i32) (result i32)))');
      expect(output).toContain('(memory 1)');
      expect(output).toContain('(global $g (mut i32) (i32.const 0))');
      expect(output).toContain('local.get 0');
      expect(output).toContain('i32.const 1');
      expect(output).toContain('i32.add');
    });

    it('preserves inline exports', () => {
      const input = `(module
  (func (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b)))
)`;
      const output = foldedToLinear(input);
      expect(output).toContain('(func (export "add") (param $a i32) (param $b i32) (result i32)');
    });
  });

  describe('select', () => {
    it('linearizes select with nested args', () => {
      const input = `(module
  (func (param $a i32) (param $b i32) (param $c i32) (result i32)
    (select (local.get $a) (local.get $b) (local.get $c)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (param $a i32) (param $b i32) (param $c i32) (result i32)
    local.get $a
    local.get $b
    local.get $c
    select
  )
)`);
    });
  });

  describe('drop', () => {
    it('linearizes drop', () => {
      const input = `(module
  (func (param $x i32)
    (drop (local.get $x)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (param $x i32)
    local.get $x
    drop
  )
)`);
    });
  });

  describe('deeply nested', () => {
    it('handles triple nesting', () => {
      const input = `(module
  (func (result i32)
    (i32.add
      (i32.mul
        (i32.sub (i32.const 10) (i32.const 3))
        (i32.const 2))
      (i32.const 1)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func (result i32)
    i32.const 10
    i32.const 3
    i32.sub
    i32.const 2
    i32.mul
    i32.const 1
    i32.add
  )
)`);
    });
  });

  describe('try_table', () => {
    it('linearizes try_table as a block', () => {
      const input = `(module
  (tag $e (param i32))
  (func (result i32)
    (block $handler (result i32)
      (try_table (result i32) (catch $e $handler)
        (i32.const 42)
        (throw $e (i32.const 1)))
      (i32.const 0)))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (tag $e (param i32))
  (func (result i32)
    block $handler (result i32)
      try_table (result i32) (catch $e $handler)
        i32.const 42
        i32.const 1
        throw $e
      end
      i32.const 0
    end $handler
  )
)`);
    });

    it('linearizes try_table with catch_all', () => {
      const input = `(module
  (func $may_throw)
  (func
    (block $fallback
      (try_table (catch_all $fallback)
        (call $may_throw))))
)`;
      const output = foldedToLinear(input);
      expect(output).toBe(`(module
  (func $may_throw)
  (func
    block $fallback
      try_table (catch_all $fallback)
        call $may_throw
      end
    end $fallback
  )
)`);
    });
  });

  describe('if with comments between branches', () => {
    it('preserves else when comment separates then and else', () => {
      const input = `(module
  (func (param $a i32) (param $b i32) (result i32)
    (if (result i32) (i32.eqz (local.get $b))
      (then (i32.const 0)) ;; zero case
      (else (i32.div_s (local.get $a) (local.get $b)))))
)`;
      const output = foldedToLinear(input);
      expect(output).toContain('else');
      expect(output).toContain('i32.div_s');
    });
  });

  describe('line comments in non-folded code', () => {
    it('preserves code after line comments', () => {
      const input = `(module
  (func (param $a i32) (param $b i32) (result i32)
    (i32.add
      (local.get $a) ;; first operand
      (local.get $b)))
)`;
      const output = foldedToLinear(input);
      expect(output).toContain('local.get $a');
      expect(output).toContain('local.get $b');
      expect(output).toContain('i32.add');
    });
  });
});
