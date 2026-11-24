#!/usr/bin/env bash
# Every workspace's dependencies (each tracked Cargo.lock's directory) against deny.toml: banned crates, licences and
# sources, from the locks alone. The advisories need the network and are audit.yml's (weekly and on every lock change).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
for lock in $(git ls-files -- '*Cargo.lock'); do
  dir=$(dirname "$lock")
  # cargo-deny reads offline, so the locked crates are fetched first: a machine that never built a workspace has none.
  cargo fetch --locked --quiet --manifest-path "$dir/Cargo.toml"
  cargo deny --log-level error --manifest-path "$dir/Cargo.toml" --offline check bans licenses sources
done
