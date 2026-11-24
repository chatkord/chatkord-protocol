#!/usr/bin/env bash
# The privacy guard over the tree and its self-test, and the commit-message check's self-test.
set -euo pipefail
TOOLS="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$(git rev-parse --show-toplevel)"
bash "$TOOLS/scripts/privacy/run.sh"
bash "$TOOLS/scripts/privacy/run.sh" --self-test
# shellcheck source=../find-python.sh
. "$TOOLS/scripts/find-python.sh"
[[ -n "${PYTHON:-}" ]] || { echo "FAIL: no Python $PYTHON_FLOOR+"; exit 1; }
$PYTHON -I "$TOOLS/scripts/check_commit_msg.py" --self-test
