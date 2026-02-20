# WAST Spec Test Coverage Plan

**Generated**: 2026-02-20
**Baseline**: commit 488fa6b4
**Updated**: 2026-02-20 (Phase 1 complete)

## Current Results (after Phase 1)

| Category | Pass | Total | Rate | Delta |
|---|---|---|---|---|
| Modules (valid) | 2092 | 2128 | 98.3% | +0 |
| assert_invalid | 2667 | 2689 | 99.2% | **+17** |
| assert_malformed | 1229 | 1229 | 100.0% | **+1** |
| **TOTAL (non-skip)** | **5988** | **6046** | **99.0%** | **+18** |
| Skipped (runtime) | 58577 | — | — | — |

**58 total failures** across 21 files (was 76 across 25).

### Phase 1 Changes (completed)
- **array.copy/fill/init_data/init_elem**: Element type validation (+7 assert_invalid)
- **br_on_cast/br_on_cast_fail**: Cast type hierarchy validation (+4 assert_invalid)
- **try_table catch clauses**: Handler type validation against target labels (+4 assert_invalid)
- **throw_ref**: Stack operand validation (+2 assert_invalid)
- **struct duplicate fields**: Duplicate field name detection (+1 assert_malformed)

## Previous Baseline

| Category | Pass | Total | Rate |
|---|---|---|---|
| Modules (valid) | 2092 | 2128 | 98.3% |
| assert_invalid | 2650 | 2689 | 98.5% |
| assert_malformed | 1228 | 1229 | 99.9% |
| **TOTAL (non-skip)** | **5970** | **6046** | **98.7%** |

**76 total failures** across 25 files.

## Per-File Failures

| File | Mod | Inv | Mal | Total |
|---|---|---|---|---|
| type-subtyping.wast | 11 | 7 | 0 | 18 |
| type-rec.wast | 0 | 9 | 0 | 9 |
| try_table.wast | 1 | 5 | 0 | 6 |
| br_on_cast_fail.wast | 3 | 2 | 0 | 5 |
| br_on_cast.wast | 3 | 2 | 0 | 5 |
| array.wast | 4 | 0 | 0 | 4 |
| return_call_ref.wast | 1 | 2 | 0 | 3 |
| array_copy.wast | 0 | 3 | 0 | 3 |
| instance.wast | 2 | 0 | 0 | 2 |
| table.wast | 2 | 0 | 0 | 2 |
| array_fill.wast | 0 | 2 | 0 | 2 |
| array_init_elem.wast | 0 | 2 | 0 | 2 |
| struct.wast | 0 | 1 | 1 | 2 |
| throw_ref.wast | 0 | 2 | 0 | 2 |
| annotations.wast | 1 | 0 | 0 | 1 |
| call_indirect64.wast | 1 | 0 | 0 | 1 |
| i31.wast | 1 | 0 | 0 | 1 |
| id.wast | 1 | 0 | 0 | 1 |
| memory.wast | 1 | 0 | 0 | 1 |
| memory64.wast | 1 | 0 | 0 | 1 |
| ref_as_non_null.wast | 0 | 1 | 0 | 1 |
| stack.wast | 1 | 0 | 0 | 1 |
| table-sub.wast | 1 | 0 | 0 | 1 |
| table64.wast | 1 | 0 | 0 | 1 |
| array_init_data.wast | 0 | 1 | 0 | 1 |

## Failure Root Causes

| Root Cause | Mod | Inv | Mal | Total | Description |
|---|---|---|---|---|---|
| GC rec types & subtyping | 11 | 16 | 0 | **27** | Recursive type groups, type equivalence across rec boundaries, variance violations |
| GC instruction validation | 6 | 10 | 0 | **16** | br_on_cast/fail nullability, array ops mutability/types, try_table catch types |
| Module definition syntax | 5 | 0 | 0 | **5** | `(module definition ...)` wast syntax (instance.wast, memory.wast, table.wast) |
| Memory64/Table64 edge cases | 3 | 0 | 0 | **3** | 64-bit index types in memory.size, table ops, call_indirect64 |
| Misc single-file issues | 11 | 13 | 1 | **25** | Quoted IDs, annotations, multi-value stack, throw_ref, struct field dup, ref_as_non_null |

## Prioritized Phases

### Phase 1: GC Instruction Validation (~16 fixes)
**Feasibility**: Medium | **Impact**: High

Root cause: The type checker doesn't understand GC-specific instructions well enough.

**Subphase 1a: Array operation type validation (+8)**
- `array_copy.wast`: 3 invalid — need to check array element type compatibility and mutability for `array.copy`
- `array_fill.wast`: 2 invalid — need to check `array.fill` element type (packed i8/i16 → i32 widening)
- `array_init_data.wast`: 1 invalid — need to verify array type is numeric/vector for `array.init_data`
- `array_init_elem.wast`: 2 invalid — need element type matching for `array.init_elem`
- **Files**: `semantic.rs`, `type_check.rs`

