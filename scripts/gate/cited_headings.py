#!/usr/bin/env python3
"""A doc section cited in a comment names a heading that exists.

Comments and config cite a doc's section by its quoted title, `wire/README.md "Versions"`, which lychee
(Markdown only) never reads. This checks each such citation outside Markdown names a heading in that file; a citation
of a file that is not there fails too. A finding prints `path:line: cited heading not found`, never the text.

Usage: cited_headings.py | --self-test
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

CITATION = re.compile(r'(?P<path>[\w./-]+\.md) "(?P<title>[^"]+)"')
HEADING = re.compile(r"^#{1,6}\s+(.*?)\s*#*\s*$")
FENCE = re.compile(r"^\s*(```|~~~)")


def headings(text):
    found, fenced = set(), False
    for line in text.splitlines():
        if FENCE.match(line):
            fenced = not fenced
        elif not fenced and (heading := HEADING.match(line)):
            found.add(heading.group(1))
    return found


def refusals(files, doc):
    """`path:line` of each citation in `files` ({path: text}) whose heading `doc(path)` (a set, or None) lacks."""
    found = []
    for path, text in files.items():
        for number, line in enumerate(text.splitlines(), 1):
            for cited in CITATION.finditer(line):
                known = doc(cited.group("path"))
                if known is None or cited.group("title") not in known:
                    found.append(f"{path}:{number}: cited heading not found")
    return found


def tracked(root):
    listed = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=root, check=True, capture_output=True,
    ).stdout.decode().split("\0")
    files = {}
    for path in filter(None, listed):
        if path.endswith(".md") or path.startswith(("upstream/", "protocol/")) or not (root / path).is_file():
            continue
        try:
            files[path] = (root / path).read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
    return files


def reader(root):
    cache = {}

    def doc(path):
        if path not in cache:
            try:
                cache[path] = headings((root / path).read_text(encoding="utf-8"))
            except (UnicodeDecodeError, OSError):
                cache[path] = None
        return cache[path]

    return doc


def self_test():
    docs = {"docs/a.md": headings("# A\n\n## Boundaries ##\n```\n# Fenced\n```\n")}
    def cited(file, title):
        return f"// {file} {chr(34)}{title}{chr(34)}"

    cases = {
        "a heading": (cited("docs/a.md", "Boundaries"), 0),
        "a fenced line": (cited("docs/a.md", "Fenced"), 1),
        "another title": (cited("docs/a.md", "Layout"), 1),
        "a missing file": (cited("docs/b.md", "A"), 1),
        "no citation": ("// docs/a.md is long", 0),
    }
    wrong = [
        name for name, (text, count) in cases.items()
        if len(refusals({"x.rs": text}, docs.get)) != count
    ]
    for name in wrong:
        print(f"cited_headings self-test: wrong answer for {name}")
    if not wrong:
        print("cited_headings self-test: every shape is answered")
    return 1 if wrong else 0


def main(args):
    if args == ["--self-test"]:
        return self_test()
    if args:
        print(__doc__)
        return 2
    root = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True,
                               text=True).stdout.strip())
    found = refusals(tracked(root), reader(root))
    for line in found:
        print(f"FAIL: {line}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
