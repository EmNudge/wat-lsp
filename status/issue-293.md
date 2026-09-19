# Issue #293 — audit-293c (mixed-memory copy + name-string Unicode)

Ports 9030-9039 (none needed).

## Item 1: mixed-memory copy operand typing — FIXED (false positive)
`memory.copy $dst $src` between memories with differing index types (memory64
i64 vs memory32 i32) produced false `type mismatch` errors. The `memory.copy`
arm in `src/diagnostics_core/semantic.rs` used a single address type derived
from the first memory operand and applied it to all three stack operands.

Fix (localized): added `resolve_memory_copy_addr_types` (mirrors the existing
`resolve_table_copy_addr_types`) and rewrote the `memory.copy` arm to type each
address operand from its own memory; count `n` is i64 only when both memories
are i64.

semantic.rs functions touched (NOT #314's br_table/br_on_cast functions):
- `infer_consumed_types_from_name` `memory.copy` arm (~line 1220)
- NEW fn `resolve_memory_copy_addr_types`

memory.init/fill verified already correct (single memory operand); no change.

Regression fixtures (native+WASM parity corpus):
- memory_copy_mixed_index_valid.wat/.expected.json (dst64/src32, dst32/src64,
  both64 — all valid per `wasm-tools validate --features all`, 0 diagnostics)
- memory_copy_mixed_index_invalid.wat/.expected.json (i32 addr into memory64
  dest — genuine mismatch still caught)

## Item 2: name-string Unicode in validation path — VERIFIED ALREADY CORRECT
Both the browser-reachable path (tree-sitter + semantic) and the native
`wast_validator.rs` path already accept valid Unicode name strings
(export/import names, \u{} escapes, multi-byte, combining marks) with zero false
diagnostics. Confirmed against `wasm-tools validate --features all`. NOTE: WAT
identifiers ($...) are ASCII-idchar only per spec; wasm-tools rejects `$日本語`
— so the concern is specifically name *strings*, which are handled correctly.

Added PASSING regression tests to lock behavior:
- tests/diagnostic_corpus/name_string_unicode_valid.wat/.expected.json (path)
- src/diagnostics/wast_validator.rs unit tests:
  `test_unicode_name_strings_no_false_errors`,
  `test_error_positioning_past_multibyte_name_string`

## CI note — ROOT CAUSE (playground editor severity bug, NOT the Rust fix)
PR #316 Playground Test failed on memory_copy_mixed_index_valid ("should NOT
show errors but do"). Investigation:
- Ran the EXACT CI-built WASM artifact (downloaded emnudge-wat-lsp) in node with
  web-tree-sitter@0.25.3: fix produces only severity-4 HINTS for the valid
  fixture, severity-1 ERROR for the invalid one, 0 for unicode. Correct.
- Built the playground locally and reproduced the chromium failure, then probed
  the actual Monaco markers: the failing marker's message was the memory64
  address-operand HINT, rendered at Monaco severity 8 (Error).
- Root cause: packages/playground/src/components/Editor.vue hardcoded
  `severity: monaco.MarkerSeverity.Error` for every diagnostic, promoting hints
  to red error markers. Fixed to map LSP severities 1/2/3/4 → Monaco
  Error/Warning/Info/Hint. Rebuilt + ran playwright: BOTH parity checks pass
  (23 error-files, 19 no-error-files). Playground typecheck passes.
The shared corpus stays honest: the valid fixture yields no ERRORS on native
AND WASM; only informational hints remain (severity 4).

## Verify: all pass
fmt --check; clippy native --all-targets; clippy wasm --lib; test --features
native (28 test-binary ok, 0 failures; diagnostic_parity corpus green).
tree-sitter build --wasm run before wasm clippy.
