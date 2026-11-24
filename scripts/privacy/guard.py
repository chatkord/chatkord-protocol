#!/usr/bin/env python3
"""The privacy and portability guard (README.md "Privacy guard").

The repo must read the same to anyone running any Chatkord server, so no tracked file may carry the maintainer's
server, accounts, machines or monitors, nor a secret. gitleaks applies the shapes (`.gitleaks.toml`, on its own secret
rules) and the maintainer's private terms, which are read at run time from the gitignored LAB.local.md, or from the
`PRIVACY_TERMS` secret in CI, and never written to a file. This script keeps what a pattern cannot say: an id standing
in with 0/-1 or a bare number, a test skipped without saying so (both by where a line sits in its file), a private host
name (in source code only where one can be written, a string or a comment), and a file that cannot be read as text. A finding prints `path:line: rule` and never the matched text, because this output ends
up in terminals, hook messages and CI logs.

Every mode puts what it scans in a scratch tree at its own paths and scans that, so a path's allowlist holds alike:

    guard.py                  every file git does not ignore (the gate)
    guard.py --staged         the index (pre-commit)
    guard.py --message FILE   a commit message (commit-msg)
    guard.py --hook           an agent's edit (JSON on stdin; exit 2 refuses it)
    guard.py --self-test      every rule still catches its shape, and clean text passes
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import shutil
import string
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

# The tools are this script's repository (the protocol, which an app holds at `protocol/`); the tree is the repository
# the guard is run in, whose files it scans. Inside the protocol repository they are one folder.
TOOLS = Path(__file__).resolve().parents[2]


def tree_root() -> Path:
    """The top of the git tree the guard runs in; the tools' own when the working directory is in none."""
    run = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    return Path(run.stdout.strip()) if run.returncode == 0 and run.stdout.strip() else TOOLS


ROOT = tree_root()
# The shapes are the protocol's alone; a tree adds only its allowances, as top-level `[[allowlists]]` (`targetRules`
# names a shape's id). Joined as text, since an app config extending this one would pass gitleaks' extend depth.
CONFIG = TOOLS / ".gitleaks.toml"
TREE_ALLOW_PATH = "scripts/privacy/allow.gitleaks.toml"
# One private file serves every repository: the tree's own, else the one in the sibling chatkord-dev.
LAB = next((lab for lab in (ROOT / "LAB.local.md", ROOT.parent / "chatkord-dev" / "LAB.local.md") if lab.is_file()), ROOT / "LAB.local.md")
ALLOW_PATH = "scripts/privacy/allow.txt"
ALLOW_FILE = ROOT / ALLOW_PATH
# A fuzzer writes any address, host or path by design: its inputs are read for the private terms and for secrets,
# never for the shapes, and as bytes, a NUL among them included.
FUZZ_INPUTS = "fuzz/seeds/"

TEST, PROD = "test", "prod"

# An unknown id is an absent field, never 0/-1, and a test fixture gives its ids names so no
# number gains meaning.
ID_PROD_RS = re.compile(
    r"\bset_\w*_id\(\s*-?[01]\s*\)"
    r"|\b\w*_id:\s*-?\d+\s*[,})]"
    r"|\b\w*_id\s*=\s*-?\d+\s*;"
    r"|_ID:\s*i\d+\s*=\s*-?\d+\s*;"
    r"|_id\)?\s*\.unwrap_or(?:_default)?\(\s*(?:-?[01])?\s*\)"
    r"|\.unwrap_or\(\s*-1\s*\)|\bmap_or\(\s*-1\b"
)
ID_TEST_RS = re.compile(
    r"\b(?:user|channel|category|role|message|remote|server|author|parent_message"
    r"|reply_to|own_user|voice_channel|session|track)_id\s*[:=]\s*(?:Some\()?-?\d"
)
# A hardware test that returns early without saying so passes on a machine where it proved nothing.
TEST_SKIP_RS = re.compile(r"eprintln!\(.*\bskip|else\s*\{\s*(?:return|continue)\s*;?\s*\}", re.IGNORECASE)
# A name only a home or office network resolves. In source code a host is a value, so it sits in a string or a
# comment; outside them a dotted name is a field or a method, which is no host.
PRIVATE_HOST = re.compile(r"(?i)\b([a-z0-9][a-z0-9-]*\.(?:lan|local|localdomain|home\.arpa|internal|ts\.net))(?:[^.\w-]|$)")
# The name every machine calls itself.
LOOPBACK_HOST = re.compile(r"(?i)^localhost\.")
# A protocol constant a server defines itself, the one exception, says so on its own line.
ID_LINE_EXEMPT = "privacy: upstream constant"

