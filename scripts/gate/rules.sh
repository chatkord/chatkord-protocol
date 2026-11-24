#!/usr/bin/env bash
# The gate's own rules answer their self-tests: what a pass records, which headings a comment may cite, what the lint
# table and the clippy settings hold equal, and what a commit message must say.
set -euo pipefail
TOOLS="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$(git rev-parse --show-toplevel)"
# shellcheck source=../find-python.sh
. "$TOOLS/scripts/find-python.sh"
[[ -n "${PYTHON:-}" ]] || { echo "FAIL: no Python $PYTHON_FLOOR+"; exit 1; }
$PYTHON -I "$TOOLS/scripts/gate/record.py" --self-test
$PYTHON -I "$TOOLS/scripts/gate/cited_headings.py" --self-test
$PYTHON -I "$TOOLS/scripts/gate/lints.py" --self-test
$PYTHON -I "$TOOLS/scripts/gate/clippy_configs.py" --self-test
$PYTHON -I "$TOOLS/scripts/check_commit_msg.py" --self-test
