#!/usr/bin/env bash
# A short hash of what goes into a build: hash.sh <path>... prints 12 hex
# digits over the files' contents (git's blob ids, edits not yet staged,
# and new files not yet added) and the compiler. A clean checkout hashes
# its blob ids alone, as CI sees it.
set -euo pipefail
cd "$(dirname "$0")/.."
{
  git ls-files -s -- "$@"
  git diff --no-ext-diff -- "$@"
  git ls-files -o --exclude-standard -z -- "$@" | xargs -0 -r sha256sum
  rustc -V
} | sha256sum | cut -c1-12
