#!/usr/bin/env bash
# Every workspace (each tracked Cargo.lock's directory) formatted as rustfmt.toml says.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
for lock in $(git ls-files -- '*Cargo.lock'); do
  dir=$(dirname "$lock")
  (cd "$dir" && cargo fmt --check) || { echo "FAIL: $dir is not formatted; run cargo fmt there"; exit 1; }
done
