# WAST Spec Test Coverage Plan

**Generated**: 2026-02-20
**Baseline**: commit 488fa6b4
**Updated**: 2026-02-20 (Phase 1 complete, re-analyzed at commit 18da9a8c)

## Current Results

| Category | Pass | Total | Rate | Delta from baseline |
|---|---|---|---|---|
| Modules (valid) | 2092 | 2128 | 98.3% | +0 |
| assert_invalid | 2667 | 2689 | 99.2% | **+17** |
| assert_malformed | 1229 | 1229 | 100.0% | **+1** |
| **TOTAL (non-skip)** | **5988** | **6046** | **99.0%** | **+18** |
| Skipped (runtime) | 57766 | — | — | — |
| Skipped (binary/linking) | 810 | — | — | — |

**58 total failures** across 21 files (down from 76 across 25 at baseline).

### Phase 1 Changes (completed)
- **array.copy/fill/init_data/init_elem**: Element type validation (+7 assert_invalid)
- **br_on_cast/br_on_cast_fail**: Cast type hierarchy validation (+4 assert_invalid)
- **try_table catch clauses**: Handler type validation against target labels (+4 assert_invalid)
- **throw_ref**: Stack operand validation (+2 assert_invalid)
- **struct duplicate fields**: Duplicate field name detection (+1 assert_malformed)

## Baseline (pre-Phase 1)

| Category | Pass | Total | Rate |
|---|---|---|---|
| Modules (valid) | 2092 | 2128 | 98.3% |
| assert_invalid | 2650 | 2689 | 98.5% |
| assert_malformed | 1228 | 1229 | 99.9% |
| **TOTAL (non-skip)** | **5970** | **6046** | **98.7%** |

## Per-File Failures (58 remaining)

| File | Mod | Inv | Total | Root Cause |
|---|---|---|---|---|
| type-subtyping.wast | 11 | 7 | **18** | Subtype variance |
| type-rec.wast | 0 | 9 | **9** | Rec type groups |
| array.wast | 4 | 0 | **4** | Structref placeholder |
| br_on_cast_fail.wast | 3 | 0 | **3** | Cast stack effects |
| br_on_cast.wast | 3 | 0 | **3** | Cast stack effects |
| return_call_ref.wast | 1 | 2 | **3** | Ref type resolution |
| instance.wast | 2 | 0 | **2** | Definition syntax |
| table.wast | 1 | 1 | **2** | Definition + structref |
| try_table.wast | 1 | 1 | **2** | Import aliasing + type |
| annotations.wast | 1 | 0 | **1** | Grammar |
| array_copy.wast | 0 | 1 | **1** | Array type matching |
| call_indirect64.wast | 1 | 0 | **1** | Table64 index type |
| i31.wast | 1 | 0 | **1** | i31ref type |
| id.wast | 1 | 0 | **1** | Quoted identifiers |
| memory.wast | 1 | 0 | **1** | Definition syntax |
| memory64.wast | 1 | 0 | **1** | Definition syntax |
| ref_as_non_null.wast | 0 | 1 | **1** | Ref type narrowing |
| stack.wast | 1 | 0 | **1** | Flat call_indirect |
| struct.wast | 0 | 1 | **1** | GC field type |
| table-sub.wast | 1 | 0 | **1** | Ref subtyping |
| table64.wast | 1 | 0 | **1** | Definition syntax |

## Failure Root Causes (58 total)

| Root Cause | Mod | Inv | Total | Description |
|---|---|---|---|---|
| Function subtype variance | 11 | 7 | **18** | `module_checks.rs` requires exact match; spec allows covariant returns + contravariant params |
| Rec type groups | 0 | 9 | **9** | No rec group modeling; can't validate type equivalence or forward refs within rec |
| GC concrete ref resolution | 8 | 0 | **8** | All concrete GC types map to Structref placeholder — loses arrayref/funcref/i31ref distinction |
| br_on_cast/fail stack | 6 | 0 | **6** | Type checker doesn't model type narrowing after cast; false stack underflow |
| Module definition syntax | 6 | 0 | **6** | `(module definition ...)` wast syntax not supported by grammar |
| GC type checking gaps | 0 | 6 | **6** | Missing: array type match, ref narrowing, return_call_ref return types, struct fields |
| Miscellaneous | 5 | 0 | **5** | Annotations grammar, call_indirect64, quoted IDs, flat call_indirect, imported tag aliasing |

## Prioritized Phases

### Phase 2: Function Subtype Variance (~18 fixes)
**Feasibility**: Medium | **Impact**: Highest | **Files**: `module_checks.rs`

