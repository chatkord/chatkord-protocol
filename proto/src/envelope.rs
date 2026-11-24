//! What both ends share about the envelope: the protocol version, the frame bound, how a frame is read and written,
//! how a close reason is carried as a WebSocket code, and how a reply names the request it answers.

use crate::chatkord_v1::wire_message::Body;
use crate::chatkord_v1::{Close, CloseReason, Error, ErrorCode, ProtoVersion, WireMessage};
use crate::entries::Uncounted;
use prost::Message;

/// The protocol this binary speaks, read from `wire/PROTOCOL` (`MAJOR.MINOR`) while compiling: what `ServerHello`
/// states and every opening exchange is compared against before an op is read.
pub const PROTO: ProtoVersion = match protocol(include_str!("../../wire/PROTOCOL").trim_ascii().as_bytes()) {
    Some(version) => version,
    None => panic!("wire/PROTOCOL must hold MAJOR.MINOR"),
};

/// `MAJOR.MINOR` in decimal, each part without a leading zero; anything else is `None`.
const fn protocol(text: &[u8]) -> Option<ProtoVersion> {
    let mut parts = [0u32; 2];
    let (mut part, mut digits, mut at) = (0, 0, 0);
    while at < text.len() {
        let byte = text[at];
        if byte == b'.' {
            if digits == 0 || part == 1 {
                return None;
            }
            (part, digits) = (1, 0);
        } else if byte.is_ascii_digit() {
            if digits == 1 && parts[part] == 0 {
                return None;
            }
            parts[part] = match parts[part].checked_mul(10) {
                Some(tens) => match tens.checked_add((byte - b'0') as u32) {
                    Some(value) => value,
                    None => return None,
                },
                None => return None,
            };
            digits += 1;
        } else {
            return None;
        }
        at += 1;
    }
    if part == 1 && digits > 0 { Some(ProtoVersion { major: parts[0], minor: parts[1] }) } else { None }
}

/// The most members one page of `ListPeople` holds; a page ends sooner where its members and their roles would pass half
/// a frame.
pub const PEOPLE_PAGE: usize = 500;

/// The longest `Event.cursor` a `Resume` may carry; a longer one is refused.
pub const MAX_CURSOR_BYTES: usize = 64;

/// The longest `Handshake.client_build` or `Handshake.platform` a server reads; a longer one is refused.
pub const MAX_HANDSHAKE_TEXT: usize = 64;

/// The most bytes one frame may carry, in either direction.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

/// The most values a frame may decode into, each message and each entry of a repeated field (`entries`): a frame of
/// empty entries past it is refused before it is decoded, as a frame past [`MAX_FRAME_BYTES`] is. The largest a server
/// sends (every channel, everyone in voice) holds a few thousand; past this one decodes into tens of megabytes.
pub const MAX_FRAME_ENTRIES: usize = 1 << 14;

/// The most asks one socket may leave unanswered: a client waits to send another, and a server refuses another with
/// `RATE_LIMITED`. A heartbeat, a `Resync` and a close are not asks.
pub const ASKS_IN_FLIGHT: usize = 8;

/// How often a client sends a `Heartbeat`, in seconds: how a connection that died without a close frame is found.
pub const HEARTBEAT_SECONDS: u64 = 20;

/// How often a client sends a `Heartbeat` while a voice call is joined, in seconds: the answer's timing is the round
/// trip a call shows when no media gives one (LiveKit's signal ping is 5 s as well). Silence is still judged by
/// [`SILENT_SECONDS`].
pub const IN_CALL_HEARTBEAT_SECONDS: u64 = 5;

/// How many beats may pass in silence before either end drops the other.
pub const HEARTBEATS_ALLOWED_SILENT: u64 = 3;

/// How long either end waits on a silent peer before it calls the connection dead.
pub const SILENT_SECONDS: u64 = HEARTBEAT_SECONDS * HEARTBEATS_ALLOWED_SILENT;

/// How long either end waits for a voice transport to connect its media, in seconds, from the client's first ask to
/// connect it: past it the call ends at both ends, and the client says which address and port it could not reach.
pub const MEDIA_CONNECT_SECONDS: u64 = 15;