HINTS = {
    "private-term": "a term listed in LAB.local.md; use its pseudonym",
    "id-prod": "an unknown id is an absent field/None, never 0/-1/''",
    "id-test": "give the id a name (const ADA = ...) so no number gains meaning",
    "test-skip": "report a skip through the skip helper (tier::skipped), never a bare return",
    "unscannable": "a tracked file the guard cannot read as text hides what it holds; keep tracked files UTF-8 text",
    "private-host": "use example.invalid (or a pseudonym) for a host",
}
SECRET_HINT = "a secret gitleaks recognises: remove it, and rotate it if it was real"
OWN_RULES = ("id-prod", "id-test", "test-skip", "unscannable", "private-host")


@dataclass(frozen=True)
class Hit:
    path: str
    line: int
    rule: str
    hint: str


class GuardError(Exception):
    """The guard could not run: never a pass."""


# --- The private terms -------------------------------------------------------------------------------------------


def term_lines() -> list[str] | None:
    """The list items of LAB.local.md's `## Private terms`, or PRIVACY_TERMS (the same items, one per line).
    None: neither is here."""
    if os.environ.get("PRIVACY_TERMS", "").strip():
        source = os.environ["PRIVACY_TERMS"].splitlines()
    elif LAB.is_file():
        source, inside = [], False
        for raw in LAB.read_text(encoding="utf-8").splitlines():
            if raw.startswith("## "):
                inside = raw[3:].strip().lower() == "private terms"
            elif inside:
                source.append(raw)
    else:
        return None
    # Only list items are terms; prose is not.
    return [raw[2:].strip() for raw in source if raw.startswith("- ") and raw[2:].strip()]


# The one file a private term may sit in.
CHANGELOG_REASON = (
    "written by release-please: each entry links its commit at the repository's own GitHub URL; "
    "the subjects were checked by the commit hooks"
)
RE2_SPECIAL = set(r"\.+*?()|[]{}^$")


def as_pattern(item: str) -> str:
    if item.startswith("re:"):
        return item[3:].strip()
    return "".join("\\" + c if c in RE2_SPECIAL else c for c in item)


def gitleaks_config(terms: list[str] | None, shapes: bool = True) -> str:
    """The shapes with the tree's allowances (without the shapes, gitleaks' own secret rules alone), and the private
    terms, as text handed to gitleaks in its environment."""
    if shapes:
        allowances = ROOT / TREE_ALLOW_PATH
        config = CONFIG.read_text(encoding="utf-8")
        if ROOT != TOOLS and allowances.is_file():
            config += "\n" + allowances.read_text(encoding="utf-8")
    else:
        config = "[extend]\nuseDefault = true\n"
    if terms:
        alternatives = "|".join(f"(?:{as_pattern(t)})" for t in terms)
        config += (
            '\n[[rules]]\nid = "private-term"\n'
            f"description = {json.dumps(HINTS['private-term'])}\n"
            f"regex = {json.dumps('(?i)' + alternatives)}\n"
            "[[rules.allowlists]]\n"
            f"description = {json.dumps(CHANGELOG_REASON)}\n"
            f"paths = [{json.dumps('^CHANGELOG[.]md$')}]\n"
        )
    return config


# --- Scanning ----------------------------------------------------------------------------------------------------


def is_test_file(path: str) -> bool:
    return "/tests/" in path or path.endswith("_tests.rs") or "/testdata/" in path


