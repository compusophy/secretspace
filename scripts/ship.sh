#!/usr/bin/env bash
# Assemble ship/: a tiny image of the prebuilt server and the pages,
# for `railway up`. Run after scripts/build-web.sh. ship/build.txt is the
# server's build: a hash of every input to the server, which /health
# reports, so CI can skip shipping a server that would not change.
set -euo pipefail
cd "$(dirname "$0")/.."
build=$(bash scripts/hash.sh crates/engine crates/server crates/wyrm crates/luciphon crates/wandfall Cargo.toml Cargo.lock scripts/ship.sh)
SECRETSPACE_BUILD=$build cargo build -p secretspace-server --profile server --target x86_64-unknown-linux-musl
rm -rf ship && mkdir -p ship
cp target/x86_64-unknown-linux-musl/server/server ship/server
cp -r dist ship/dist
echo "$build" > ship/build.txt
cat > ship/Dockerfile <<'DOCKER'
FROM scratch
COPY server /server
COPY dist /dist
ENV PORT=8787
# Visits, souls and worlds are kept here; a volume at /data keeps them.
ENV DATA_DIR=/data
EXPOSE 8787
CMD ["/server", "--static", "/dist"]
DOCKER
echo "ship/: $(du -sh ship | cut -f1), server build $build"
