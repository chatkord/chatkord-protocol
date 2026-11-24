#!/usr/bin/env bash
# The fuzz target linted, never run, as cargo-fuzz builds it (`--cfg fuzzing`, which the entry it calls needs), in a
# tree of its own.
set -euo pipefail
cd "$(dirname "$0")/../.."
CARGO_TARGET_DIR="$PWD/build/target/fuzz-lint" RUSTFLAGS="--cfg fuzzing" \
  cargo clippy --locked --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
