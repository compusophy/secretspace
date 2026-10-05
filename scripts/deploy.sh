#!/usr/bin/env bash
# Build the page and deploy it to Vercel (production).
#
#   bash scripts/deploy.sh                          # page only: tabs of one browser link
#   RELAY=wss://your-relay/ws bash scripts/deploy.sh   # devices link through your relay
#
# Vercel hosts the page, not the relay: the relay holds WebSockets open, so
# it runs elsewhere (Railway, Fly: deploy the Dockerfile; it honours PORT).
# Needs: rustup, and the Vercel CLI logged in. Installs the wasm target and
# the pinned wasm-bindgen CLI if they are missing.
set -euo pipefail
cd "$(dirname "$0")/.."

need=$(awk '/name = "wasm-bindgen"/{getline; gsub(/[^0-9.]/,""); print; exit}' Cargo.lock)
rustup target add wasm32-unknown-unknown >/dev/null
if [ "$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" != "$need" ]; then
  echo "installing wasm-bindgen-cli $need (one time, a few minutes)"
  cargo install wasm-bindgen-cli --version "$need" --locked
fi
command -v vercel >/dev/null || { echo "the Vercel CLI is missing: npm i -g vercel, then vercel login"; exit 1; }

bash scripts/build-web.sh
relay="${RELAY:-off}"
sed -i.bak "s|<meta name=\"relay\" content=\"[^\"]*\">|<meta name=\"relay\" content=\"$relay\">|" dist/index.html && rm -f dist/index.html.bak
grep -q "name=\"relay\" content=\"$relay\"" dist/index.html || { echo "could not set the relay in dist/index.html"; exit 1; }
cp web/vercel.json dist/
echo "relay: $relay"
# Deploy from a folder named after the project, keeping its Vercel link
# (.vercel) between deploys, so Vercel names the project secretspace.
stage=.deploy/secretspace
mkdir -p "$stage"
find "$stage" -mindepth 1 -maxdepth 1 ! -name .vercel -exec rm -rf {} +
cp -R dist/. "$stage"/
vercel deploy "$stage" --prod --yes
