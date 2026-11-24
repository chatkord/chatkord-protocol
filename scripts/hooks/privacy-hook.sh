#!/usr/bin/env bash
# The agents' edit hook (.claude/settings.json, .opencode/plugins/privacy-guard.js): the payload on stdin -- one JSON
# object with a `tool_input` (file_path plus the new text) -- goes to the privacy guard, which refuses the edit before
# it lands. Exit 2 blocks the call and returns the reason; the matched text is never echoed. A guard that could not run
# exits 1: the person sees it on every edit, never a silent pass, while the git hooks and the gate still refuse.
# The guard runs on the project the agent works in ($CLAUDE_PROJECT_DIR, else the working directory).
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 1
exec bash "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/../privacy/run.sh" --hook