Root cause: The subtype check in `module_checks.rs` emits "Function subtype signature must match parent exactly" — it requires identical signatures. The spec allows:
- **Covariant** return types (subtype may return a subtype of parent's return)
- **Contravariant** param types (subtype may accept a supertype of parent's param)

**Module fixes (11)**: type-subtyping.wast lines 177, 188, 229, 422, 438, 486, 497, 652, 659, 668, 677
- All fail with "Function subtype signature must match parent exactly"
- Fix: Replace exact-match with `is_subtype(sub_return, parent_return)` and `is_subtype(parent_param, sub_param)`

**Invalid fixes (7)**: type-subtyping.wast lines 139, 205, 215, 726, 734, 851, 883
- "no errors (expected type mismatch/sub type)" — need to validate variance violations
- Fix: Detect when subtype attempts *invalid* variance (contravariant returns, covariant params, `sub final` violations)

**Prerequisite**: Need `is_subtype(a, b)` for ValueType/ref types. Partial support exists (type_check.rs has `is_subtype_of` for stack checking); needs extension to handle concrete type indices.

### Phase 3: GC Concrete Type Resolution (~14 fixes)
**Feasibility**: Hard | **Impact**: High | **Files**: `parser.rs`, `semantic.rs`, `type_check.rs`

Root cause: The parser maps all concrete GC types (`ref $my_array`, `ref $my_struct`, etc.) to `ValueType::Structref` as a placeholder. This means the type checker can't distinguish arrayref from structref from funcref at concrete type boundaries.

**Module fixes (8)**:
- array.wast: 4 — "expected arrayref, found structref" (concrete array type → should be arrayref)
- table.wast:93 — "expected funcref, found structref" (concrete func type used as funcref)
- table-sub.wast:1 — type mismatch on ref subtype table copy
- return_call_ref.wast:213 — "expected (ref null 2), found (ref 1)" (concrete type refs)
- i31.wast:128 — i31ref table init expression

**Invalid fixes (6)**:
- array_copy.wast:41 — "array types do not match" (needs concrete type tracking)
- ref_as_non_null.wast:31, struct.wast:58, try_table.wast:470, return_call_ref.wast:231,286 — various type mismatches requiring concrete GC type knowledge

**Approach**: Add `ValueType::Ref(TypeIndex)` variant or enhance the symbol table to track concrete type kinds (struct/array/func) so subtype checks can query the structural type.

### Phase 4: Rec Type Groups (~9 fixes)
**Feasibility**: Hard | **Impact**: Medium | **Files**: `parser.rs`, `module_checks.rs`

Root cause: No modeling of `(rec ...)` type groups. The spec requires:
- Types within the same rec group can forward-reference each other
- Type equivalence is defined per rec group (iso-recursive)
- Rec group boundaries affect type identity

**Fixes (9 invalid)**: type-rec.wast lines 28, 51, 59, 93, 103, 114, 124, 204, 216
- Line 28: "expected unknown type" — rec group forward ref should be invalid when referencing beyond group
- Lines 51+: "expected type mismatch" — rec type equivalence violations

**Approach**: Track rec group membership in symbol table; add rec group boundary checks to type validation.

### Phase 5: br_on_cast/fail Stack Modeling (~6 fixes)
**Feasibility**: Medium | **Impact**: Medium | **Files**: `semantic.rs`, `type_check.rs`

Root cause: `br_on_cast` and `br_on_cast_fail` have complex stack effects — they narrow the type on the stack based on the cast result. The current type checker doesn't model this, causing false "Stack underflow" and "type mismatch" errors.

**Fixes (6 modules)**:
- br_on_cast.wast: 3 (lines 3, 104, 211)
- br_on_cast_fail.wast: 3 (lines 3, 104, 226)

**Approach**: When processing `br_on_cast`, push the *difference type* (non-cast result) back onto the stack after the branch. Similarly for `br_on_cast_fail` (push the cast result type). Requires understanding the input type and cast target type.

### Phase 6: Module Definition Syntax (~6 fixes)
**Feasibility**: Easy | **Impact**: Low | **Files**: `wast-runner` or `grammar.js`

Root cause: `(module definition ...)` is a WAST-only syntax for defining module instances. Not valid WAT — only appears in test harness contexts.

**Fixes (6 modules)**:
- instance.wast: 2 (lines 3, 109)
- memory.wast:8, memory64.wast:8, table.wast:9, table64.wast:9

**Approach**: Have wast-runner skip or handle `(module definition ...)` directives gracefully.

### Phase 7: Miscellaneous (~5 fixes)
**Feasibility**: Easy-Medium | **Impact**: Low

| Fix | File | Issue |
|---|---|---|
| annotations.wast:1 | `grammar.js` | Annotation with special chars causes parse error at line 15 |
| call_indirect64.wast:3 | `semantic.rs` | `table i64 funcref` — call_indirect with i64 table index |
| id.wast:1 | `parser.rs` | `$"fh"` quoted identifier should normalize to `$fh` |
| stack.wast:156 | `semantic.rs` | Flat (non-folded) call_indirect inside block — stack tracking |
| try_table.wast:10 | `parser.rs` | Imported tag/func aliases not resolved across registered modules |

## Projected Pass Rates

| Phase | Pass | Total | Rate | Delta | Cumulative |
|---|---|---|---|---|---|
| Current (Phase 1 done) | 5988 | 6046 | 99.04% | — | — |
| After Phase 2 | ~6006 | 6046 | ~99.3% | +18 | +18 |
| After Phase 3 | ~6020 | 6046 | ~99.6% | +14 | +32 |
| After Phase 4 | ~6029 | 6046 | ~99.7% | +9 | +41 |
| After Phase 5 | ~6035 | 6046 | ~99.8% | +6 | +47 |
| After Phase 6 | ~6041 | 6046 | ~99.9% | +6 | +53 |
| After Phase 7 | ~6046 | 6046 | ~100% | +5 | +58 |

## Recommended Next Step

**Start with Phase 2** (function subtype variance). It has the best effort-to-fix ratio:
- 18 fixes (11 modules + 7 invalid) from a single file (`module_checks.rs`)
- The existing "Function subtype signature must match parent exactly" check just needs covariant/contravariant awareness
- No new data structures needed — just enhance the comparison logic
- `is_subtype_of` already partially exists in `type_check.rs` — extend for ref types

**Alternative quick wins**: Phase 6 (module definition syntax, 6 easy fixes) and Phase 7 (miscellaneous, 5 varied fixes) are simpler but lower impact.
