#!/bin/bash
set -e

cargo build --release
mkdir -p vscode-extension/server
cp target/release/wat-lsp-rust* vscode-extension/server/
cd vscode-extension && npm install && npm run compile
