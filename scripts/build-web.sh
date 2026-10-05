#!/usr/bin/env bash
# Build the page into dist/: index.html + the wasm and its glue.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p secretspace-web --release --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist/pkg
wasm-bindgen --target web --no-typescript --out-dir dist/pkg \
  target/wasm32-unknown-unknown/release/secretspace_web.wasm
cp web/index.html dist/
echo "dist/: $(du -sh dist | cut -f1); wasm $(gzip -9c dist/pkg/secretspace_web_bg.wasm | wc -c) bytes gzipped"
