#!/usr/bin/env bash
# Build the pages into dist/: the hub at /, each game at /<game>/. Each page
# knows its build (a hash of every input to the pages), and dist/version.txt
# says the newest, so an open page can tell it is out of date. Where
# binaryen's wasm-opt is installed, each page's wasm is shrunk with it
# (about a quarter smaller, an eighth gzipped); without it, as built.
set -euo pipefail
cd "$(dirname "$0")/.."
build=$(bash scripts/hash.sh crates/engine crates/pixels crates/kit crates/gpu crates/render crates/hub-web crates/wyrm crates/wyrm-look crates/wyrm-web crates/luciphon crates/luciphon-look crates/luciphon-web crates/showcase-web crates/wandfall crates/wandfall-look crates/wandfall-web web Cargo.toml Cargo.lock scripts/build-web.sh)
SECRETSPACE_PAGE=$build cargo build -p secretspace-hub -p secretspace-wyrm-web -p secretspace-luciphon-web -p secretspace-showcase-web -p secretspace-wandfall-web --release --target wasm32-unknown-unknown
target=${CARGO_TARGET_DIR:-target}
opt=$(command -v wasm-opt || true)
rm -rf dist && mkdir -p dist
page() { # page <wasm lib name> <out name> <dir under dist> <html>
  mkdir -p "dist/$3/pkg"
  wasm-bindgen --target web --no-typescript --out-dir "dist/$3/pkg" --out-name "$2" \
    "$target/wasm32-unknown-unknown/release/$1.wasm"
  cp "$4" "dist/$3/index.html"
  local w="dist/$3/pkg/$2_bg.wasm"
  if [ -n "$opt" ]; then
    # The features rustc's wasm already uses: wasm-opt checks against them.
    "$opt" -O3 --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int \
      --enable-mutable-globals --enable-reference-types --enable-multivalue "$w" -o "$w.opt"
    mv "$w.opt" "$w"
  fi
  echo "dist/$3: wasm $(wc -c < "$w") bytes, $(gzip -9c "$w" | wc -c) gzipped${opt:+ (wasm-opt)}"
}
page hub hub . web/index.html
page wyrm_web wyrm wyrm web/wyrm/index.html
page luciphon_web luciphon luciphon web/luciphon/index.html
page showcase_web showcase showcase web/showcase/index.html
page wandfall_web wandfall wandfall web/wandfall/index.html
echo "$build" > dist/version.txt
echo "pages build $build"
