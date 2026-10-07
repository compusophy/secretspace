#!/usr/bin/env bash
# Build the pages into dist/: the hub at /, each game at /<game>/. Each page
# knows its build (a hash of every input to the pages), and dist/version.txt
# says the newest, so an open page can tell it is out of date.
set -euo pipefail
cd "$(dirname "$0")/.."
build=$(bash scripts/hash.sh crates/engine crates/pixels crates/kit crates/hub-web crates/wyrm crates/wyrm-look crates/wyrm-web crates/luciphon crates/luciphon-look crates/luciphon-web web Cargo.toml Cargo.lock)
SECRETSPACE_PAGE=$build cargo build -p secretspace-hub -p secretspace-wyrm-web -p secretspace-luciphon-web --release --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist
page() { # page <wasm lib name> <out name> <dir under dist> <html>
  mkdir -p "dist/$3/pkg"
  wasm-bindgen --target web --no-typescript --out-dir "dist/$3/pkg" --out-name "$2" \
    "target/wasm32-unknown-unknown/release/$1.wasm"
  cp "$4" "dist/$3/index.html"
  local w="dist/$3/pkg/$2_bg.wasm"
  echo "dist/$3: wasm $(wc -c < "$w") bytes, $(gzip -9c "$w" | wc -c) gzipped"
}
page hub hub . web/index.html
page wyrm_web wyrm wyrm web/wyrm/index.html
page luciphon_web luciphon luciphon web/luciphon/index.html
echo "$build" > dist/version.txt
echo "pages build $build"
