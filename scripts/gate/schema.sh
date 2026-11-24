#!/usr/bin/env bash
# The schema lints and builds, and keeps the wire of the commit that opened its major: a change buf calls breaking
# passes only with a new major in wire/PROTOCOL, above every other major wire/MAJORS lists and listed there since the
# commit it breaks (wire/README.md "Versions"). An app pins a commit, so what it shipped is within that major. Run in
# this repository alone: an app holds it as a submodule and gates the pin, not the schema.
set -euo pipefail
cd "$(dirname "$0")/../.."
(cd wire && buf lint && buf build -o /dev/null)
now="$(< wire/PROTOCOL)"
major="${now%%.*}"

# The majors a ledger lists, one a line, its `#` lines left out.
majors() { grep -v '^#' | grep -v '^$' || true; }
listed="$(majors < wire/MAJORS)"
if ! grep -qx "$major" <<< "$listed"; then
  echo "FAIL: wire/PROTOCOL's major $major is not listed in wire/MAJORS"
  exit 1
fi
if grep -qvE '^(0|[1-9][0-9]*)$' <<< "$listed"; then
  echo "FAIL: wire/MAJORS holds a line that is no major"
  exit 1
fi
if ! awk 'NR > 1 && $1 <= last { exit 1 } { last = $1 }' <<< "$listed"; then
  echo "FAIL: wire/MAJORS lists a major not above every line before it"
  exit 1
fi
# The ledger only grows: HEAD's lines stay its first, so no major it lists is used again.
kept="$({ git show HEAD:wire/MAJORS 2> /dev/null || true; } | majors)"
if [[ -n "$kept" && "$listed"$'\n' != "$kept"$'\n'* ]]; then
  echo "FAIL: wire/MAJORS drops or changes a major HEAD's lists; the ledger only gains lines"
  exit 1
fi

# Passes when wire/ keeps the wire of `commit` (named `name`, read by buf as `input`), or its major is a new one.
against() {
  local name="$1" input="$2" commit="$3" depth before
  # buf finds a ref within its clone's depth (50 by default): deep enough to reach `commit` from here.
  depth="$(( $(git rev-list --count "$commit..HEAD") + 1 ))"
  buf breaking wire --against ".git#$input,subdir=wire,depth=$depth" && return 0
  before="$({ git show "$commit:wire/MAJORS" 2>/dev/null || true; } | majors)"
  if grep -qx "$major" <<< "$before"; then
    echo "FAIL: wire/ breaks $name (above), whose ledger already lists major $major: keep the change additive" \
      "(a minor), or raise wire/PROTOCOL to a new major and list it in wire/MAJORS"
    return 1
  fi
  if grep -vx "$major" <<< "$listed" | awk -v major="$major" '$1 >= major { found = 1 } END { exit !found }'; then
    echo "FAIL: wire/ breaks $name (above) with major $major, not above every major wire/MAJORS lists: a major is" \
      "never used twice"
    return 1
  fi
  echo "gate: wire/ breaks $name, allowed by the new major $major"
}

# The first commit has no HEAD to compare with.
if ! git rev-parse --quiet --verify HEAD > /dev/null; then
  echo "gate: no commit yet, so the schema is compared with nothing"
  exit 0
fi

# The commit that opened this major is the first whose ledger lists it (the root commit, for the first major); a major
# no commit lists yet is opened by this tree, against HEAD.
opened="$(git log --reverse --format=%H -G "^$major\$" -- wire/MAJORS | awk 'NR == 1')"
if [[ -n "$opened" ]]; then
  against "$opened, which opened major $major" "ref=$opened" "$opened"
else
  against "HEAD" "ref=$(git rev-parse HEAD)" HEAD
fi
