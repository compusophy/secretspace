#!/usr/bin/env bash
# The rules' teeth. Run before pushing.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0
say() { echo "caps: $*"; fail=1; }

# 2. The shared crates have no dependencies; a game's core builds only on
# the engine.
deps() { awk '/^\[dependencies\]/{f=1; next} /^\[/{f=0} f && NF' "crates/$1/Cargo.toml"; }
for c in engine pixels; do
  [ -z "$(deps $c)" ] || say "crates/$c has dependencies: $(deps $c)"
done
[ "$(deps wyrm)" = 'engine = { package = "secretspace-engine", path = "../engine" }' ] \
  || say "crates/wyrm depends on more than the engine: $(deps wyrm)"
[ "$(deps luciphon)" = 'engine = { package = "secretspace-engine", path = "../engine" }' ] \
  || say "crates/luciphon depends on more than the engine: $(deps luciphon)"
[ "$(deps wandfall)" = 'engine = { package = "secretspace-engine", path = "../engine" }' ] \
  || say "crates/wandfall depends on more than the engine: $(deps wandfall)"

# 1. No hand-written JavaScript or TypeScript.
js=$(git ls-files '*.js' '*.ts' '*.mjs' '*.jsx' '*.tsx')
[ -z "$js" ] || say "hand-written script in the tree: $js"

# 6. Size caps.
for f in $(git ls-files '*.rs') $(git ls-files --others --exclude-standard '*.rs'); do
  n=$(wc -l < "$f")
  [ "$n" -le 1000 ] || say "$f has $n lines (cap 1000)"
done
chars=$(wc -c < CLAUDE.md)
[ "$chars" -le 8000 ] || say "CLAUDE.md has $chars characters (cap 8000)"

total=$(cat crates/*/src/*.rs | wc -l)
[ "$fail" = 0 ] && echo "caps: ok ($total lines of Rust, CLAUDE.md $chars chars)"
exit "$fail"
