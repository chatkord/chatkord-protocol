#!/usr/bin/env bash
# The privacy guard's one launcher (the gate, the git hooks through lefthook, the agents' edit hook): Python as
# find-python.sh finds it and gitleaks at the version the tree's mise.toml pins. It runs on the tree it is started in
# (an agent's hook starts it in $CLAUDE_PROJECT_DIR); the guard's own files are found beside this script. A missing
# tool exits 1, never a pass.
tools="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
tree="$(git rev-parse --show-toplevel 2> /dev/null || echo "$tools")"
# shellcheck source=../find-python.sh
. "$tools/scripts/find-python.sh"
[[ -n "$PYTHON" ]] || { echo "privacy guard: Python $PYTHON_FLOOR+ not found on PATH; the guard did not run" >&2; exit 1; }
command -v mise > /dev/null || { echo "privacy guard: mise not found on PATH; the guard did not run" >&2; exit 1; }
cd "$tree"
eval "$(mise env --shell bash --quiet)"
exec $PYTHON -I "$tools/scripts/privacy/guard.py" "$@"
