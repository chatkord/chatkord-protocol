# Chatkord protocol

What the Chatkord client and server share, and nothing else: the wire schema both generate from, the Rust types and
rules both ends read, and what a log line or a debug report may hold.

| Path | What |
|---|---|
| `wire/` | The schema: `chatkord/v1/*.proto`, its version (`PROTOCOL`) and the ledger of majors (`MAJORS`); [its rules](wire/README.md) |
| `proto/` | The generated wire types (`gen/`, checked in, never hand-edited) and the rules both ends read: envelope, names and grants, SFU parameters (and the one `mediasoup-types` release, re-exported), files |
| `report/` | `Public` facts, the typed log, the recent notes and the report; `report/derive` is its one derive |
| `fuzz/` | The fuzz target of the server's frame as a client reads it, and its seeds |
| `scripts/` | The gate's tools, which the apps run on their own trees too |

## How an app takes it

Each app adds this repository as a git submodule at `protocol/`, pinned to a commit, and depends on the crates by
path (`protocol/proto`, `protocol/report`). A protocol change lands here first, gated; an app moves its pin when it
adopts the change. An additive change is a `minor`, which a peer on another minor tolerates, and a feature that needs
it is a member of `Feature`, offered in `ServerHello.features` and used only when offered; a removal, renumbering or
rename is a `major`, which ends the session of a peer on another ([versions](wire/README.md#versions)).

An app runs the shared tools as `protocol/scripts/...` from its own root: they work on the tree they are started in and
find their own files beside themselves. The lint table, `clippy.toml`, `rustfmt.toml` and `rust-toolchain.toml` of this
repository are the apps' too, and each app's gate holds its copies equal; an app's own clippy rules are its root
`clippy.extra.toml`, added to this `clippy.toml` in the one its root keeps.

## The gate

`mise install` installs every tool at the version `mise.toml` pins; the Rust toolchain is `rust-toolchain.toml`'s, and
`mise run hooks` installs the git hooks.

- `mise run gate:fast` before every commit: the hooks, the privacy guard, formatting, the schema (`buf lint`, and
  `buf breaking` against the commit that opened the current major and the latest `v*` release tag, a break passing only
  with a new major listed in `wire/MAJORS`), the generated types compared with `wire/`, links, the lint table,
  `cargo deny` over the lock, and the gate's own self-tests.
- `mise run gate` on demand, and before anything reaches `main`: those, then clippy, `cargo nextest run`, `cargo doc`
  with warnings denied, and the lints of the fuzz target (Linux). A pass on a clean tree is recorded
  (`build/gate/passed/`, `scripts/gate/record.py`), and the pre-push hook refuses a push to `main` whose tip has none;
  `develop` and other branches are pushed without one.
- `mise run fuzz [TARGET] [SECONDS]` fuzzes `frame_from_server` on Linux; the gate replays its seeds in `proto`'s tests.

CI runs `mise run gate` on Linux and Windows for every pull request. `.github/rulesets/` is what GitHub enforces
(`gh api -X POST repos/<owner>/<repo>/rulesets --input <file>`): `main` changes only by a merge commit of a pull
request both gate jobs passed, and neither `main` nor `develop` is ever rewritten or deleted.

## Privacy guard

A repository that takes these tools reads the same to anyone running any Chatkord server, so nothing in it names a real
host, address, account, device or personal path, and no secret is in it. `scripts/privacy/run.sh` runs the guard on the
tree it is started in, from the gate, the git hooks, and an agent's edit hook; a finding prints `file:line: rule`, never
the matched text.

- gitleaks applies its own secret rules and the shapes in `.gitleaks.toml`.
- `scripts/privacy/guard.py` adds what a pattern cannot say: an id standing in by `0`, `-1` or a bare number (an
  unknown id is an absent field, and a test fixture names its ids, `const ADA: &str = "ada"`), a test skipped without
  saying so, a name only a private network resolves, and a file that is not text.
- Private terms (the names to keep out) are read at run time, never written to a file: the list items under
  `## Private terms` of `LAB.local.md`, looked up in the tree's root and else in `../chatkord-dev/`, or the
  `PRIVACY_TERMS` environment variable, which CI sets from a secret. With neither, the run says the terms did not run;
  in CI it fails unless the workflow says the secret cannot be had (a fork's pull request).
- The shapes are this repository's alone. A repository that runs the guard adds only allowances, each with its
  one-line reason: for the shapes in its `scripts/privacy/allow.gitleaks.toml` (top-level `[[allowlists]]`,
  `targetRules` naming a shape), joined to `.gitleaks.toml` at run time; for the script's own rules in its
  `scripts/privacy/allow.txt` (a stale one fails).

## Licence

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. A contribution is made under the same terms.
