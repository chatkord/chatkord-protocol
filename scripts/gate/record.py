"""A gate's pass, recorded by the tree it ran on (build/gate/passed/<tree>): the system it ran on, what it left out by
choice, and the pass it stood on for the workspaces it did not reach. A pass is whole when it left nothing out by
choice and what it stood on is whole on the same system; what its system cannot run is said, never a gap.

Usage: record.py write TREE SYSTEM BASE [LEFT...]   (BASE is a tree, or - for none)
       record.py base SYSTEM                        (the nearest commit on HEAD's line whose tree passed whole here)
       record.py push                               (git's pre-push lines on stdin; refuses a tip that did not)
       record.py --self-test
"""
import json
import os
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

PASSED = Path("build/gate/passed")
SYSTEMS = ("linux", "windows")
CANNOT_RUN_FILE = Path("scripts/gate/cannot-run.toml")


def cannot_run(tree=Path(".")):
    """What a gate on each system cannot run, whatever it is asked, as `tree`'s scripts/gate/cannot-run.toml states
    it (`[systems]` with a `linux` and a `windows` list of sentences); nothing when the tree has none."""
    try:
        with (tree / CANNOT_RUN_FILE).open("rb") as file:
            stated = tomllib.load(file).get("systems", {})
    except FileNotFoundError:
        stated = {}
    return {system: list(stated.get(system, [])) for system in SYSTEMS}


ZERO = "0" * 40


def read(tree, folder=PASSED):
    """The record of `tree`, None when there is none or it is not one this script wrote."""
    try:
        record = json.loads((folder / tree).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return record if isinstance(record, dict) and record.get("system") in SYSTEMS else None


def whole(tree, records, system=None):
    """Whether `tree`'s pass is whole (on `system` when given): through its bases, nothing left out by choice."""
    seen = set()
    while tree not in seen:
        seen.add(tree)
        record = records(tree)
        if record is None or record.get("left_out") or (system and record["system"] != system):
            return False
        system = record["system"]
        if record.get("base") is None:
            return True
        tree = record["base"]
    return False


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout.strip()


def write(tree, system, base, left):
    PASSED.mkdir(parents=True, exist_ok=True)
    record = {"system": system, "base": None if base == "-" else base, "left_out": left, "cannot_run": cannot_run()[system]}
    (PASSED / tree).write_bytes((json.dumps(record) + "\n").encode())


def base(system):
    # A repository with no commit yet has no pass to stand on.
    if subprocess.run(["git", "rev-parse", "--quiet", "--verify", "HEAD"], capture_output=True).returncode != 0:
        return ""
    for commit in git("rev-list", "--first-parent", "--max-count=100", "HEAD").split():
        if whole(git("rev-parse", f"{commit}^{{tree}}"), read, system):
            return commit
    return ""


def push(lines, records, tree_of):
    """The refusals for git's pre-push `lines`, and what each whole tip's system could not run."""
    refused, said = [], []
    for line in lines:
        fields = line.split()
        if len(fields) != 4 or fields[1] == ZERO:
            continue
        ref, tree = fields[0], tree_of(fields[1])
        record = records(tree)
        if not whole(tree, records):
            left = "; left out: " + ", ".join(record["left_out"]) if record and record.get("left_out") else ""
            refused.append(f"{ref}: no whole gate pass for its tree (`mise run gate`){left}")
        else:
            said.append(f"{ref}: passed on {record['system']}; not run there: {', '.join(record['cannot_run'])}")
    return refused, said


def self_test():
    full = {"system": "linux", "base": None, "left_out": [], "cannot_run": []}
    records = {
        "full": full,
        "chained": {**full, "base": "full"},
        "on-a-gap": {**full, "base": "gap"},
        "gap": {**full, "left_out": ["profile ci"]},
        "on-nothing": {**full, "base": "missing"},
        "on-windows": {**full, "system": "windows", "base": "full"},
        "loop-a": {**full, "base": "loop-b"},
        "loop-b": {**full, "base": "loop-a"},
    }
    cases = {
        "a full pass is whole": whole("full", records.get),
        "one standing on a full pass is whole": whole("chained", records.get),
        "one standing on a gap is not": not whole("on-a-gap", records.get),
        "one left out by choice is not": not whole("gap", records.get),
        "one standing on no record is not": not whole("on-nothing", records.get),
        "one standing on another system's pass is not": not whole("on-windows", records.get),
        "a loop of bases is not": not whole("loop-a", records.get),
        "another system's pass is no base here": not whole("full", records.get, "windows"),
        "no record is not": not whole("none", records.get),
    }
    refused, said = push([f"refs/heads/a {'1' * 40} refs/heads/a {ZERO}", f"refs/heads/b {'2' * 40} refs/heads/b {ZERO}",
                          f"(delete) {ZERO} refs/heads/c {'3' * 40}"], records.get, {"1" * 40: "chained", "2" * 40: "gap"}.get)
    cases["a push refuses the tip that did not pass whole and says the other"] = (
        len(refused) == 1 and refused[0].startswith("refs/heads/b") and "profile ci" in refused[0] and len(said) == 1)
    with tempfile.TemporaryDirectory() as scratch:
        (Path(scratch) / CANNOT_RUN_FILE).parent.mkdir(parents=True)
        (Path(scratch) / CANNOT_RUN_FILE).write_text('[systems]\nlinux = ["a"]\n', encoding="utf-8")
        cases["a tree's file says what each system cannot run"] = cannot_run(Path(scratch)) == {"linux": ["a"], "windows": []}
        cases["a tree without one says nothing"] = cannot_run(Path(scratch) / "none") == {"linux": [], "windows": []}
        fresh, here = Path(scratch) / "fresh", Path.cwd()
        subprocess.run(["git", "init", "--quiet", str(fresh)], check=True)
        os.chdir(fresh)
        try:
            cases["a repository with no commit yet has no base"] = base("linux") == ""
        except subprocess.CalledProcessError:
            cases["a repository with no commit yet has no base"] = False
        finally:
            os.chdir(here)
    wrong = [name for name, right in cases.items() if not right]
    for name in wrong:
        print(f"record self-test: wrong answer for {name}")
    if not wrong:
        print("record self-test: every shape is answered")
    return 1 if wrong else 0


def main(args):
    sys.stdout.reconfigure(newline="\n")
    match args:
        case ["--self-test"]:
            return self_test()
        case ["write", tree, system, base_tree, *left] if system in SYSTEMS:
            write(tree, system, base_tree, left)
        case ["base", system] if system in SYSTEMS:
            print(base(system))
        case ["push"]:
            refused, said = push(sys.stdin.read().splitlines(), read, lambda sha: git("rev-parse", f"{sha}^{{tree}}"))
            for line in said:
                print(f"gate: {line}")
            for line in refused:
                print(f"FAIL: {line}")
            return 1 if refused else 0
        case _:
            print(__doc__)
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
