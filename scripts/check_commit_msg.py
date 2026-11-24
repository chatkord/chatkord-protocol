#!/usr/bin/env python3
"""A commit's subject is a Conventional Commit of at most 72 characters. git's own merge
subjects, and fixup or squash subjects an interactive rebase folds away, pass as they are.

Usage: check_commit_msg.py FILE (the message, as the commit-msg hook is given it) | --self-test
"""
import re
import sys

MOST = 72
TYPES = ("build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test")
CONVENTIONAL = re.compile(rf"^(?P<type>{'|'.join(TYPES)})(?:\([\w./-]+\))?(?P<bang>!)?: \S")
# The Conventional Commits footer that marks a breaking change beside `!`.
BREAKING = re.compile(r"^BREAKING[ -]CHANGE: ", re.MULTILINE)
GIT_OWN = re.compile(r"^(?:Merge |fixup! |squash! |amend! )")


def subject(message):
    """The first line that is not a comment, as git keeps it."""
    for line in message.splitlines():
        if not line.startswith("#"):
            return line.rstrip()
    return ""


def kind(message):
    """The commit's type and whether it breaks, or None for a subject that is no Conventional Commit."""
    match = CONVENTIONAL.match(subject(message))
    if match is None:
        return None
    return match["type"], bool(match["bang"] or BREAKING.search(message))


def refusal(message):
    """Why the message is refused, or None."""
    line = subject(message)
    if GIT_OWN.match(line):
        return None
    if not CONVENTIONAL.match(line):
        return f"the subject is not a Conventional Commit ({', '.join(TYPES)}, an optional scope, then ': ')"
    if len(line) > MOST:
        return f"the subject is {len(line)} characters; at most {MOST}"
    return None


def self_test():
    passes = [
        "fix(voice): a late reply refuses only its ask",
        "docs: known issues say whether they block v1",
        "feat!: a major",
        "build(deps): " + "x" * 59,  # 72
        "# a comment git strips\nfix: after it",
        "Merge branch 'develop'",
        "fixup! fix: something",
    ]
    refused = [
        "build(deps): " + "x" * 60,  # 73
        "Fix the thing",
        "fix:no space",
        "feature: not a type",
        "",
    ]
    wrong = [m for m in passes if refusal(m)] + [m for m in refused if not refusal(m)]
    for message in wrong:
        print(f"check-commit-msg self-test: wrong answer for {message!r}")
    if not wrong:
        print("check-commit-msg self-test: every shape is answered")
    return 1 if wrong else 0


def main(args):
    if args == ["--self-test"]:
        return self_test()
    if len(args) != 1:
        print(__doc__.strip().splitlines()[-1])
        return 2
    with open(args[0], encoding="utf-8") as file:
        why = refusal(file.read())
    if why:
        print(f"commit-msg: {why}.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
