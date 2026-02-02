# Diagnostic Corpus

This directory contains WAT files with their expected diagnostics. These tests
are run against BOTH the native and WASM implementations to ensure parity.

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