/// The most one mediasoup parameters field may hold, in either direction: a real set is a few kilobytes.
pub const MAX_SFU_JSON_BYTES: usize = 16 * 1024;

/// The WebSocket close code of a reason is this base plus the reason's number, derived and never stored.
pub const CLOSE_BASE: u16 = 4000;

/// Whether `host` names only this machine: a loopback address in any spelling, or `localhost`. Only there may a
/// socket be plain text, since a credential crosses it.
pub fn is_local_authority(host: &str) -> bool {
    match host.parse::<std::net::IpAddr>() {
        Ok(address) => address.is_loopback(),
        Err(_) => host.eq_ignore_ascii_case("localhost"),
    }
}

/// The link a person is sent to join a server: `https://<host[:port]>/join#<code>`, or `http://` where `authority`
/// is this machine's, whose socket may be plain, so the link claims no protection it may not have.
pub fn join_link(authority: &str, code: &str) -> String {
    let host = match authority.strip_prefix('[') {
        Some(bracketed) => bracketed.split_once(']').map_or(bracketed, |(host, _)| host),
        None => authority.split_once(':').map_or(authority, |(host, _)| host),
    };
    let scheme = if is_local_authority(host) { "http" } else { "https" };
    format!("{scheme}://{authority}/join#{code}")
}

/// Where a webhook's posts are taken, before its secret: `POST /v1/webhooks/<token>`.
pub const WEBHOOK_PATH: &str = "/v1/webhooks/";

/// The URL a program posts a webhook's messages to: `https://<host[:port]>/v1/webhooks/<token>`, on the clients'
/// port. Both ends read this one shape.
pub fn webhook_url(authority: &str, token: &str) -> String {
    format!("https://{authority}{WEBHOOK_PATH}{token}")
}

/// A frame read: the op it carries, or the protocol's reason it was refused.
#[expect(
    clippy::large_enum_variant,
    reason = "a frame is read and moved once; a box would allocate for every op so a rare refusal is smaller"
)]
pub enum Frame {
    /// A frame whose op was read whole.
    Op(WireMessage),
    /// A frame that did not parse, carried no op, or whose op its door refused.
    Refused(Refused),
}

/// A refused frame: why, and the request it answers when the envelope was read far enough to know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused {
    /// What the refusal says.
    pub code: ErrorCode,
    /// The refused request's `seq`, or `None` when the frame did not decode that far.
    pub seq: Option<u64>,
}

impl Refused {
    const fn unreadable() -> Self {
        Self { code: ErrorCode::BadFrame, seq: None }
    }
}

/// Reads one frame whatever op it carries.
pub fn read_frame(bytes: &[u8]) -> Frame {
    read_admitted(bytes, |_| Ok(()))
}

/// Reads one frame through a door that judges its op by kind alone (an empty op of that kind) and refuses it with
/// its own code. The bounds are applied before the parse, so an oversized frame never reaches a decoder: a list past the
/// most its field states (`options.proto`'s `most`) is `INVALID_REQUEST`, as its op's own check would answer it.
pub fn read_admitted(bytes: &[u8], door: impl FnOnce(&Body) -> Result<(), ErrorCode>) -> Frame {
    if bytes.len() > MAX_FRAME_BYTES {
        return Frame::Refused(Refused::unreadable());
    }
    let Some(Envelope { seq, op, unknown_field }) = envelope(bytes) else {
        return Frame::Refused(Refused::unreadable());
    };
    let refused = |code| Frame::Refused(Refused { code, seq: Some(seq) });
    // A field this build does not know is an op from a newer minor, not a frame with nothing in it.
    let Some(op) = op else { return refused(if unknown_field { ErrorCode::UnknownOp } else { ErrorCode::BadFrame }) };
    if let Err(code) = door(&op) {
        return refused(code);
    }
    match crate::entries::entries(bytes) {
        Ok(entries) if entries > MAX_FRAME_ENTRIES => return refused(ErrorCode::BadFrame),
        Ok(_) => {}
        Err(Uncounted::PastMost) => return refused(ErrorCode::InvalidRequest),
        Err(Uncounted::Unreadable) => return Frame::Refused(Refused::unreadable()),
    }
    match WireMessage::decode(bytes) {
        Ok(message) if message.body.is_some() => Frame::Op(message),
        Ok(_) | Err(_) => Frame::Refused(Refused::unreadable()),
    }
}

