#!/usr/bin/env bash
set -euo pipefail

# Build the wat-lsp playground/docs site
# Based on the CI workflow in .github/workflows/ci.yml

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "==> Building WASM LSP module..."
wasm-pack build --target web --features wasm --no-default-features

echo "==> Copying WASM files to playground..."
mkdir -p playground/public
cp pkg/wat_lsp_rust.js playground/
cp pkg/wat_lsp_rust_bg.wasm playground/public/

echo "==> Installing playground dependencies..."
cd playground
npm install

echo "==> Building playground..."
npm run build

echo "==> Done! Built site is in playground/dist/"
echo "    To preview: cd playground && npm run preview"
