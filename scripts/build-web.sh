#!/usr/bin/env bash
# Build the page into dist/: index.html + the wasm client and its glue.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p secretspace-client --release --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist/pkg
wasm-bindgen --target web --no-typescript --out-dir dist/pkg \
  target/wasm32-unknown-unknown/release/secretspace_client.wasm
cp web/index.html dist/
echo "dist/: $(du -sh dist | cut -f1); wasm $(gzip -9c dist/pkg/secretspace_client_bg.wasm | wc -c) bytes gzipped"