/// A frame's envelope, read without its op's payload: the op is an empty one of the kind its field number names.
struct Envelope {
    seq: u64,
    op: Option<Body>,
    unknown_field: bool,
}

/// Walks a frame's top-level fields, skipping every payload; `None` when the frame is not a message. The kind of an
/// op field is the generated decoder's answer for an empty one, so field numbers have no table here; the last op
/// field wins, as it does in the decoder.
fn envelope(mut bytes: &[u8]) -> Option<Envelope> {
    let mut read = Envelope { seq: 0, op: None, unknown_field: false };
    while !bytes.is_empty() {
        let key = prost::decode_length_delimiter(&mut bytes).ok()?;
        let (number, wire) = (key >> 3, key & 0b111);
        match wire {
            0 => {
                let value = prost::decode_length_delimiter(&mut bytes).ok()?;
                if number == 1 {
                    read.seq = u64::try_from(value).ok()?;
                }
            }
            1 => bytes = bytes.get(8..)?,
            5 => bytes = bytes.get(4..)?,
            2 => {
                let length = prost::decode_length_delimiter(&mut bytes).ok()?;
                bytes = bytes.get(length..)?;
            }
            // A group, which this schema has none of, or no wire type at all.
            _ => return None,
        }
        // Past `seq` and `ack_of` every field is an op, or one this build does not know; an op's number with
        // another wire type is a frame the decoder refuses.
        match (number, empty_op(number << 3 | 2)?, wire) {
            (0, _, _) => return None,
            (1 | 2, _, _) => {}
            (_, Field::Op(op), 2) => read.op = Some(op),
            (_, Field::Op(_), _) => return None,
            (_, Field::Unknown, _) => read.unknown_field = true,
        }
    }
    Some(read)
}

/// What a top-level field number holds in this build's schema.
#[expect(clippy::large_enum_variant, reason = "made once a field and matched at once, never kept")]
enum Field {
    Op(Body),
    Unknown,
}

/// The op an empty length-delimited field with this key holds; `None` if the key cannot be written.
fn empty_op(key: usize) -> Option<Field> {
    let mut stub = [0u8; 11];
    let mut free = &mut stub[..];
    prost::encode_length_delimiter(key, &mut free).ok()?;
    let written = 11 - free.len();
    // The byte after the key is already the empty payload's length, zero.
    let body = WireMessage::decode(stub.get(..=written)?).ok().and_then(|message| message.body);
    Some(body.map_or(Field::Unknown, Field::Op))
}

/// What a frame costs its reader, against the bounds it is read under: its bytes ([`MAX_FRAME_BYTES`]) and the values
/// it decodes into ([`MAX_FRAME_ENTRIES`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    /// The frame's bytes.
    pub bytes: usize,
    /// Each message and each entry of a repeated field it decodes into.
    pub entries: usize,
}

impl Cost {
    /// The most one frame may cost, either way.
    pub const FRAME: Cost = Cost { bytes: MAX_FRAME_BYTES, entries: MAX_FRAME_ENTRIES };

    /// Both costs together.
    #[must_use]
    pub const fn plus(self, other: Cost) -> Cost {
        Cost { bytes: self.bytes.saturating_add(other.bytes), entries: self.entries.saturating_add(other.entries) }
    }

    /// This cost without `other`'s, never below none.
    #[must_use]
    pub const fn less(self, other: Cost) -> Cost {
        Cost { bytes: self.bytes.saturating_sub(other.bytes), entries: self.entries.saturating_sub(other.entries) }
    }

    /// Whether this cost is inside `bound`, both ways.
    pub const fn within(self, bound: Cost) -> bool {
        self.bytes <= bound.bytes && self.entries <= bound.entries
    }
}

/// What a frame carrying `op` costs, in the widest envelope a sender writes, its values counted by the same walk of the
/// schema a reader refuses a frame by; a frame that walk cannot read costs past every bound.
pub fn cost(op: &Body) -> Cost {
    let mut frame = WireMessage { seq: u64::MAX, ack_of: Some(u64::MAX), body: None }.encode_to_vec();
    op.encode(&mut frame);
    Cost { bytes: frame.len(), entries: crate::entries::entries(&frame).unwrap_or(usize::MAX) }
}