**Subphase 1b: br_on_cast / br_on_cast_fail validation (+4)**
- 2 invalid each for br_on_cast and br_on_cast_fail — nullability constraint on cast types
- **Files**: `semantic.rs`, `type_check.rs`

**Subphase 1c: try_table catch type validation (+5)**
- try_table.wast: 5 invalid — catch/catch_ref handler type mismatches
- **Files**: `semantic.rs`, `type_check.rs`

**Subphase 1d: Miscellaneous GC type checking (+4)**
- return_call_ref.wast: 2 invalid — return type subtyping
- throw_ref.wast: 2 invalid — exnref type on stack
- ref_as_non_null.wast: 1 invalid — nullable→non-nullable cast type narrowing
- struct.wast: 1 invalid — struct field type mismatch
- **Files**: `semantic.rs`, `type_check.rs`

### Phase 2: GC Rec Types & Subtyping (~27 fixes)
**Feasibility**: Hard | **Impact**: Highest

Root cause: The parser/type system doesn't model recursive type groups (`rec`), so type equivalence across rec boundaries and subtype variance cannot be validated.

- type-subtyping.wast: 18 failures — need proper rec type group handling, covariant/contravariant field checking
- type-rec.wast: 9 failures — need `rec` group type equivalence, forward references within rec groups
- **Files**: `parser.rs` (type representation), `module_checks.rs` (subtype validation), `type_check.rs` (type matching)
- **Prerequisite**: Need a proper GC type representation in the symbol table that tracks:
  - Rec group membership and boundaries
  - Structural type definitions (struct fields, array element, func params/results)
  - Subtype declarations (`sub` / `sub final`)
  - Type equivalence rules for iso-recursive types

### Phase 3: Module False Positives (~14 module fixes)
**Feasibility**: Medium | **Impact**: Medium

**Subphase 3a: GC module validity (+10)**
- br_on_cast.wast: 3 module — false positive errors on valid br_on_cast usage
- br_on_cast_fail.wast: 3 module — same for br_on_cast_fail
- array.wast: 4 module — false positive errors on valid array operations (rec types, GC refs)
- **Root cause**: Lack of rec type awareness causes spurious "unknown type" or "type mismatch" errors
- **Files**: `references.rs`, `module_checks.rs`, `semantic.rs`

**Subphase 3b: Memory/Table/Instance module syntax (+5)**
- instance.wast: 2 — `(module definition ...)` wast syntax
- memory.wast: 1 — same
- table.wast: 2 — same
- memory64.wast: 1 — 64-bit index type for memory.size
- table64.wast: 1 — 64-bit index type for table operations
- call_indirect64.wast: 1 — 64-bit call_indirect
- **Files**: `wast-runner` (module definition parsing), `semantic.rs` (64-bit index types)

**Subphase 3c: Misc module fixes (+5)**
- annotations.wast: 1 — quoted annotation names `(@"name")`
- id.wast: 1 — quoted identifier normalization `$"fh"` ≡ `$fh`
- i31.wast: 1 — i31ref table init expression
- stack.wast: 1 — multi-value block stack validation
- table-sub.wast: 1 — table copy with subtype refs
- **Files**: `grammar.js`, `parser.rs`, `semantic.rs`

### Phase 4: Structural Validation (+2)
**Feasibility**: Easy | **Impact**: Low

- struct.wast: 1 malformed — duplicate field detection in struct types
- return_call_ref.wast: 1 module — return type subtyping validation
- **Files**: `module_checks.rs`, `semantic.rs`

## Projected Pass Rates

| Phase | Pass | Total | Rate | Delta |
|---|---|---|---|---|
| Baseline | 5970 | 6046 | 98.7% | — |
| After Phase 1 | ~5986 | 6046 | ~99.0% | +16 |
| After Phase 2 | ~6013 | 6046 | ~99.5% | +27 |
| After Phase 3 | ~6027 | 6046 | ~99.7% | +14 |
| After Phase 4 | ~6029 | 6046 | ~99.7% | +2 |

**Remaining ~17 failures** after all phases would be primarily `(module definition)` wast syntax (not WAT) and edge cases requiring full iso-recursive type system.

## Recommended Next Step

**Start with Phase 1a** (array operation type validation). It has the best effort-to-fix ratio:
- 8 assert_invalid fixes from adding type checks for 4 array instructions
- Self-contained changes in `semantic.rs` / `type_check.rs`
- No prerequisite infrastructure changes needed
