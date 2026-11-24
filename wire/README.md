# The wire schema

`wire/` is the single truth for everything the client and server say to each other: both generate from it, through
the `proto` crate of this repository, which each app holds as a submodule at `protocol/`, and an op not in `wire.proto`
can be sent by neither.

## Layout

| File | Owns |
|---|---|
| `PROTOCOL` | The protocol's `MAJOR.MINOR` ([Versions](#versions)). |
| `MAJORS` | The ledger of every major the protocol has used, one a line, which the schema gate reads ([Versions](#versions)). |
| `chatkord/v1/options.proto` | What the schema says beyond types, read from the generated descriptor: a field's `secret` and `most`, an op's `sent_by` and an ask's `reply`. |
| `chatkord/v1/shared.proto` | Version handshake, the `Feature` enum, `Ok`, `Heartbeat`, the error and close enums. Imports only `options.proto`. |
| `chatkord/v1/auth.proto` | Identity: password or token sign-in, registration with an invite and the check before it, how a session ends, the name others see, invites, a kick, a ban, a password reset, leaving the server, the member and role lists, roles made, edited, moved and given words, giving or taking a role, handing ownership on. |
| `chatkord/v1/people.proto` | The server's members as every member sees them: the directory, who is signed in and how each is shown (`Status`), the account's own choice (`SetStatus`, `StatusChosen`) and the changes told (`PresenceChanged`). |
| `chatkord/v1/channel.proto` | Channels and their categories, as each account may see them: listed, made, renamed, moved and deleted, and what a role may do in one channel (its overwrites). One install is one server, so there is no server op. |
| `chatkord/v1/chat.proto` | Messages in a timeline: sending, editing and deleting one with its spans, files and reply, a page of history, the changes since a revision, reactions, pins, who is writing, threads made by a first reply to a message (and following them), a search of the messages the asker reads by their words and their files' names, a link preview a message carries, an app as an author, the standing of a timeline no channel owns, and the events each session that may read the timeline is told. |
| `chatkord/v1/conversation.proto` | Conversations: private talk between members of the server, in a timeline of its own: opening one, listing the asker's, and the event that tells a member one is listed for them. |
| `chatkord/v1/emoji.proto` | The server's own emoji: added, renamed, deleted and listed, and the events every member is told. |
| `chatkord/v1/notify.proto` | An account's notification settings for the server and each channel: a level, suppressions, a mute, set and read whole; and which device of an account alerts (`Activity`, `DeviceInUse`). |
| `chatkord/v1/voice.proto` | Voice: joining, producing and taking tracks, the SFU's opaque surface. |
| `chatkord/v1/events.proto` | One-way announcements, the only place stream lifetime is decided: what every signed-in session is told whatever room it is in (each change of a channel it may see and of the categories, who is in each voice room it may see, a member's new name or colour, the roles and the server's emoji changing, the server's name and colour), and an account's own standing when its roles or sight change, its own reactions and its notification settings. |
| `chatkord/v1/server.proto` | The server itself as people see it: its name, colour and picture, its debug report, its feature switches and its hosting facts (public address and the files an owner allows), set by an admin. |
| `chatkord/v1/webhook.proto` | Incoming webhooks: made, listed, regenerated and deleted, and the secret shown once. |
| `chatkord/v1/wire.proto` | `WireMessage`: every op named exactly once, plus the framing facts. |

## Timeline owners

Chat names a **timeline**, never a channel. A timeline has exactly one owner, and the owner decides who reads it, who
writes in it and who is told of it; no op changes with a new owner:

| Owner | Who reads and writes | Who is told |
|---|---|---|
| A channel (`ChannelSummary.timeline_id`) | The channel's words, by roles and overwrites | The sessions that see the channel |
| A conversation (`Conversation.timeline_id`) | Its members alone, a fixed set of words, no role's | Its members' sessions |
| A thread (`Message.thread`'s timeline), made by a first reply to a message of a channel | The channel's words | The channel's readers |

`MarkRead` answers `Ok` for all three and tells the account's sockets its standing in whatever owns the timeline:
`ChannelStanding` for a channel, `TimelineStanding` for a conversation or a thread (and for a followed thread its
channel's too). An administrator can switch some parts of the server off (`Switch`, `SetServerSwitch`): the greeting
says which are off (`ServerHello.switched_off`), and `ServerChanged.switched_off` tells every session a change. A
payload too large for a frame (a server's debug
report, up to `report::MAX_REPORT_BYTES`) is no field: the reply carries a secret one-use id, and the bytes are
downloaded over HTTP as a file's are (`GET /v1/files/reports/<id>`, the same session).

## Rules the schema enforces

- **Field numbers are pinned, never implied.** Every field, oneof arm and enum member carries its number: a message's
  fields run from 1 and an enum's values from 0, each in reading order. An additive field takes the next number;
  inside a major, `reserved` marks a number or name once on the wire, so it is never reused. A new major packs the
  numbers again and drops its `reserved`, except the opening's: `WireMessage`'s 1 to 6, the field 1 of `Handshake`,
  `ServerHello`, `Error` and `Close`, `ErrorCode.PROTO_VERSION_MISMATCH` and `CloseReason.VERSION_MISMATCH`, which a
  build of another major reads to say which end to update
  (`an_opening_of_another_major_is_read_and_answered_with_version_mismatch`). A renumbering is a `major` change;
  review it with `buf breaking --against .git#subdir=wire`.
  `buf.yaml` leaves that command's two reservation rules out: removing a spare run is no wire fact, and buf 1.75.0's
  WIRE set would report each one as a change.
- **Ids are opaque strings.** An unknown id is an absent field, never `""`, `"0"` or `-1`; an id the server keeps as a
  number is stringified at the boundary, so no client leans on its order or value. An id has one spelling, the one the
  server sent: any other (`"007"` for `"7"`) is no id, and every op refuses it as `INVALID_REQUEST`, where an id
  nobody holds is `NOT_FOUND`.
- **An absent enum reads as `*_UNSPECIFIED`, never as a real value.** Each enum's zero member carries its own name
  (the same bare name in two enums of one package is a codegen collision buf reports), so an enum the far end does
  not know reads as unknown rather than as the first real member.
- **`Heartbeat` is the client's own liveness fact**, not a server-internal keepalive: a connection that died without
  a close frame is something the client acts on, not a 30-second silence
  (a socket whose peer address stopped existing is found by the client's own check, not waited for).
- **A repeated field a client sends states its most** (`[(most) = N]`). The server decodes an op's payload only once
  its kind says the socket may send it, and it then costs about its own
  bytes; a list without a most would let a frame of empty entries cost many times its size. A frame holding more
  entries than a list's most is refused `INVALID_REQUEST` before it is decoded (`proto::entries`), and
  `proto/tests/decode_cost.rs` reads each op's fields from the descriptor, fails for a list a client sends
  that states none, and fills each list to its most. A server's lists state none, so both ends count a frame's
  messages and repeated entries before it is decoded and refuse one past `envelope::MAX_FRAME_ENTRIES`, the schema's
  shape read from the descriptor codegen writes beside the types (`file_descriptor_set` in `buf.gen.yaml`).
- **Every op says who sends it** (`option (sent_by)`), and a client's op that is answered names its answer
  (`option (reply)`): that makes it an ask, which holds a slot of the asks in flight; a client's op with none is a
  tell, answered by nothing. `proto/gen/ops.rs` is generated from them (`proto::ops`: `Ask`, `Tell`, `sent_by`,
  `reply_of`), so `Link::ask` returns its reply's type, the server refuses an answer the schema does not name
  `INTERNAL`, and no end matches replies by hand.
- **A secret is marked in the schema** (`[(secret) = true]`): a password, a token, an invite code. No generated type
  prints one, and `proto::redact`'s test, walking every op from the descriptor, fails for a marked field it can read
  back or a plain text field it cannot.
- **Close reasons are values here**, not codes smuggled through a WebSocket library.
- **The SFU's parameters are bytes: mediasoup's own JSON.** mediasoup speaks typed parameters (ICE, DTLS, RTP), not
  SDP. They travel as its JSON in `SfuOffer` (one per transport: `VoiceState.send` and `receive`), `SfuAnswer`
  (naming its `lane`), `SfuIce` and `VoiceProduce`, at most 16 KiB each. The server parses them once, at the edge, into
  mediasoup's types and refuses what does not parse; the client's validated bridge turns them into the offer and
  answer libwebrtc needs. The shape is one fact, the pinned mediasoup's.
- **A release fixes what a peer may rely on.** v1 holds only what is served: an op or field nothing sends or reads is
  removed: in place before a release, and from one on in the next major ([Versions](#versions)), within a major its
  number and name `reserved` until then.

## Versions

`wire/PROTOCOL` holds the protocol's `MAJOR.MINOR`, the value `ProtoVersion` carries; `proto::envelope::PROTO` is
read from it while compiling, and no copy is written elsewhere. Both ends compare it. A change that only adds bumps
the `minor`, which a peer on another minor tolerates; a feature that needs it is a member of `Feature`, offered in
`ServerHello.features` and used only when offered, and a member this end does not know is no feature. A major's first
minor offers none. A change that removes, renumbers or renames bumps the `major`, and a peer on another `major` ends
the session (`CloseReason.VERSION_MISMATCH`), told the server's own version first so its client says which end to
update.

A major is never used twice: `wire/MAJORS` lists every major the protocol has used, each above the one before, and
only gains lines; `wire/PROTOCOL`'s must be one of them. The gate runs `buf breaking` (the WIRE set) against the
`wire/` of the commit that opened the current major (the first whose ledger lists it: the root commit, for major 1;
HEAD, for a major this tree opens); a break passes only when `wire/PROTOCOL`'s major is above every other line of
`wire/MAJORS` and that commit's ledger does not list it yet, so a breaking change raises the major and lists it in the
same commit ([the gate](../README.md#the-gate)). An app pins a commit of this repository, so whatever it shipped is
within the major that commit opened. This repository has no release version of its own.

## Lint and breaking

`wire/buf.yaml` holds the schema, under the `buf` `mise.toml` pins, to `lint.use: [BASIC]` and `breaking.use: [WIRE]`,
the set `scripts/gate/schema.sh` checks against the major's opening commit ([Versions](#versions)).
BASIC on purpose: STANDARD's enum-naming rules want members renamed, and a rename is a `major` change on a protocol
whose names are the facts. The rule no lint expresses (`MESSAGES_ARE_NEVER_ENUMS`) is kept by review: an op's body is
a named message, and an enum is a value inside one, never a message field a plane could mistake for a message.

## Codegen

`buf.gen.yaml` is the one pass: `protoc-gen-prost` (from crates.io, run locally) generates Rust into `proto/`, and
both apps depend on that one crate; no other copy is generated, since a second would be two places for one truth. How
the gate checks the generated code is [the gate](../README.md#the-gate)'s.

The pass also derives from the op oneof `wire_message::Kind`: one fieldless variant per op, named after its message
(`Kind::from(&body)`), by which a log line or a counter labels an op; a new op has its kind the day it is added, and
nothing keeps a list of op names. An op's arm carries no comment of its own: its message's comment is its contract,
and the pass allows the arms (and `Kind`'s) `missing_docs`.
