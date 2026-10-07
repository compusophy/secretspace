#!/usr/bin/env bash
# A short hash of what goes into a build: hash.sh <path>... prints 12 hex
# digits over the files' contents (git's blob ids) and the compiler.
set -euo pipefail
cd "$(dirname "$0")/.."
{ git ls-files -s -- "$@"; rustc -V; } | sha256sum | cut -c1-12
