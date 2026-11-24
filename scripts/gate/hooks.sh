#!/usr/bin/env bash
# Every hook lefthook.yml names is installed and executable: git skips a hook that is not executable with a hint and no
# failure, which `lefthook check-install` does not see. One that is not is put back by `mise run hooks`, then checked
# again. Under CI and in the builder no commit is made, so there is nothing to check.
set -euo pipefail
TOOLS="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$(git rev-parse --show-toplevel)"
if [[ -n "${CI:-}" || -n "${CHATKORD_IN_BUILDER:-}" ]]; then
  echo "gate: the git hooks not checked (no commit is made under CI or in the builder)"
  exit 0
fi
# shellcheck source=../find-python.sh
. "$TOOLS/scripts/find-python.sh"
[[ -n "${PYTHON:-}" ]] || { echo "FAIL: no Python $PYTHON_FLOOR+"; exit 1; }
# A hook is a top-level entry with commands, jobs or scripts; the rest are lefthook's settings.
mapfile -t hooks < <(lefthook dump --format json | $PYTHON -I -c '
import json, sys
for name, value in json.load(sys.stdin).items():
    if isinstance(value, dict) and value.keys() & {"commands", "jobs", "scripts"}:
        print(name)')
(( ${#hooks[@]} )) || { echo "FAIL: lefthook.yml names no hook"; exit 1; }

missing() {
  lefthook check-install > /dev/null 2>&1 || { echo "lefthook check-install"; return; }
  local hook path
  for hook in "${hooks[@]}"; do
    path="$(git rev-parse --git-path "hooks/$hook")"
    [[ -f "$path" && -x "$path" ]] || echo "$hook"
  done
}

wrong="$(missing)"
[[ -z "$wrong" ]] && exit 0
echo "gate: the git hooks not installed or not executable (${wrong//$'\n'/, }), so they are installed again"
mise run hooks > /dev/null
wrong="$(missing)"
[[ -z "$wrong" ]] ||
  { echo "FAIL: the git hooks are still not installed and executable after mise run hooks: ${wrong//$'\n'/, }"; exit 1; }
