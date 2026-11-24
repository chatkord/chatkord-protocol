"""Every workspace states the same lints: cargo shares no table across workspaces, so each root restates the table
the protocol repository owns (its root manifest's `[workspace.lints]`, read beside this script) and this holds the
copies equal. A root is a directory with a tracked Cargo.lock in the tree this runs in; a package states `[lints]
workspace = true` under a root that is a workspace, and its own `[lints]` where it is the root.

Usage: lints.py | --self-test
"""
import subprocess
import sys
import tomllib
from pathlib import Path, PurePosixPath

OWNER = Path(__file__).resolve().parents[2] / "Cargo.toml"


def lints_of(manifest):
    """The lint table `root`'s manifest states for its packages."""
    if "workspace" in manifest:
        return manifest.get("workspace", {}).get("lints")
    return manifest.get("lints")


def refusals(manifests, owner):
    """Each way `manifests` ({directory: parsed Cargo.toml}, with `roots` the directories holding a lock) differ from
    `owner`, the owner's lint table, as one line each."""
    roots = sorted(manifests["roots"], key=lambda root: len(root.parts), reverse=True)
    tables = manifests["tables"]
    if not owner:
        return [f"{OWNER.name}: states no [workspace.lints], which every root copies"]
    found = []
    for root in sorted(manifests["roots"]):
        table = lints_of(tables[root])
        if table != owner:
            for group in sorted(set(owner) | set(table or {})):
                ours, theirs = owner.get(group, {}), (table or {}).get(group, {})
                for lint in sorted(set(ours) | set(theirs)):
                    if ours.get(lint) != theirs.get(lint):
                        found.append(f"{root}/Cargo.toml: {group}.{lint} is {theirs.get(lint)!r}, the owner's is "
                                     f"{ours.get(lint)!r}")
    for directory, manifest in sorted(tables.items()):
        if "package" not in manifest:
            continue
        root = next((root for root in roots if directory == root or root in directory.parents), None)
        if root is not None and "workspace" in tables[root] and manifest.get("lints") != {"workspace": True}:
            found.append(f"{directory}/Cargo.toml: its [lints] is not `workspace = true`")
    return found


def tracked():
    """The tracked manifests and the roots their locks mark."""
    listed = subprocess.run(["git", "ls-files", "*Cargo.toml", "*Cargo.lock"], capture_output=True, text=True,
                            check=True).stdout.split()
    tables = {}
    for path in (PurePosixPath(line) for line in listed if line.endswith("Cargo.toml")):
        with open(path, "rb") as file:
            tables[path.parent] = tomllib.load(file)
    roots = {PurePosixPath(line).parent for line in listed if line.endswith("Cargo.lock")}
    tables = {path: table for path, table in tables.items() if any(root == path or root in path.parents for root in roots)}
    return {"roots": roots, "tables": tables}


def self_test():
    table = {"rust": {"missing_docs": "warn"}, "clippy": {"panic": "warn"}}
    root = {"workspace": {"members": ["a"], "lints": table}}
    member = {"package": {"name": "a"}, "lints": {"workspace": True}}
    cases = {
        "equal": ({"app": root, "app/a": member, "lone": {"package": {}, "lints": table}}, 0),
        "a root's lint differs": ({"app": root, "other": {"workspace": {"lints": {"rust": table["rust"]}}}}, 1),
        "a lone package's differs": ({"app": root, "lone": {"package": {}, "lints": {}}}, 2),
        "a member keeps its own": ({"app": root, "app/a": {"package": {}, "lints": table}}, 1),
    }
    wrong = []
    for name, (given, count) in cases.items():
        tables = {PurePosixPath(path): manifest for path, manifest in given.items()}
        roots = {path for path in tables if path.name != "a"}
        if len(refusals({"roots": roots, "tables": tables}, table)) != count:
            wrong.append(name)
    for name in wrong:
        print(f"lints self-test: wrong answer for {name}")
    if not wrong:
        print("lints self-test: every shape is answered")
    return 1 if wrong else 0


def main(args):
    if args == ["--self-test"]:
        return self_test()
    if args:
        print(__doc__)
        return 2
    with open(OWNER, "rb") as file:
        owner = lints_of(tomllib.load(file))
    found = refusals(tracked(), owner)
    for line in found:
        print(f"FAIL: {line}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
