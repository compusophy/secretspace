#!/usr/bin/env bash
# The rules' teeth. Run before pushing.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0
say() { echo "caps: $*"; fail=1; }

# 1. The game has no dependencies.
deps=$(awk '/^\[dependencies\]/{f=1; next} /^\[/{f=0} f && NF' crates/game/Cargo.toml)
[ -z "$deps" ] || say "crates/game has dependencies: $deps"

# 5. Size caps.
for f in $(git ls-files '*.rs') $(git ls-files --others --exclude-standard '*.rs'); do
  n=$(wc -l < "$f")
  [ "$n" -le 1000 ] || say "$f has $n lines (cap 1000)"
done
chars=$(wc -c < CLAUDE.md)
[ "$chars" -le 8000 ] || say "CLAUDE.md has $chars characters (cap 8000)"

total=$(cat crates/game/src/*.rs | wc -l)
[ "$fail" = 0 ] && echo "caps: ok (game $total lines, CLAUDE.md $chars chars)"
exit "$fail"