/// A message that would not fit in a frame the peer's own reader accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTooLarge;

/// Writes one frame, the envelope's own encoding; one over [`MAX_FRAME_BYTES`] is refused.
pub fn write_frame(message: &WireMessage) -> Result<Vec<u8>, FrameTooLarge> {
    if message.encoded_len() > MAX_FRAME_BYTES {
        return Err(FrameTooLarge);
    }
    Ok(message.encode_to_vec())
}

/// Whether a peer stating `stated` speaks this protocol: the same `major`, whatever its `minor`. A `major` ahead and
/// one behind are the same refusal.
pub fn speaks_this_protocol(stated: ProtoVersion) -> bool {
    stated.major == PROTO.major
}

/// [`speaks_this_protocol`] for a version that may be absent; an absent one is refused.
pub fn agrees_with_this_proto(stated: Option<ProtoVersion>) -> bool {
    match stated {
        Some(version) => speaks_this_protocol(version),
        None => false,
    }
}

/// The close frame that ends a session for `reason`.
pub fn close_frame(reason: CloseReason) -> Close {
    Close { reason: reason.into() }
}

/// The WebSocket code a reason is carried as.
pub fn close_code(reason: CloseReason) -> u16 {
    let number: i32 = reason.into();
    CLOSE_BASE.saturating_add(u16::try_from(number).unwrap_or(0))
}

/// The reason a WebSocket code carries, or none for a code Chatkord never sends.
pub fn close_reason_of_code(code: u16) -> Option<CloseReason> {
    let number = i32::from(code.checked_sub(CLOSE_BASE)?);
    match CloseReason::try_from(number) {
        Ok(CloseReason::Unspecified) | Err(_) => None,
        Ok(reason) => Some(reason),
    }
}

/// A close frame's reason; none for an absent one or one the schema does not have.
pub fn close_reason(close: Close) -> Option<CloseReason> {
    match CloseReason::try_from(close.reason) {
        Ok(CloseReason::Unspecified) | Err(_) => None,
        Ok(reason) => Some(reason),
    }
}

/// An error's code; none for an absent one or one the schema does not have, so it is never read as a real one.
pub fn code_of(error: &Error) -> Option<ErrorCode> {
    match ErrorCode::try_from(error.code) {
        Ok(ErrorCode::Unspecified) => None,
        Ok(code) => Some(code),
        Err(_) => None,
    }
}

/// The reply to `request`: its `ack_of` is the request's `seq`.
pub fn answered(request: &WireMessage, seq: u64, op: Body) -> WireMessage {
    WireMessage { seq, ack_of: Some(request.seq), body: Some(op) }
}

/// A message the far end sends without being asked: `ack_of` stays absent.
pub fn one_way(seq: u64, op: Body) -> WireMessage {
    WireMessage { seq, ack_of: None, body: Some(op) }
}

/// Every op the schema has, each empty, read from the schema rather than listed.
#[cfg(any(test, feature = "testing"))]
pub fn every_op() -> Vec<Body> {
    // Every number a two-byte key can carry; the schema's highest is far below.
    (3..=2047)
        .filter_map(|number| {
            let mut frame = Vec::new();
            prost::encoding::encode_key(number, prost::encoding::WireType::LengthDelimited, &mut frame);
            frame.push(0);
            WireMessage::decode(frame.as_slice()).ok()?.body
        })
        .collect()
}

/// The schema's field numbers of the message `op` carries, for a test that feeds each one.
#[cfg(any(test, feature = "testing"))]
pub fn fields_of(op: &Body) -> Vec<u32> {
    let schema = &*crate::schema::SCHEMA;
    let message = schema.op(op_number(op)).and_then(|at| schema.messages.get(at));
    let mut numbers: Vec<u32> = message.map(|message| message.fields.keys().copied().collect()).unwrap_or_default();
    numbers.sort_unstable();
    numbers
}

