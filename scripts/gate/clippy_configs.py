"""Clippy reads only the clippy.toml nearest a crate and has no include, so a tree's rules are stated in
clippy.extra.toml files and each clippy.toml is written from the protocol repository's (beside this script's repository
root) and the extras from the tree's root down to it: the protocol's settings, then each extra's, a list the earlier's
followed by the later's. A setting two of them state is refused, since one of them would be lost. A root with no extra
of its own keeps the protocol's clippy.toml as it is; rustfmt.toml and rust-toolchain.toml are always the protocol's. A
file that is not what it is written from is written in place, so the difference is in the tree to look at.

Usage: clippy_configs.py | --self-test
"""
import contextlib
import io
import json
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path, PurePosixPath

TOOLS = Path(__file__).resolve().parents[2]
CLIPPY = "clippy.toml"
SHARED = ("rustfmt.toml", "rust-toolchain.toml")
EXTRA = "clippy.extra.toml"
HEADER = ("# Written by the protocol repository's scripts/gate/clippy_configs.py from its clippy.toml and each\n"
          "# clippy.extra.toml from the root to this directory: edit those, never this.\n")


def value(item):
    """`item` as TOML: a bool, a string, a table of those inline, or an array of them a line each."""
    if isinstance(item, bool):
        return "true" if item else "false"
    if isinstance(item, (int, float)):
        return str(item)
    if isinstance(item, str):
        return json.dumps(item, ensure_ascii=False)
    if isinstance(item, dict):
        return "{ " + ", ".join(f"{key} = {value(each)}" for key, each in item.items()) + " }"
    if isinstance(item, list):
        return "[\n" + "".join(f"    {value(each)},\n" for each in item) + "]"
    raise ValueError(f"no TOML written here for {type(item).__name__}")


def merged(root, extra):
    """The settings of `root` with `extra`'s after them, each list the root's followed by the crate's; a refusal for
    each setting both state that is not a list."""
    settings, refused = dict(root), []
    for key, each in extra.items():
        if key not in settings:
            settings[key] = each
        elif isinstance(settings[key], list) and isinstance(each, list):
            settings[key] = settings[key] + each
        else:
            refused.append(key)
    return settings, refused


def written(root, extra):
    """The clippy.toml written from `root` and `extra` (both parsed), or the settings both state."""
    settings, refused = merged(root, extra)
    if refused:
        return None, refused
    lines = [("\n" if isinstance(each, list) else "") + f"{key} = {value(each)}\n" for key, each in settings.items()]
    return HEADER + "\n" + "".join(lines), []


def held(path, wanted, why):
    """`path` written with `wanted` when it differs, and 1 then (0 when equal)."""
    try:
        current = path.read_bytes()
    except FileNotFoundError:
        current = None
    if current == wanted:
        return 0
    path.write_bytes(wanted)
    print(f"FAIL: {path} {why} (written now)")
    return 1


def configs(tree, tools, extras):
    """Each clippy.toml, rustfmt.toml and rust-toolchain.toml `tree` keeps held to what `tools` (the protocol
    repository) and `extras` (the tree's clippy.extra.toml paths, relative) write; the count of failures."""
    with (tools / CLIPPY).open("rb") as file:
        owner = tomllib.load(file)
    failed = sum(held(tree / name, (tools / name).read_bytes(), "is not the protocol repository's") for name in SHARED)
    parsed = {}
    for extra_path in extras:
        with (tree / extra_path).open("rb") as file:
            parsed[extra_path] = tomllib.load(file)
    root_extra = PurePosixPath(EXTRA)
    root = owner
    if root_extra in parsed:
        root, _ = merged(owner, parsed[root_extra])
    else:
        failed += held(tree / CLIPPY, (tools / CLIPPY).read_bytes(), "is not the protocol repository's")
    for extra_path, extra in sorted(parsed.items()):
        base, above = (owner, "the protocol repository's") if extra_path == root_extra else (root, "a clippy.toml above")
        text, refused = written(base, extra)
        for key in refused:
            print(f"FAIL: {extra_path}: {key} is set by {above} too")
            failed += 1
        if text is not None:
            failed += held(tree / extra_path.parent / CLIPPY, text.encode(), f"was not what {extra_path} writes")
    return failed


def self_test():
    root = {"allow-unwrap-in-tests": True, "disallowed-methods": [{"path": "a::b", "reason": "say \"why\""}]}
    extra = {"disallowed-methods": [{"path": "c::d", "reason": "wait"}], "too-many-lines-threshold": 9}
    wrong = []
    # The refusals the shapes below provoke are what is tested, not what the gate reports.
    with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stdout(io.StringIO()):
        tree, tools = Path(scratch, "tree"), Path(scratch, "tools")
        (tree / "crate").mkdir(parents=True)
        tools.mkdir()
        (tools / CLIPPY).write_bytes(b"a = 1\nlist = [\"x\"]\n")
        for name in SHARED:
            (tools / name).write_bytes(b"b = 2\n")
        (tree / CLIPPY).write_bytes(b"a = 0\n")
        if configs(tree, tools, []) != 1 + len(SHARED) or (tree / CLIPPY).read_bytes() != (tools / CLIPPY).read_bytes():
            wrong.append("a shared file that differs or is missing is written in place")
        if configs(tree, tools, []) != 0:
            wrong.append("a shared file that is equal is left alone")
        (tree / EXTRA).write_bytes(b"list = [\"y\"]\n")
        (tree / "crate" / EXTRA).write_bytes(b"list = [\"z\"]\n")
        extras = [PurePosixPath(EXTRA), PurePosixPath("crate", EXTRA)]
        configs(tree, tools, extras)
        if tomllib.loads((tree / CLIPPY).read_text()) != {"a": 1, "list": ["x", "y"]}:
            wrong.append("a root's extra is added to the protocol's")
        if tomllib.loads((tree / "crate" / CLIPPY).read_text()) != {"a": 1, "list": ["x", "y", "z"]}:
            wrong.append("a crate's extra is added to its root's")
        (tree / "crate" / EXTRA).write_bytes(b"a = 2\n")
        if configs(tree, tools, extras) != 1:
            wrong.append("a crate's setting its root states is refused")
    text, refused = written(root, extra)
    if refused or text is None:
        wrong.append("a list both state is joined")
    elif tomllib.loads(text) != {**root, "disallowed-methods": root["disallowed-methods"] + extra["disallowed-methods"],
                                 "too-many-lines-threshold": 9}:
        wrong.append("what is written reads back as the two merged")
    if written(root, {"allow-unwrap-in-tests": False}) != (None, ["allow-unwrap-in-tests"]):
        wrong.append("a setting both state is refused")
    for name in wrong:
        print(f"clippy_configs self-test: wrong answer for {name}")
    if not wrong:
        print("clippy_configs self-test: every shape is answered")
    return 1 if wrong else 0


def main(args):
    if args == ["--self-test"]:
        return self_test()
    if args:
        print(__doc__)
        return 2
    listed = subprocess.run(["git", "ls-files", f"*{EXTRA}"], capture_output=True, text=True, check=True).stdout
    return 1 if configs(Path(), TOOLS, [PurePosixPath(line) for line in listed.split()]) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
