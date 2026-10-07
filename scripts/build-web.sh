#!/usr/bin/env bash
# Build the pages into dist/: the hub at /, each game at /<game>/.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p secretspace-hub -p secretspace-wyrm-web --release --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist
page() { # page <wasm lib name> <out name> <dir under dist> <html>
  mkdir -p "dist/$3/pkg"
  wasm-bindgen --target web --no-typescript --out-dir "dist/$3/pkg" --out-name "$2" \
    "target/wasm32-unknown-unknown/release/$1.wasm"
  cp "$4" "dist/$3/index.html"
  echo "dist/$3: wasm $(gzip -9c "dist/$3/pkg/$2_bg.wasm" | wc -c) bytes gzipped"
}
page hub hub . web/index.html
page wyrm_web wyrm wyrm web/wyrm/index.html