_CFG_TEST = re.compile(r"^\s*#\[cfg\(test\)\]")
_MOD = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{")  # a body: a `mod x;` only names a file


def test_region_start(lines: list[str]) -> int | None:
    """Index of the line a Rust file's `#[cfg(test)] mod ...` starts at."""
    for i, line in enumerate(lines):
        if not _CFG_TEST.match(line):
            continue
        for following in lines[i + 1 : i + 4]:
            if following.strip():
                if _MOD.match(following):
                    return i
                break
    return None


def region_of(path: str, lines: list[str]) -> list[str]:
    if is_test_file(path):
        return [TEST] * len(lines)
    start = test_region_start(lines) if path.endswith(".rs") else None
    if start is None:
        return [PROD] * len(lines)
    return [PROD] * start + [TEST] * (len(lines) - start)


# Source code by its file's extension: how a comment starts, and whether `'` opens a string or (Rust) a lifetime too.
SLASH_COMMENTS = {".rs", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".go", ".c", ".h", ".cc", ".cpp", ".hpp", ".wgsl",
                  ".java", ".kt", ".swift", ".cs", ".proto", ".comp", ".hlsl"}
HASH_COMMENTS = {".py"}
_RUST_CHAR = re.compile(r"'(?:\\.[^']*|[^'\\\n])'")
_RUST_RAW = re.compile(r'b?r(#*)"')


def written_parts(path: str, text: str) -> str:
    """The text with every character of code outside a string literal or a comment made a space, lines kept: what in
    a source file can hold a value someone wrote. A file that is not source code is all written."""
    ext = Path(path).suffix
    if ext not in SLASH_COMMENTS and ext not in HASH_COMMENTS:
        return text
    rust, python = ext == ".rs", ext in HASH_COMMENTS
    out = [c if c == "\n" else " " for c in text]
    i, n = 0, len(text)

    def keep(start: int, end: int) -> None:
        out[start:end] = text[start:end]

    while i < n:
        c = text[i]
        if (python and c == "#") or (not python and text.startswith("//", i)):
            end = text.find("\n", i)
            end = n if end < 0 else end
            keep(i, end)
            i = end
        elif not python and text.startswith("/*", i):
            end = text.find("*/", i + 2)
            end = n if end < 0 else end + 2
            keep(i, end)
            i = end
        elif rust and (raw := _RUST_RAW.match(text, i)) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            close = '"' + raw.group(1)
            end = text.find(close, raw.end())
            end = n if end < 0 else end + len(close)
            keep(i, end)
            i = end
        elif rust and c == "'":
            char = _RUST_CHAR.match(text, i)
            end = char.end() if char else i + 1
            keep(i, end)
            i = end
        elif c in "\"'`" and (c != "`" or not python):
            quote = text[i : i + 3] if python and text.startswith(c * 3, i) else c
            j = i + len(quote)
            while j < n and not text.startswith(quote, j):
                j += 2 if text[j] == "\\" else 1
            end = min(n, j + len(quote))
            keep(i, end)
            i = end
        else:
            i += 1
    return "".join(out)


def private_hosts(path: str, text: str) -> list[Hit]:
    hits = []
    for number, line in enumerate(written_parts(path, text).splitlines(), start=1):
        for match in PRIVATE_HOST.finditer(line):
            if not LOOPBACK_HOST.match(match.group(1)):
                hits.append(Hit(path, number, "private-host", HINTS["private-host"]))
                break
    return hits


def own_rules(path: str, text: str, regions: list[str] | None = None) -> list[Hit]:
    """The rules a pattern alone cannot apply: they depend on where a line sits in its file."""
    hosts = private_hosts(path, text)
    if not path.endswith(".rs"):
        return hosts
    lines = text.splitlines()
    regions = regions if regions is not None else region_of(path, lines)
    hits = []
    for number, (line, region) in enumerate(zip(lines, regions), start=1):
        if ID_LINE_EXEMPT in line:
            continue
        rules = []
        if region == PROD and ID_PROD_RS.search(line):
            rules.append("id-prod")
        if region == TEST and ID_TEST_RS.search(line):
            rules.append("id-test")
        if region == TEST and TEST_SKIP_RS.search(line):
            rules.append("test-skip")
        hits.extend(Hit(path, number, rule, HINTS[rule]) for rule in rules)
    return hosts + hits


def decode(data: bytes) -> str | None:
    """Text of a file, or None when it cannot be read as text. A BOM-marked UTF-16 file is decoded; undecodable bytes
    are replaced, not skipped, so a stray byte never hides a value on the same line."""
    if data.startswith((b"\xff\xfe", b"\xfe\xff")):
        return data.decode("utf-16", errors="replace")
    if b"\0" in data:
        return None
    return data.decode("utf-8", errors="replace")


def gitleaks(tree: Path, terms: list[str] | None, shapes: bool = True) -> list[Hit]:
    """gitleaks over a scratch tree, its findings by path relative to it. Nothing it finds is printed or kept."""
    exe = shutil.which("gitleaks")
    if exe is None:
        raise GuardError("gitleaks is not on PATH: run the guard through mise (`mise run gate:privacy`)")
    report = tree.parent / f"report-{tree.name}.json"
    env = {**os.environ, "GITLEAKS_CONFIG_TOML": gitleaks_config(terms, shapes)}
    env.pop("GITLEAKS_CONFIG", None)
    run = subprocess.run(
        [exe, "dir", ".", "--no-banner", "--redact", "--log-level", "error", "--exit-code", "0",
         "--report-format", "json", "--report-path", str(report)],
        cwd=tree, env=env, capture_output=True, text=True,
    )
    if run.returncode != 0 or not report.is_file():
        # Its own errors name the config, never a scanned value; the terms are an environment variable it never prints.
        raise GuardError(f"gitleaks failed ({run.returncode}): {run.stderr.strip()[-400:]}")
    found = json.loads(report.read_text(encoding="utf-8") or "[]")
    report.unlink()
    return [
        Hit(Path(f["File"]).as_posix().removeprefix("./"), f["StartLine"], f["RuleID"], f.get("Description") or SECRET_HINT)
        for f in found
    ]


def scan(files: dict[str, bytes | Path], terms: list[str] | None, regions: dict[str, list[str]] | None = None) -> list[Hit]:
    """Every file at its own path: bytes are written, a Path is linked (the tree's own files)."""
    hits: list[Hit] = []
    with tempfile.TemporaryDirectory(prefix="guard-", dir=scratch_parent()) as scratch:
        tree, fuzz = Path(scratch) / "tree", Path(scratch) / "fuzz"
        for path, content in files.items():
            data = content.read_bytes() if isinstance(content, Path) else content
            if path.startswith(FUZZ_INPUTS):
                target = fuzz / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
                continue
            target = tree / path
            target.parent.mkdir(parents=True, exist_ok=True)
            text = decode(data)
            if text is None:
                hits.append(Hit(path, 1, "unscannable", HINTS["unscannable"]))
                continue
            if isinstance(content, Path):
                try:
                    os.link(content, target)
                except OSError:
                    shutil.copyfile(content, target)
            else:
                target.write_bytes(data)
            hits.extend(own_rules(path, text, (regions or {}).get(path)))
        if tree.is_dir():
            hits.extend(gitleaks(tree, terms))
        if fuzz.is_dir():
            hits.extend(gitleaks(fuzz, terms, shapes=False))
    return sorted(set(hits), key=lambda h: (h.path, h.line, h.rule))


def scratch_parent() -> Path:
    """Inside the repository's build/, so a link to a tracked file stays on its file system."""
    parent = ROOT / "build" / "privacy"
    parent.mkdir(parents=True, exist_ok=True)
    return parent


# --- The allowlist of this script's own rules ----------------------------------------------------------------------


def load_allowlist() -> tuple[dict[tuple[str, str], int], list[str]]:
    """`path rule  # reason`, for this script's own rules (gitleaks' are in .gitleaks.toml). An entry without a reason,
    or that does not parse, is an error rather than an entry: a typo must not quietly allow nothing."""
    allowed: dict[tuple[str, str], int] = {}
    errors: list[str] = []
    if not ALLOW_FILE.is_file():
        return allowed, errors
    for number, raw in enumerate(ALLOW_FILE.read_text(encoding="utf-8").splitlines(), start=1):
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        body, _, reason = raw.partition("#")
        entry = body.split()
        if len(entry) != 2 or entry[1] not in OWN_RULES:
            errors.append(f"{ALLOW_PATH}:{number}: malformed allowlist entry")
        elif not reason.strip():
            errors.append(f"{ALLOW_PATH}:{number}: allowlist entry has no reason")
        else:
            allowed[(entry[0], entry[1])] = 0
    return allowed, errors


def allowed_by(allow: dict[tuple[str, str], int], hit: Hit) -> bool:
    for (path, rule) in allow:
        if rule == hit.rule and (hit.path == path or (path.endswith("/") and hit.path.startswith(path))):
            allow[(path, rule)] += 1
            return True
    return False


# --- Modes -------------------------------------------------------------------------------------------------------


def git(*args: str, root: Path = ROOT) -> bytes:
    return subprocess.run(["git", *args], cwd=root, check=True, capture_output=True).stdout


def tree_files(root: Path = ROOT) -> list[str]:
    """Every file git does not ignore, added or not."""
    listed = git("ls-files", "-z", "--cached", "--others", "--exclude-standard", "--deduplicate", root=root)
    return [p for p in listed.decode().split("\0") if p and (root / p).is_file() and not (root / p).is_symlink()]


def staged_files(root: Path = ROOT) -> list[str]:
    """Every file the index adds or changes. A submodule's pin is a commit, not a file: its own repository's guard
    reads what it holds."""
    stage = git("ls-files", "--stage", "-z", root=root).decode().split("\0")
    pins = {entry.split("\t", 1)[1] for entry in stage if entry.startswith("160000 ")}
    changed = git("diff", "--cached", "--name-only", "-z", "--diff-filter=ACMR", root=root).decode().split("\0")
    return [p for p in changed if p and p not in pins]


def terms_or_say() -> list[str] | None:
    """The private terms. Without them the run says so; in CI it fails, unless the workflow says the secret cannot
    reach this run (PRIVACY_TERMS_UNAVAILABLE: a fork's pull request, or a job the secret is kept from)."""
    terms = term_lines()
    if terms is None:
        if os.environ.get("CI") and not os.environ.get("PRIVACY_TERMS_UNAVAILABLE"):
            raise GuardError("no private terms in CI: set the PRIVACY_TERMS secret (README.md, 'Privacy guard')")
        print("guard: private terms NOT RUN -- no LAB.local.md or PRIVACY_TERMS here. This is a report, not a pass.")
    return terms


def report(hits: list[Hit]) -> int:
    for hit in hits:
        print(f"{hit.path}:{hit.line}: {hit.rule}")
    if not hits:
        return 0
    print(f"guard: {len(hits)} finding(s). The text is never printed; fix by rule:")
    for rule, hint in sorted({(h.rule, h.hint) for h in hits}):
        print(f"  {rule}: {hint}")
    return 1


def run_tree(staged: bool) -> int:
    terms = terms_or_say()
    allow, errors = load_allowlist()
    if staged:
        files: dict[str, bytes | Path] = {p: git("show", f":{p}") for p in staged_files()}
    else:
        files = {p: ROOT / p for p in tree_files()}
    hits = [h for h in scan(files, terms) if not allowed_by(allow, h)]
    status = report(hits)
    for error in errors:
        print(error)
    if not staged:
        stale = [key for key, used in allow.items() if used == 0]
        for path, rule in stale:
            print(f"guard: stale allowlist entry, remove it: {path} {rule}")
        status = status or (1 if stale else 0)
    return status or (1 if errors else 0)


def run_message(message_file: str) -> int:
    terms = terms_or_say()
    text = Path(message_file).read_text(encoding="utf-8")
    body = "\n".join("" if line.startswith("#") else line for line in text.splitlines())
    return report(scan({"COMMIT_MSG": body.encode()}, terms))


def run_hook() -> int:
    """An agent's edit, refused before it lands: exit 2 blocks the tool call and hands stderr back to the agent, which
    learns the rule and rewrites generically."""
    tool_input = (json.load(sys.stdin).get("tool_input")) or {}
    file_path = tool_input.get("file_path") or tool_input.get("notebook_path")
    if not file_path:
        return 0
    try:
        rel = Path(file_path).resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return 0
    if subprocess.run(["git", "check-ignore", "-q", "--no-index", rel], cwd=ROOT).returncode == 0:
        return 0
    whole = "content" in tool_input
    fragments: list[tuple[str, str]] = []
    if whole:
        fragments.append((tool_input["content"], ""))
    if "new_string" in tool_input:
        fragments.append((tool_input["new_string"], tool_input.get("old_string") or ""))
    fragments += [(e.get("new_string", ""), e.get("old_string") or "") for e in tool_input.get("edits") or []]
    if "new_source" in tool_input:
        fragments.append((tool_input["new_source"], ""))
    text = "\n".join(f for f, _ in fragments)
    regions = None
    if not whole:
        regions = {rel: [r for f, anchor in fragments for r in [fragment_region(rel, anchor)] * len(f.splitlines())]}
    allow, _ = load_allowlist()
    hits = [h for h in scan({rel: text.encode()}, term_lines(), regions) if not allowed_by(allow, h)]
    if not hits:
        return 0
    rules = sorted({(h.rule, h.hint) for h in hits})
    print(
        f"the privacy guard refused this edit to {rel}: "
        + "; ".join(f"{r} ({hint})" for r, hint in rules)
        + ". The matched text is not repeated here. Rewrite it as the rule's hint says.",
        file=sys.stderr,
    )
    return 2


def fragment_region(rel: str, anchor: str) -> str:
    """Whether an Edit's fragment lands in a file's test region."""
    if is_test_file(rel):
        return TEST
    target = ROOT / rel
    if not rel.endswith(".rs") or not target.is_file():
        return PROD
    current = target.read_text(encoding="utf-8", errors="replace")
    start = test_region_start(current.splitlines())
    position = current.find(anchor) if anchor else -1
    if start is None or position < 0:
        return PROD
    return TEST if current.count("\n", 0, position) >= start else PROD


def run_self_test() -> int:
    """Every rule still catches its shape, and clean text passes. Samples are assembled here rather than written as
    literals, so this file never trips its own rules and never holds a real value."""
    lap, top, own = "lap", "top", "own" + "er"
    dot = ".".join
    cases = [
        (f"measured on the {lap}{top}", "owner-wording"),
        (f"the {own}'s call", "owner-wording"),
        (f"on the {own}'s machine", "owner-wording"),
        (f"the {own} of this machine", "owner-wording"),
        (f"the {own} ran it", "owner-wording"),
        ("monitor " + "D" + "P-2", "connector"),
        ("monitor " + "Display" + "Port-1", "connector"),
        ("nic " + "en" + "p5s0", "connector"),
        ("host " + dot(["192", "168", "7", "9"]), "private-ip"),
        ("host " + dot(["100", "64", "1", "2"]), "private-ip"),
        ("host " + dot(["169", "254", "1", "2"]), "private-ip"),
        ("host " + "fd12" + ":3456::1", "private-ip"),
        ("host " + dot(["8", "26", "4", "19"]), "public-ip"),
        ("host " + "2a01" + ":4f8:1:2::3", "public-ip"),
        ("ssh to " + "nas" + ".lan", "private-host"),
        ("ssh to " + "printer" + ".home.arpa", "private-host"),
        ("path " + "/m" + "nt/disk/file", "personal-path"),
        ("path " + "/var/" + "home/someone/x", "personal-path"),
        ("path " + "/Us" + "ers/someone/x", "personal-path"),
        ("mail " + "someone" + "@" + "mail-provider.gr", "email"),
        ("mail " + "someone" + "@" + "workstation", "email"),
        ("mail " + "someone" + "@" + "feee3b6bb0d4c68370f256a4502498c9227e5c6b" + ".mail-provider.gr", "email"),
        # Made at run time from a fixed seed: no token, real or fake, is written in this file.
        ("token = ghp_" + "".join(random.Random(0).choices(string.ascii_letters + string.digits, k=36)), "github-pat"),
    ]
    rust_cases = [
        ("client/webrtc/src/x.rs", "self.set_voice_channel_id(" + "0);", "id-prod"),
        ("server/src/x.rs", "let u = User { user" + "_id: 42, };", "id-prod"),
        ("client/webrtc/src/x_tests.rs", "user" + "_id: 7,", "id-test"),
        ("client/webrtc/src/y_tests.rs", 'eprintln!("no encoder, ' + 'skipping");', "test-skip"),
        ("client/webrtc/src/z_tests.rs", 'const NAS: &str = "' + "nas" + '.lan";', "private-host"),
        ("client/webrtc/src/w.rs", "// ssh to " + "nas" + ".lan first", "private-host"),
        ("client/webrtc/src/v.rs", "/// the box at " + "printer" + ".home.arpa", "private-host"),
        ("client/webrtc/src/u.rs", 'let at = r#"' + "nas" + '.lan"#;', "private-host"),
        ("scripts/x.py", "host = '" + "nas" + ".lan'", "private-host"),
        ("scripts/y.mjs", "const host = `" + "nas" + ".internal`;", "private-host"),
        ("scripts/z.sh", "ssh " + "nas" + ".lan", "private-host"),
    ]
    clean = [
        "Machine B receives; the maintainer's call; an RX 9060 XT on Fedora 44",
        "only codecs this machine encodes; 203.0.113.7; 2001:db8::7; user@example.invalid; 127.0.0.1",
        "- uses: actions/checkout@v7",
        "- uses: owner/action@" + "feee3b6bb0d4c68370f256a4502498c9227e5c6b" + " # v0.7",
        "pin it as chatkord-server:0.1.0@" + "sha256:" + "ab" * 32,
        "NIST SP 800-63B-4 section " + dot(["3", "1", "1", "2"]) + "; libva " + dot(["2", "24", "1"]),
        "the owner role holds every word; the owner code is logged; Ask the owner of <server> to reset it",
        "the owner's role; an owner's join link; the owner can kick; a server owner; the owner may transfer",
        "LAB" + ".local.md is gitignored; localhost" + ".localdomain resolves here",
        "self.set_voice_channel_id(channel); user_id: ADA,",
    ]
    files: dict[str, bytes | Path] = {f"docs/case{i}.md": s.encode() for i, (s, _) in enumerate(cases)}
    files |= {path: s.encode() for path, s, _ in rust_cases}
    files |= {f"docs/clean{i}.md": s.encode() for i, s in enumerate(clean)}
    files["client/webrtc/src/clean.rs"] = (
        "self.set_voice_channel_id(channel);\nturn(&mut self." + "local, &mut x." + "internal, 'a', '\\'');\n"
        "fn f<'a>(s: &'a str) { s." + "lan(); }\n"
    ).encode()
    files["scripts/clean.py"] = ("self." + "local = state." + "internal  # no host here\n").encode()
    # A fuzz input: any address or host is the fuzzer's, a secret or a private term is still a leak.
    files[f"{FUZZ_INPUTS}x/clean"] = ("\0\x7f host " + dot(["8", "26", "4", "19"]) + " to " + "nas" + ".lan\0").encode()
    files[f"{FUZZ_INPUTS}x/secret"] = b"\0" + cases[-1][0].encode()
    files["client/webrtc/src/clean_tests.rs"] = b"user_id: ADA,\n"
    terms = term_lines()
    literals = [t for t in terms or [] if not t.startswith("re:")]
    # Built and matched in memory only; a term is never printed, only its index.
    files |= {f"docs/term{i}.md": f"a {t} b".encode() for i, t in enumerate(literals)}
    files |= {f"{FUZZ_INPUTS}term{i}": f"\0{t}\0".encode() for i, t in enumerate(literals)}
    found = {(h.path, h.rule) for h in scan(files, terms)}
    failures = [f"rule {rule} no longer catches its sample #{i}" for i, (_, rule) in enumerate(cases)
                if (f"docs/case{i}.md", rule) not in found]
    failures += [f"rule {rule} no longer catches {path}" for path, _, rule in rust_cases if (path, rule) not in found]
    failures += [f"clean sample {path} was flagged" for path, _ in found if "clean" in path]
    if (f"{FUZZ_INPUTS}x/secret", "github-pat") not in found:
        failures.append("a secret in a fuzz input is not caught")
    failures += [f"private term #{i + 1} is not caught in a fuzz input" for i in range(len(literals))
                 if (f"{FUZZ_INPUTS}term{i}", "private-term") not in found]
    failures += [f"private term #{i + 1} is not caught" for i in range(len(literals))
                 if (f"docs/term{i}.md", "private-term") not in found]
    # The tree scan must see a file that is not added yet, or the gate passes what the commit refuses.
    with tempfile.TemporaryDirectory() as scratch:
        git("init", "-q", root=Path(scratch))
        (Path(scratch) / "new.md").write_text("new\n", encoding="utf-8")
        if "new.md" not in tree_files(Path(scratch)):
            failures.append("the tree scan misses a file that is not added yet")
        # A staged submodule pin is no file to read, or every commit that moves one fails.
        git("add", "new.md", root=Path(scratch))
        git("update-index", "--add", "--cacheinfo", f"160000,{'1' * 40},pinned", root=Path(scratch))
        if staged_files(Path(scratch)) != ["new.md"]:
            failures.append("the staged scan reads a submodule's pin as a file")
    for failure in failures:
        print(f"self-test FAILED: {failure}")
    if terms is None:
        print("guard self-test: private terms NOT RUN -- no LAB.local.md or PRIVACY_TERMS here.")
    if failures:
        return 1
    print("guard self-test: every rule catches its shape")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--staged", action="store_true")
    mode.add_argument("--message", metavar="FILE")
    mode.add_argument("--hook", action="store_true")
    mode.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    try:
        if args.self_test:
            return run_self_test()
        if args.hook:
            return run_hook()
        if args.message:
            return run_message(args.message)
        return run_tree(staged=args.staged)
    except GuardError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