/// Whether the message `op` carries holds a repeated field that states no most (`options.proto`'s `most`), itself
/// or in a message it holds.
#[cfg(any(test, feature = "testing"))]
pub fn carries_unbounded_repeated(op: &Body) -> bool {
    let schema = &*crate::schema::SCHEMA;
    fn repeats(schema: &crate::schema::Schema, message: usize, seen: &mut Vec<usize>) -> bool {
        if seen.contains(&message) {
            return false;
        }
        seen.push(message);
        schema.messages.get(message).is_some_and(|message| {
            message.fields.values().any(|field| {
                (field.repeated && field.most.is_none())
                    || matches!(field.shape, crate::schema::Shape::Message(Some(held)) if repeats(schema, held, seen))
            })
        })
    }
    schema.op(op_number(op)).is_some_and(|at| repeats(schema, at, &mut Vec::new()))
}

/// The most entries the schema lets field `number` of the message `op` carries hold; none when it states none.
#[cfg(any(test, feature = "testing"))]
pub fn most_of(op: &Body, number: u32) -> Option<usize> {
    let schema = &*crate::schema::SCHEMA;
    let message = schema.op(op_number(op)).and_then(|at| schema.messages.get(at))?;
    message.fields.get(&number)?.most
}

/// The field numbers of the message field `number` of the message `op` carries holds, for a test that pads each entry
/// of a list with a field that message does not know.
#[cfg(any(test, feature = "testing"))]
pub fn entry_fields_of(op: &Body, number: u32) -> Vec<u32> {
    let schema = &*crate::schema::SCHEMA;
    let entry = schema.op(op_number(op)).and_then(|at| schema.messages.get(at)).and_then(|message| {
        match message.fields.get(&number)?.shape {
            crate::schema::Shape::Message(held) => schema.messages.get(held?),
            _ => None,
        }
    });
    let mut numbers: Vec<u32> = entry.map(|entry| entry.fields.keys().copied().collect()).unwrap_or_default();
    numbers.sort_unstable();
    numbers
}

#[cfg(any(test, feature = "testing"))]
fn op_number(op: &Body) -> u32 {
    let mut bytes = Vec::new();
    op.encode(&mut bytes);
    let mut key = bytes.as_slice();
    prost::decode_length_delimiter(&mut key).ok().and_then(|key| u32::try_from(key >> 3).ok()).unwrap_or_default()
}

#[cfg(test)]
mod tests {

    /// The frames a build of another major sends and reads at the opening, written out byte by byte: their numbers
    /// keep their meaning in every major, so that build is read and told `VERSION_MISMATCH` and which end to update.
    #[test]
    fn an_opening_of_another_major_is_read_and_answered_with_version_mismatch() {
        use crate::chatkord_v1::{Close, CloseReason, Error, ErrorCode, Handshake, ProtoVersion, ServerHello};
        // seq 1, then `Handshake` (3) stating 2.5.
        let theirs = [0x08, 0x01, 0x1a, 0x06, 0x0a, 0x04, 0x08, 0x02, 0x10, 0x05];
        let read = WireMessage::decode(theirs.as_slice()).expect("another major's opening decodes");
        let Some(Body::Handshake(Handshake { proto_version: Some(stated), .. })) = read.body else {
            panic!("not read as its handshake: {read:?}")
        };
        assert_eq!(stated, ProtoVersion { major: 2, minor: 5 });
        assert!(!speaks_this_protocol(stated), "another major");
        let answer = |body| WireMessage { seq: 0, ack_of: Some(1), body: Some(body) }.encode_to_vec();
        let hello = ServerHello { proto_version: Some(ProtoVersion { major: 3, minor: 0 }), ..ServerHello::default() };
        // `ack_of` (2), then `ServerHello` (4) carrying its version (1).
        assert_eq!(answer(Body::ServerHello(hello)), [0x10, 0x01, 0x22, 0x04, 0x0a, 0x02, 0x08, 0x03]);
        // `Error` (5) with its code (1) `PROTO_VERSION_MISMATCH` (1).
        let refused = Error { code: ErrorCode::ProtoVersionMismatch.into() };
        assert_eq!(answer(Body::Error(refused)), [0x10, 0x01, 0x2a, 0x02, 0x08, 0x01]);
        // `Close` (6) with its reason (1) `VERSION_MISMATCH` (2).
        let close = Close { reason: CloseReason::VersionMismatch.into() };
        let ending = WireMessage { seq: 0, ack_of: None, body: Some(Body::Close(close)) }.encode_to_vec();
        assert_eq!(ending, [0x32, 0x02, 0x08, 0x02]);
    }

