#!/usr/bin/env bash
# Assemble ship/: a tiny image of the prebuilt arena server and the page,
# for `railway up`. Run after scripts/build-web.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p secretspace-server --release --target x86_64-unknown-linux-musl
rm -rf ship && mkdir -p ship
cp target/x86_64-unknown-linux-musl/release/server ship/server
cp -r dist ship/dist
cat > ship/Dockerfile <<'DOCKER'
FROM scratch
COPY server /server
COPY dist /dist
ENV PORT=8787
EXPOSE 8787
CMD ["/server", "--static", "/dist"]
DOCKER
echo "ship/: $(du -sh ship | cut -f1)"
