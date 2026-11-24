#!/usr/bin/env bash
# proto/gen is what wire/ generates: generated in place, so a difference is left in the tree to look at.
set -euo pipefail
cd "$(dirname "$0")/../.."
buf generate --template wire/buf.gen.yaml wire
# What each op is, from the schema's options (proto/src/ops.rs). Written under build/ and moved in whole, so a run the
# gate kills midway, as it kills every task when another fails, leaves neither a file cut short nor a stray in proto/gen.
mkdir -p build/codegen
next="$(mktemp build/codegen/ops.rs.XXXXXX)"
trap 'rm -f "$next"' EXIT
(cd proto && cargo run --quiet --locked --features codegen --example ops) > "$next"
rustfmt --edition 2024 --config-path rustfmt.toml "$next"
mv "$next" proto/gen/ops.rs
if ! git diff --exit-code --stat -- proto/gen || [[ -n "$(git ls-files --others --exclude-standard -- proto/gen)" ]]; then
  echo "FAIL: proto/gen is not what wire/ generates (the difference is in the tree now)"
  exit 1
fi
