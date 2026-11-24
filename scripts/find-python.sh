# Sourced, not run: sets PYTHON to the first interpreter that actually runs
# Python PYTHON_FLOOR or later, or leaves it empty. One copy, shared by the
# gate, the git hooks and the guard's launcher, so "which Python" is decided
# in one place; 3.11 is the first with `tomllib`, which gate:lints reads.
#
# `command -v` is the wrong test: on Windows `python3` is a stub that is on
# PATH and exits 9009, and a python.org install may provide only `python`
# (or only the `py` launcher).
PYTHON_FLOOR="3.11"
PYTHON=""
for candidate in python3 python "py -3"; do
    # shellcheck disable=SC2086  # "py -3" is deliberately two words
    if $candidate -c "import sys; sys.exit(sys.version_info < tuple(map(int, '$PYTHON_FLOOR'.split('.'))))" \
        >/dev/null 2>&1; then
        PYTHON="$candidate"
        break
    fi
done
