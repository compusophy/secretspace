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
[ -z "$(deps battlestation)" ] || say "crates/battlestation has dependencies: $(deps battlestation)"

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

# The core is deterministic: what a page predicts (Wandfall's movement and
# the island it moves over) must come out bit for bit as the server's, so
# that code calls crate::trig, never the platform's float functions (sqrt,
# exact everywhere, is fine). Tests may use them (after a #[cfg(test)],
# or in a tests.rs).
for f in crates/wandfall/src/{motion,tether,wall,predict,map,places}.rs \
  $(find crates/wandfall/src/motion crates/wandfall/src/predict -name '*.rs' ! -name tests.rs 2>/dev/null); do
  [ -f "$f" ] || continue
  hits=$(sed '/#\[cfg(test)\]/,$d' -- "$f" | grep -nE '\.(sin|cos|tan|asin|acos|atan|atan2|sinh|cosh|tanh|sin_cos|powf|exp|exp2|exp_m1|ln|ln_1p|log|log2|log10|hypot|cbrt)\(' || true)
  [ -z "$hits" ] || say "$f calls the platform's float functions (use crate::trig): $hits"
done

# 7. Every WGSL string has a naga test: each string holding an entry point
# sits in a const (or a fn giving a string) that a test's code names, or
# that something such a test reaches names (a shader put together from
# parts). shaders.awk says how, and why a test saying `new` is not enough.
for crate in crates/*/; do
  srcs=$(find "${crate}src" -name '*.rs' 2>/dev/null | sort || true)
  [ -n "$srcs" ] || continue
  tests=$(find "${crate}tests" -name '*.rs' 2>/dev/null | sort || true)
  # shellcheck disable=SC2086
  while IFS= read -r why; do
    [ -z "$why" ] || say "$why"
  done < <(awk -v crate="$crate" -f scripts/shaders.awk $srcs $tests | sort)
done

total=$(cat crates/*/src/*.rs | wc -l)
[ "$fail" = 0 ] && echo "caps: ok ($total lines of Rust, CLAUDE.md $chars chars)"
exit "$fail"
