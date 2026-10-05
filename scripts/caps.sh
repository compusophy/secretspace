#!/usr/bin/env bash
# The constitution's teeth. Run before pushing.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0
say() { echo "caps: $*"; fail=1; }

# 1. The world has no dependencies.
deps=$(awk '/^\[dependencies\]/{f=1; next} /^\[/{f=0} f && NF' crates/space/Cargo.toml)
[ -z "$deps" ] || say "crates/space has dependencies: $deps"

# 2. Determinism: no hash-ordered collections, clocks or floats in the world.
if grep -rnE 'HashMap|HashSet|Instant|SystemTime|\bf32\b|\bf64\b' crates/space/src; then
  say "crates/space/src uses something nondeterministic (above)"
fi

# 7. Size caps.
for f in $(git ls-files '*.rs'); do
  n=$(wc -l < "$f")
  [ "$n" -le 1000 ] || say "$f has $n lines (cap 1000)"
done
total=$(cat crates/space/src/*.rs | wc -l)
[ "$total" -le 5000 ] || say "crates/space/src has $total lines (cap 5000)"
chars=$(wc -c < CLAUDE.md)
[ "$chars" -le 8000 ] || say "CLAUDE.md has $chars characters (cap 8000)"

[ "$fail" = 0 ] && echo "caps: ok (world $total lines, CLAUDE.md $chars chars)"
exit "$fail"
