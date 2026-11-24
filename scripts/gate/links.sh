#!/usr/bin/env bash
# Every relative link in the Markdown git does not ignore names a file that exists, and its anchor a heading there.
set -euo pipefail
TOOLS="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$(git rev-parse --show-toplevel)"
git ls-files -z --cached --others --exclude-standard -- '*.md' | xargs -0 lychee --offline --include-fragments --no-progress
# shellcheck source=../find-python.sh
. "$TOOLS/scripts/find-python.sh"
[[ -n "${PYTHON:-}" ]] || { echo "FAIL: no Python $PYTHON_FLOOR+"; exit 1; }
# A heading quoted in a comment (`wire/README.md "Versions"`) is a link lychee does not read.
$PYTHON -I "$TOOLS/scripts/gate/cited_headings.py" --self-test > /dev/null
$PYTHON -I "$TOOLS/scripts/gate/cited_headings.py"
