#!/usr/bin/env bash
# The parser an untrusted peer feeds, fuzzed: `mise run fuzz [TARGET] [SECONDS]` runs TARGET, or every target the fuzz
# workspace lists, for SECONDS each (60 unless given), from its tracked seeds, the frames the schema makes for every op
# (`proto::fuzz::made`) and this machine's working corpus (fuzz/corpus, ignored), then minimises that corpus.
# Never a gate stage: the gate lints the target and replays the seeds in proto's tests. Linux only (libFuzzer).
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ "$(uname -s)" == Linux ]] || { echo "FAIL: fuzzing runs on Linux"; exit 1; }
seconds="${2:-60}"
# Built for an explicit target with coverage flags: a tree apart from the gate's, so neither rebuilds the other.
export CARGO_TARGET_DIR="$PWD/build/target/fuzz"
if [[ -n "${1:-}" ]]; then targets=("$1"); else mapfile -t targets < <(cargo fuzz list --fuzz-dir fuzz); fi
for target in "${targets[@]}"; do
  # A safe-Rust parser needs no sanitizer, so the stable toolchain builds it; an input over 10 s is a finding.
  mkdir -p "fuzz/corpus/$target"
  # One frame for every op a peer sends, made from the schema and so never kept as files.
  cargo run --quiet --manifest-path proto/Cargo.toml --features testing --example seeds -- "$target" "fuzz/corpus/$target"
  cargo fuzz run --fuzz-dir fuzz --sanitizer none "$target" "fuzz/corpus/$target" "fuzz/seeds/$target" -- \
    -max_total_time="$seconds" -timeout=10
  # Merge into a corpus of its own and copy back: libFuzzer's merge swaps the folder it is given.
  kept="$CARGO_TARGET_DIR/kept/$target"
  rm -rf "$kept" && mkdir -p "$kept"
  cargo fuzz run --fuzz-dir fuzz --sanitizer none "$target" "$kept" "fuzz/corpus/$target" -- -merge=1
  find "fuzz/corpus/$target" -type f -delete
  cp "$kept"/* "fuzz/corpus/$target/"
done