    #[test]
    fn the_protocol_is_read_as_major_dot_minor_and_nothing_else() {
        use super::protocol;
        assert_eq!(protocol(b"1.9"), Some(ProtoVersion { major: 1, minor: 9 }));
        assert_eq!(protocol(b"12.0"), Some(ProtoVersion { major: 12, minor: 0 }));
        for wrong in [&b""[..], b"1", b"1.", b".9", b"1.9.0", b"01.9", b"1.09", b"1,9", b" 1.9", b"4294967296.0"] {
            assert_eq!(protocol(wrong), None, "{:?}", String::from_utf8_lossy(wrong));
        }
    }
    use super::*;
    use crate::chatkord_v1::wire_message::Kind;
    use crate::chatkord_v1::{Heartbeat, SfuOffer, VoiceState};

    #[test]
    fn every_op_has_a_kind_of_its_own_named_after_its_message() {
        use crate::chatkord_v1::ChannelRename;
        use crate::chatkord_v1::wire_message::Kind;
        let ops = every_op();
        let kinds: std::collections::HashSet<Kind> = ops.iter().map(Kind::from).collect();
        assert_eq!(kinds.len(), ops.len(), "no two ops share a kind");
        let named: &'static str = Kind::from(&Body::ChannelRename(ChannelRename::default())).into();
        assert_eq!(named, "ChannelRename");
    }

    #[test]
    fn the_silence_window_is_three_beats() {
        assert_eq!(SILENT_SECONDS, 60);
    }

    #[test]
    fn only_an_address_this_machine_can_be_reached_by_is_local() {
        for local in ["127.0.0.1", "::1", "localhost", "LOCALHOST"] {
            assert!(is_local_authority(local), "{local}");
        }
        for other in
            ["", "0.0.0.0", "192.0.2.7", "example.invalid", "127.0.0.1.example.invalid", "localhost.localdomain"]
        {
            assert!(!is_local_authority(other), "{other}");
        }
    }

    const FIRST_SEQ: u64 = 1;
    const NEXT_SEQ: u64 = 2;
    const FIRST_NONCE: u64 = 7;
    const FOREIGN_CLOSE_CODE: u16 = 1000;
    const UNSCHEMATA_CODE: i32 = 999;

    fn opened_exchange() -> WireMessage {
        WireMessage { seq: FIRST_SEQ, ack_of: None, body: Some(Body::Heartbeat(Heartbeat { nonce: FIRST_NONCE })) }
    }

    #[test]
    fn a_join_link_names_the_server_and_the_code() {
        assert_eq!(join_link("example.invalid:8443", "c0de"), "https://example.invalid:8443/join#c0de");
        assert_eq!(join_link("example.invalid", "c0de"), "https://example.invalid/join#c0de");
        assert_eq!(join_link("[2001:db8::1]:8443", "c0de"), "https://[2001:db8::1]:8443/join#c0de");
    }

    #[test]
    fn a_join_link_to_this_machine_claims_no_protection_its_plain_socket_may_lack() {
        for (authority, link) in [
            ("127.0.0.1:18443", "http://127.0.0.1:18443/join#c0de"),
            ("[::1]:18443", "http://[::1]:18443/join#c0de"),
            ("localhost", "http://localhost/join#c0de"),
            ("LOCALHOST:8443", "http://LOCALHOST:8443/join#c0de"),
            ("localhost.example.invalid:8443", "https://localhost.example.invalid:8443/join#c0de"),
        ] {
            assert_eq!(join_link(authority, "c0de"), link, "{authority}");
        }
    }

    #[test]
    fn a_webhook_url_names_the_server_and_the_token_under_its_path() {
        let url = webhook_url("example.invalid:8443", "t0ken");
        assert_eq!(url, "https://example.invalid:8443/v1/webhooks/t0ken");
        assert!(url.contains(WEBHOOK_PATH));
    }

