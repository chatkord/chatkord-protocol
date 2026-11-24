#!/usr/bin/env bash
# The gate (`mise run gate`): the fast checks, then clippy, the tests and the documentation of the workspace, and the
# fuzz target's lints on Linux, on this machine. A tool that is absent fails the gate; nothing here reports a pass it
# did not earn. A pass on a clean tree is recorded by its tree's id (scripts/gate/record.py), for a push.
set -euo pipefail
cd "$(dirname "$0")/../.."
case "$(uname -s)" in MINGW* | MSYS*) windows=1; system=windows ;; *) windows=0; system=linux ;; esac

# The gate shares the machine with whatever else is running. The defaults are this machine's own: GATE_MEM is five
# eighths of its memory; clippy runs one rustc per core; a test build links at about 1.1 GB per binary, so
# GATE_BUILD_JOBS is half the cores and never more than one per 2 GB of that ceiling. Each is an override when set.
mem_gb=$(( $(awk '/^MemTotal:/ {print $2}' /proc/meminfo) * 5 / 8 / 1024 / 1024 ))
build_jobs=$(( $(nproc) / 2 < mem_gb / 2 ? $(nproc) / 2 : mem_gb / 2 ))
export GATE_CHECK_JOBS="${GATE_CHECK_JOBS:-$(nproc)}"
export GATE_BUILD_JOBS="${GATE_BUILD_JOBS:-$(( build_jobs > 0 ? build_jobs : 1 ))}"
# On Linux it gets its own memory ceiling too: inside a cgroup scope the kernel kills the gate's own heaviest process
# when it goes over, never something outside it. A runner is single-purpose and sets GATE_SCOPED.
if (( ! windows )) && [[ -z "${GATE_SCOPED:-}" && -z "${CI:-}" ]] && command -v systemd-run > /dev/null; then
  export GATE_SCOPED=1
  exec systemd-run --user --scope --quiet --collect \
    -p "MemoryMax=${GATE_MEM:-${mem_gb}G}" -p MemorySwapMax=0 -- "$0" "$@"
fi

started=$SECONDS
keep=0
for arg in "$@"; do
  case "$arg" in --keep-going) keep=1 ;; *) echo "usage: gate.sh [--keep-going]"; exit 2 ;; esac
done
# --keep-going: each step runs though one before it failed, every failure is named at the end, and nothing is recorded.
failed=()
step() { if (( keep )); then "$@" || failed+=("$*"); else "$@"; fi; }
# shellcheck source=../find-python.sh
. scripts/find-python.sh
[[ -n "${PYTHON:-}" ]] || { echo "FAIL: no Python $PYTHON_FLOOR+"; exit 1; }
clean_tree() { [[ -z "$(git status --porcelain)" ]] && git rev-parse 'HEAD^{tree}'; }
tree="$(clean_tree || true)"

if (( keep )); then step mise run --continue-on-error gate:fast; else mise run gate:fast; fi

# The fuzz target is built with libFuzzer's C++ runtime, which a Linux machine has and a Windows runner may not.
if (( ! windows )); then step scripts/gate/fuzz-lint.sh; else echo "gate: the fuzz target's lints are left to a Linux machine's gate"; fi
step cargo clippy --locked -j "$GATE_CHECK_JOBS" --workspace -- -D warnings
step cargo clippy --locked -j "$GATE_CHECK_JOBS" --workspace --all-targets -- -D warnings
step cargo nextest run --locked --workspace --build-jobs "$GATE_BUILD_JOBS" --no-tests=warn \
  --tool-config-file "gate:$PWD/scripts/gate/nextest.toml"
RUSTDOCFLAGS="-D warnings" step cargo doc --locked -j "$GATE_CHECK_JOBS" --no-deps --workspace

# Recorded only for the tree it ran on from start to end: a commit made meanwhile is not what it tested.
if (( ${#failed[@]} )); then
  echo "GATE: FAIL, ${#failed[@]} step(s), not recorded ($(( SECONDS - started )) s):"
  printf '  %s\n' "${failed[@]}"
  exit 1
fi
left=()
(( windows )) && left+=("the fuzz target's lints, left to Linux")
if [[ -n "$tree" && "$(clean_tree || true)" == "$tree" ]]; then
  $PYTHON -I scripts/gate/record.py write "$tree" "$system" - "${left[@]}"
  echo "GATE: pass, tree $tree, recorded${left[*]:+, leaving out: ${left[*]}} ($(( SECONDS - started )) s)"
else
  echo "GATE: pass, but the tree was not clean or moved while it ran, so not recorded for a push ($(( SECONDS - started )) s)"
fi
