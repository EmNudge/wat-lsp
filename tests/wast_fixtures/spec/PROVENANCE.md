# Vendored spec-testsuite slice

These `.wast` files are a small, **pinned** slice of the upstream WebAssembly
spec test suite, vendored verbatim so the conformance regression gate can cover
real spec cases without any network access at test time.

- Upstream: https://github.com/WebAssembly/testsuite
- Pinned revision: `b464a4cd100d98175ae6e3890db89a2e6c8302f7`
- Vendored files (copied unmodified from the pinned revision):
  - `forward.wast` — a valid module with forward references.
  - `type.wast` — valid modules plus `assert_malformed` type-syntax cases.
  - `local_get.wast` — a valid module plus many `assert_invalid` type-mismatch cases.
  - `int_literals.wast` — a valid module plus many `assert_malformed` literal cases.

Together they exercise all three directive kinds the runner scores (valid
`module`, `assert_invalid`, `assert_malformed`) with cases our pipeline handles
today, so the committed baseline reflects genuine coverage rather than only the
hand-authored fixtures in the parent directory.

## Updating the pin

To bump the revision or widen the slice, re-copy the files from a specific
upstream commit, then regenerate the committed baseline (see the header comment
in `.github/workflows/wast-progress.yml`):

    cargo run --features native --bin wast-runner -- tests/wast_fixtures \
      --format json | scripts/normalize-baseline.py \
      > tests/baseline/wast_fixtures_baseline.json

The runner discovers these recursively, so no runner changes are needed when the
slice grows.