    #[test]
    fn only_the_same_major_speaks_this_protocol() {
        let other = |major, minor| ProtoVersion { major, minor };
        assert!(speaks_this_protocol(PROTO));
        assert!(speaks_this_protocol(other(PROTO.major, PROTO.minor + 5)));
        assert!(!speaks_this_protocol(other(PROTO.major + 1, 0)));
        assert!(!speaks_this_protocol(other(PROTO.major.saturating_sub(1), 0)));
        assert!(!agrees_with_this_proto(None));
    }

    #[test]
    fn a_frame_that_carries_no_op_is_refused_and_says_which_request() {
        let bare = WireMessage { seq: FIRST_SEQ, ack_of: None, body: None };
        let bytes = write_frame(&bare).expect("fits");
        match read_frame(&bytes) {
            Frame::Refused(refused) => assert_eq!(refused, Refused { code: ErrorCode::BadFrame, seq: Some(FIRST_SEQ) }),
            Frame::Op(_) => panic!("a frame with no op read as an op"),
        }
    }

    #[test]
    fn an_op_this_build_does_not_know_is_unknown_not_bad() {
        // seq = 5, then field 999 (length-delimited), which no build of this schema has.
        let mut newer = vec![0x08, 0x05];
        newer.extend_from_slice(&[0xBA, 0x3E, 0x02, 0x0A, 0x00]);
        match read_frame(&newer) {
            Frame::Refused(refused) => assert_eq!(refused, Refused { code: ErrorCode::UnknownOp, seq: Some(5) }),
            Frame::Op(_) => panic!("an op this build cannot know read as an op"),
        }
    }

    #[test]
    fn a_door_judges_the_op_by_its_kind_before_its_payload_is_read() {
        // seq = 5, then `ChannelsList`'s field whose payload is not a message at all.
        let mut frame = vec![0x08, 0x05];
        let number = op_number(&Body::ChannelsList(crate::chatkord_v1::ChannelsList {}));
        prost::encoding::encode_key(number, prost::encoding::WireType::LengthDelimited, &mut frame);
        frame.extend_from_slice(&[0x02, 0xFF, 0xFF]);
        let mut judged = None;
        let read = read_admitted(&frame, |op| {
            judged = Some(Kind::from(op));
            Err(ErrorCode::Unauthenticated)
        });
        assert_eq!(judged, Some(Kind::ChannelsList));
        match read {
            Frame::Refused(refused) => {
                assert_eq!(refused, Refused { code: ErrorCode::Unauthenticated, seq: Some(5) });
            }
            Frame::Op(_) => panic!("a refused op read as an op"),
        }
        assert!(matches!(read_frame(&frame), Frame::Refused(Refused { code: ErrorCode::BadFrame, seq: None })));
    }

    #[test]
    fn a_frame_that_cannot_be_parsed_is_refused_without_a_seq() {
        let truncated = [0x08, 0x80];
        match read_frame(&truncated[..]) {
            Frame::Refused(refused) => assert_eq!(refused, Refused { code: ErrorCode::BadFrame, seq: None }),
            Frame::Op(_) => panic!("an unparsable frame read as an op"),
        }
    }

    #[test]
    fn a_frame_over_the_bound_is_refused_before_the_parse() {
        let oversized = vec![0u8; MAX_FRAME_BYTES + 1];
        match read_frame(&oversized) {
            Frame::Refused(refused) => assert_eq!(refused, Refused { code: ErrorCode::BadFrame, seq: None }),
            Frame::Op(_) => panic!("an oversized frame read as an op"),
        }
    }

    #[test]
    fn a_message_the_peer_would_refuse_is_not_written() {
        let big = Body::VoiceState(VoiceState {
            send: Some(SfuOffer { transport: vec![0; MAX_FRAME_BYTES], router_rtp: Vec::new() }),
            ..VoiceState::default()
        });
        let message = WireMessage { seq: FIRST_SEQ, ack_of: None, body: Some(big) };
        assert_eq!(write_frame(&message), Err(FrameTooLarge));
    }

