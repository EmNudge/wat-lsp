# Diagnostic Corpus

This directory contains WAT files with their expected diagnostics. These tests
are run against BOTH the native and WASM implementations to ensure parity.

## Parity contract & native/browser capability differences

The corpus is only a meaningful parity contract if both sides run the **same
pipeline the browser can actually reach**. Concretely, that pipeline is:

- syntax diagnostics (`provide_tree_sitter_diagnostics`), plus
- semantic diagnostics (`collect_all_semantic_diagnostics`),
- merged, sorted by position, and **exact-duplicate deduplicated**.

The native `cargo test diagnostic_parity` harness deliberately mirrors this and
does **not** run `validate_wat` (the `wast`-crate validator). `validate_wat` is a
**native-only** capability: the `wast` dependency is gated behind the `native`
Cargo feature and is not part of the WASM build, so the browser's
`WatDocument.provideDiagnostics` never calls it. Adding `validate_wat` to the
parity harness would test a code path the browser cannot reach and would mask
real parity regressions, so corpus fixtures must only assert diagnostics that the
shared syntax + semantic passes produce.

Because both pipelines dedup exact duplicates, corpus `expected.json` files must
not contain two entries that would match the same emitted diagnostic.

## Structure

Each test case consists of:
- `<name>.wat` - The WAT source file
- `<name>.expected.json` - Expected diagnostics in JSON format

## Expected Format

```json
{
  "diagnostics": [
    {
      "line": 5,
      "message_contains": "Stack underflow",
      "severity": 1
    }
  ]
}
```

We use `message_contains` instead of exact message matching to allow for minor
wording differences between implementations while still catching missing errors.

## Running Tests

- Native: `cargo test diagnostic_parity`
- WASM: `npm test` in `packages/playground` (runs Playwright tests)

## Adding New Tests

1. Create `<name>.wat` with the test case
2. Run `cargo run --bin generate-expected -- <name>.wat` to generate expected file
3. Verify the expected diagnostics are correct
4. Commit both files

## CI Integration

The CI workflow runs both native and WASM implementations against this corpus
and fails if either produces different results than expected.
