#!/usr/bin/env python3
"""Writes proto/data/common-passwords.txt: the common passwords a chosen one is compared against (NIST SP 800-63B-4
section 3.1.1.2), from two of SecLists' lists (MIT) at a pinned commit, each checked against its SHA-256. Only an
entry a chosen password could be is kept: at least `proto::rules::MIN_CHOSEN_CHARS` characters once NFC-normalised,
lowercased as `proto::rules::is_common` compares, one a line, sorted, no repeats. Hashcat's `$hex[...]` entries are
read as the bytes they name. An entry the privacy guard refuses in a tracked file (an address, an email, a term of the
maintainer's) is left out, by the guard's own verdict, so the list never carries one. Run by hand when the pin moves;
the list is checked in and read at build time, so no build or check needs the network.

Usage: scripts/deps/passwords.py
"""
import hashlib
import os
import re
import subprocess
import sys
import unicodedata
import urllib.request

root = os.path.realpath(os.path.join(os.path.dirname(__file__), "..", ".."))
COMMIT = "b68489393c7a9bc20f15947b63509a9cbeeb3f92"
BASE = f"https://raw.githubusercontent.com/danielmiessler/SecLists/{COMMIT}"
SOURCES = {
    "Passwords/Common-Credentials/Pwdb_top-1000000.txt":
        "e9a88f67aafe65496682dc374559ee714e978bee50314767494c3e37a18c9fc8",
    "Passwords/Common-Credentials/xato-net-10-million-passwords-1000000.txt":
        "424a3e03a17df0a2bc2b3ca749d81b04e79d59cb7aeec8876a5a3f308d0caf51",
    "LICENSE": "3dbdc93d5f8829de0941744841730a09c106d0732e5ae0e98ca1d77be7ded66c",
}
OUT = os.path.join(root, "proto", "data")


def floor():
    """`MIN_CHOSEN_CHARS`, read from its one owner."""
    with open(os.path.join(root, "proto", "src", "rules.rs"), encoding="utf-8") as rules:
        return int(re.search(r"pub const MIN_CHOSEN_CHARS: usize = (\d+);", rules.read()).group(1))


def fetched(path, digest):
    with urllib.request.urlopen(f"{BASE}/{path}", timeout=60) as reply:
        body = reply.read()
    if hashlib.sha256(body).hexdigest() != digest:
        sys.exit(f"FAIL: {path} at {COMMIT} is not the pinned file")
    return body


def entry(line):
    hexed = re.fullmatch(rb"\$HEX\[([0-9a-f]*)\]", line, re.IGNORECASE)
    raw = bytes.fromhex(hexed.group(1).decode()) if hexed else line
    try:
        return unicodedata.normalize("NFC", raw.decode("utf-8")).lower()
    except (UnicodeDecodeError, ValueError):
        return None


def refused_lines(path):
    """The line numbers of `path` the privacy guard refuses, read from its `file:line: rule` findings."""
    guard = subprocess.run([sys.executable, "-I", os.path.join(root, "scripts", "privacy", "guard.py")],
                           cwd=root, capture_output=True, encoding="utf-8")
    name = os.path.relpath(path, root).replace(os.sep, "/")
    return {int(found.group(1)) for found in re.finditer(rf"^{re.escape(name)}:(\d+): ", guard.stdout, re.MULTILINE)}


def main():
    least = floor()
    kept = set()
    for path, digest in SOURCES.items():
        body = fetched(path, digest)
        if path == "LICENSE":
            with open(os.path.join(OUT, "LICENSE-SecLists.txt"), "wb") as licence:
                licence.write(body)
            continue
        for line in body.splitlines():
            word = entry(line)
            if word is not None and "\n" not in word and len(word) >= least:
                kept.add(word)
    path = os.path.join(OUT, "common-passwords.txt")
    words = sorted(kept)
    while True:
        with open(path, "wb") as out:
            out.write("".join(f"{word}\n" for word in words).encode("utf-8"))
        refused = refused_lines(path)
        if not refused:
            break
        words = [word for at, word in enumerate(words, start=1) if at not in refused]
    left_out = len(kept) - len(words)
    print(f"{len(words)} common passwords of {least} or more characters; {left_out} the guard refused left out")


if __name__ == "__main__":
    main()
