#!/usr/bin/env bash
# A commit message's own checks: its subject's shape, and the privacy guard, since a message is published with the
# commit and no other check reads it. It checks in the tree it is run in.
set -euo pipefail
tools="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=../find-python.sh
. "$tools/scripts/find-python.sh"
[[ -n "$PYTHON" ]] || { echo "commit-msg: Python $PYTHON_FLOOR+ not found; the message was not checked" >&2; exit 1; }
$PYTHON -I "$tools/scripts/check_commit_msg.py" "$1"
exec bash "$tools/scripts/privacy/run.sh" --message "$1"
