#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

cargo build --release
mkdir -p packages/vscode-extension/server
cp target/release/wat-lsp-rust* packages/vscode-extension/server/
cd packages/vscode-extension && npm install && npm run compile
