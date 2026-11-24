#!/usr/bin/env bash
# The pre-push hook: git's lines on stdin, refused unless the tip of each ref passed the gate whole.
set -euo pipefail
TOOLS="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$(git rev-parse --show-toplevel)"
# shellcheck source=../find-python.sh
. "$TOOLS/scripts/find-python.sh"
[[ -n "${PYTHON:-}" ]] || { echo "FAIL: no Python $PYTHON_FLOOR+"; exit 1; }
$PYTHON -I "$TOOLS/scripts/gate/record.py" push
