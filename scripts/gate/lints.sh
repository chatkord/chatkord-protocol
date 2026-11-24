#!/usr/bin/env bash
# Every workspace denies the shared lints and every clippy.toml is written from the shared one: cargo shares no table
# across workspaces, so each root restates the table the protocol repository owns, and this holds the copies equal.
set -euo pipefail
TOOLS="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$(git rev-parse --show-toplevel)"
# shellcheck source=../find-python.sh
. "$TOOLS/scripts/find-python.sh"
[[ -n "${PYTHON:-}" ]] || { echo "FAIL: no Python $PYTHON_FLOOR+"; exit 1; }
$PYTHON -I "$TOOLS/scripts/gate/lints.py" --self-test > /dev/null
$PYTHON -I "$TOOLS/scripts/gate/lints.py"
$PYTHON -I "$TOOLS/scripts/gate/clippy_configs.py" --self-test > /dev/null
$PYTHON -I "$TOOLS/scripts/gate/clippy_configs.py"