    #[test]
    fn a_frame_s_cost_is_what_its_reader_counts_and_refuses_it_by() {
        use crate::chatkord_v1::{People, Person};
        let held = |roles: usize| Person { role_ids: vec!["1".into(); roles], ..Person::default() };
        let page = |people: Vec<Person>| Body::People(People { entries: people, ..People::default() });
        // The op, then each person and each of their roles.
        assert_eq!(cost(&page(vec![held(2), held(3)])).entries, 1 + 2 + 5);
        let fits = page(vec![held(MAX_FRAME_ENTRIES - 2)]);
        let over = page(vec![held(MAX_FRAME_ENTRIES - 1)]);
        for (op, within) in [(fits, true), (over, false)] {
            let spent = cost(&op);
            assert_eq!(spent.within(Cost::FRAME), within, "{} entries", spent.entries);
            let frame = WireMessage { seq: u64::MAX, ack_of: Some(u64::MAX), body: Some(op) }.encode_to_vec();
            assert_eq!(spent.bytes, frame.len());
            assert_eq!(matches!(read_frame(&frame), Frame::Op(_)), within, "the reader agrees");
        }
        let one = Cost { bytes: 3, entries: 1 };
        assert_eq!(one.plus(one).less(one), one);
        assert_eq!(one.less(one.plus(one)), Cost::default(), "never below none");
    }

    #[test]
    fn a_one_way_message_leaves_ack_of_absent() {
        let announced = one_way(FIRST_SEQ, Body::Heartbeat(Heartbeat { nonce: FIRST_NONCE }));
        assert!(announced.ack_of.is_none());
    }

    #[test]
    fn a_result_names_the_call_it_answers() {
        let asked = opened_exchange();
        let reply = answered(&asked, NEXT_SEQ, Body::Heartbeat(Heartbeat { nonce: FIRST_NONCE }));
        assert_eq!(reply.ack_of, Some(FIRST_SEQ));
        assert_eq!(reply.seq, NEXT_SEQ);
    }

    #[test]
    fn a_close_code_is_the_reasons_own_number_in_chatkords_range_and_reads_back() {
        let ended = close_frame(CloseReason::VersionMismatch);
        assert_eq!(ended.reason, i32::from(CloseReason::VersionMismatch));
        assert_eq!(close_reason(ended), Some(CloseReason::VersionMismatch));
        assert_eq!(close_code(CloseReason::VersionMismatch), 4002);
        assert_eq!(close_code(CloseReason::SignedOut), 4007);
        assert_eq!(close_code(CloseReason::SessionExpired), 4008);
        assert_eq!(close_code(CloseReason::ServerStopping), 4009);
        for reason in [
            CloseReason::Normal,
            CloseReason::VersionMismatch,
            CloseReason::Kicked,
            CloseReason::Unreachable,
            CloseReason::ServerError,
            CloseReason::ClientShutdown,
            CloseReason::SignedOut,
            CloseReason::SessionExpired,
            CloseReason::ServerStopping,
        ] {
            // Below 1000, so the code stays in the 4000-4999 range RFC 6455 leaves to applications.
            assert!((CLOSE_BASE + 1..5000).contains(&close_code(reason)), "{reason:?}");
            assert_eq!(close_reason_of_code(close_code(reason)), Some(reason));
        }
    }

    #[test]
    fn a_code_chatkord_never_sends_is_no_reason() {
        for code in [FOREIGN_CLOSE_CODE, 1006, CLOSE_BASE, 3999, 4999, u16::MAX] {
            assert_eq!(close_reason_of_code(code), None, "{code}");
        }
        assert!(close_reason(Close { reason: 99 }).is_none());
        assert!(close_reason(Close { reason: 0 }).is_none());
    }

    #[test]
    fn a_code_outside_the_schema_is_never_borrowed_as_a_real_one() {
        let invented = Error { code: UNSCHEMATA_CODE };
        assert!(code_of(&invented).is_none());

        let absent = Error { code: ErrorCode::Unspecified.into() };
        assert!(code_of(&absent).is_none());

        let real = Error { code: ErrorCode::RateLimited.into() };
        assert_eq!(code_of(&real), Some(ErrorCode::RateLimited));
    }

    #[test]
    fn the_walk_finds_every_op_once() {
        let ops = super::every_op();
        let mut names: Vec<String> =
            ops.iter().map(|op| format!("{op:?}").split('(').next().unwrap_or_default().to_string()).collect();
        let found = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), found, "each op once");
        // Fewer than the schema's 53 means the walk read nothing.
        assert!(found >= 53, "the walk found {found}");
    }
}
