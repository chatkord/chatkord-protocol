//! What a message holds beside its text, as both ends check it: the spans that name what needs an id, its files, its
//! reactions and pins, the counts a channel shows and the pace a timeline is told who is writing at.

use super::emoji::unicode_emoji;
use super::link::LinkTarget;
use crate::chatkord_v1::{ReactionKey, Span, reaction_key, span};
use crate::files::FileId;
use std::time::Duration;

/// The most spans a body holds, as the schema's `most` on `SendMessage.spans` and `EditMessage.spans` states.
pub const MAX_SPANS: usize = 200;

/// The most files one message carries.
pub const MAX_FILES: usize = 10;

/// The most distinct emoji one message is reacted with, as Discord allows.
pub const MAX_REACTIONS: usize = 20;

/// The most pinned messages one timeline holds, as Discord allows.
pub const MAX_PINS: usize = 250;

/// The pinned messages one page of `ListPins` holds.
pub const PINS_PAGE: usize = 50;

/// How often a client tells `Typing` while a draft changes, and a server tells one timeline's `TypingNow`, at most.
pub const TYPING_EVERY: Duration = Duration::from_secs(3);

/// How long a member is shown writing after their last `Typing`.
pub const TYPING_LAPSE: Duration = Duration::from_secs(8);

/// The most a channel's unread or mention count is counted to; a count at it reads as "at least".
pub const COUNT_CAP: u32 = 100;

/// What a span names.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SpanKind {
    /// A member, by their id.
    User(String),
    /// A role, by its id.
    Role(String),
    /// Every member who may read the timeline.
    Everyone,
    /// Every member who may read the timeline and was signed in when it was sent.
    Here,
    /// A channel, by its id.
    Channel(String),
    /// A custom emoji, by its id.
    Emoji(String),
    /// A link to the text of its run.
    Link,
}

/// One span of a body, its range in Unicode scalar values, the end exclusive.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Spanned {
    /// The first scalar value of the run.
    pub start: u32,
    /// One past its last scalar value.
    pub end: u32,
    /// What it names.
    pub kind: SpanKind,
}

/// A body's spans, proved in shape against it: at most [`MAX_SPANS`], each non-empty and inside the body, sorted and
/// never overlapping, every id non-empty; a send's links each a web URL besides. Made only by [`spans_stored`] and
/// [`spans_shape`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spans(Vec<Spanned>);

/// Why a body's spans were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanError {
    /// More than [`MAX_SPANS`].
    TooMany,
    /// A span that names nothing.
    NoKind,
    /// A span that ends where it starts, or before.
    Empty,
    /// A span that ends past the body.
    PastTheEnd,
    /// A span that starts before the one before it ends: overlapping it, or out of order.
    Overlapping,
    /// A mention, a channel or an emoji with an empty id.
    NoId,
    /// A link whose text is not an `http://` or `https://` URL.
    NotALink,
}

impl Spans {
    /// No span.
    pub const NONE: Spans = Spans(Vec::new());

    /// Each span, in order.
    pub fn iter(&self) -> std::slice::Iter<'_, Spanned> {
        self.0.iter()
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The text of each link span of `body`, in order: the targets themselves. `body` must be the one these spans were
    /// proved against.
    pub fn links<'b>(&self, body: &'b str) -> Vec<&'b str> {
        let bounds = scalar_bounds(body);
        let at = |scalar: u32| usize::try_from(scalar).ok().and_then(|scalar| bounds.get(scalar).copied());
        self.0
            .iter()
            .filter(|spanned| spanned.kind == SpanKind::Link)
            .filter_map(|spanned| body.get(at(spanned.start)?..at(spanned.end)?))
            .collect()
    }

    /// The spans as the wire carries them.
    pub fn wire(&self) -> Vec<Span> {
        self.0
            .iter()
            .map(|spanned| {
                let kind = match &spanned.kind {
                    SpanKind::User(id) => span::Kind::UserId(id.clone()),
                    SpanKind::Role(id) => span::Kind::RoleId(id.clone()),
                    SpanKind::Everyone => span::Kind::Everyone(crate::chatkord_v1::Everyone {}),
                    SpanKind::Here => span::Kind::Here(crate::chatkord_v1::Here {}),
                    SpanKind::Channel(id) => span::Kind::ChannelId(id.clone()),
                    SpanKind::Emoji(id) => span::Kind::EmojiId(id.clone()),
                    SpanKind::Link => span::Kind::Link(crate::chatkord_v1::Link {}),
                };
                Span { start: spanned.start, end: spanned.end, kind: Some(kind) }
            })
            .collect()
    }
}

impl<'a> IntoIterator for &'a Spans {
    type Item = &'a Spanned;
    type IntoIter = std::slice::Iter<'a, Spanned>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// The byte offset of each scalar value of `body`, and its end.
fn scalar_bounds(body: &str) -> Vec<usize> {
    body.char_indices().map(|(at, _)| at).chain(std::iter::once(body.len())).collect()
}

/// The spans of `body` as stored, proved in shape: at most [`MAX_SPANS`], each non-empty and inside the body, sorted
/// and never overlapping, every id non-empty. What a stored row is read by, whatever policy its send was held to; a
/// send is proved by [`spans_shape`].
pub fn spans_stored(body: &str, spans: &[Span]) -> Result<Spans, SpanError> {
    if spans.len() > MAX_SPANS {
        return Err(SpanError::TooMany);
    }
    let length = body.chars().count();
    let mut shaped = Vec::with_capacity(spans.len());
    let mut after = 0u32;
    for wire in spans {
        let named = |id: &String| if id.is_empty() { Err(SpanError::NoId) } else { Ok(id.clone()) };
        let kind = match wire.kind.as_ref().ok_or(SpanError::NoKind)? {
            span::Kind::UserId(id) => SpanKind::User(named(id)?),
            span::Kind::RoleId(id) => SpanKind::Role(named(id)?),
            span::Kind::Everyone(_) => SpanKind::Everyone,
            span::Kind::Here(_) => SpanKind::Here,
            span::Kind::ChannelId(id) => SpanKind::Channel(named(id)?),
            span::Kind::EmojiId(id) => SpanKind::Emoji(named(id)?),
            span::Kind::Link(_) => SpanKind::Link,
        };
        if wire.end <= wire.start {
            return Err(SpanError::Empty);
        }
        if usize::try_from(wire.end).map_or(true, |end| end > length) {
            return Err(SpanError::PastTheEnd);
        }
        if wire.start < after {
            return Err(SpanError::Overlapping);
        }
        after = wire.end;
        shaped.push(Spanned { start: wire.start, end: wire.end, kind });
    }
    Ok(Spans(shaped))
}

/// The spans of `body` as sent: in shape as [`spans_stored`] proves, and every link's text a web URL a
/// [`LinkTarget`] reads; whether each id names something the sender may name is the server's to ask of its store.
pub fn spans_shape(body: &str, spans: &[Span]) -> Result<Spans, SpanError> {
    let shaped = spans_stored(body, spans)?;
    if !shaped.links(body).into_iter().all(|text| LinkTarget::parse(text).is_some()) {
        return Err(SpanError::NotALink);
    }
    Ok(shaped)
}

/// Why a send's files were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesError {
    /// A length that is not whole ids of [`FileId::LEN`] bytes.
    Length,
    /// More than [`MAX_FILES`].
    TooMany,
    /// One id twice.
    Repeated,
}

/// The file ids `SendMessage.files` holds, back to back, in order: whole ids, at most [`MAX_FILES`], each once.
pub fn files_shape(bytes: &[u8]) -> Result<Vec<FileId>, FilesError> {
    let (ids, rest) = bytes.as_chunks::<{ FileId::LEN }>();
    if !rest.is_empty() {
        return Err(FilesError::Length);
    }
    if ids.len() > MAX_FILES {
        return Err(FilesError::TooMany);
    }
    let mut files: Vec<FileId> = Vec::with_capacity(ids.len());
    for id in ids {
        let id = FileId::from_bytes(*id);
        if files.contains(&id) {
            return Err(FilesError::Repeated);
        }
        files.push(id);
    }
    Ok(files)
}

/// The emoji a reaction is made with, proved in shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Reacted {
    /// A Unicode emoji [`unicode_emoji`] accepts.
    Unicode(String),
    /// A custom emoji, by a non-empty id; whether the server holds it is its store's to say.
    Custom(String),
}

/// Why a reaction's emoji was refused: none, an empty custom id, or text no Unicode emoji is shaped like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReactionRefused;

/// The emoji `key` names, refused when absent or out of shape.
pub fn reaction_shape(key: Option<&ReactionKey>) -> Result<Reacted, ReactionRefused> {
    match key.and_then(|key| key.emoji.as_ref()).ok_or(ReactionRefused)? {
        reaction_key::Emoji::Unicode(text) => {
            unicode_emoji(text).map(|text| Reacted::Unicode(text.to_string())).map_err(|_| ReactionRefused)
        }
        reaction_key::Emoji::CustomId(id) if !id.is_empty() => Ok(Reacted::Custom(id.clone())),
        reaction_key::Emoji::CustomId(_) => Err(ReactionRefused),
    }
}

impl Reacted {
    /// The emoji as the wire names it.
    pub fn wire(&self) -> ReactionKey {
        let emoji = match self {
            Reacted::Unicode(text) => reaction_key::Emoji::Unicode(text.clone()),
            Reacted::Custom(id) => reaction_key::Emoji::CustomId(id.clone()),
        };
        ReactionKey { emoji: Some(emoji) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chatkord_v1::{Everyone, Link};

    fn span(start: u32, end: u32, kind: span::Kind) -> Span {
        Span { start, end, kind: Some(kind) }
    }

    fn user(start: u32, end: u32) -> Span {
        span(start, end, span::Kind::UserId("ada".into()))
    }

    fn link(start: u32, end: u32) -> Span {
        span(start, end, span::Kind::Link(Link {}))
    }

    #[test]
    fn spans_in_range_sorted_and_apart_are_kept_and_go_back_to_the_wire_unchanged() {
        let body = "héllo @ada see https://example.invalid/a?b#c";
        let spans = [user(6, 10), link(15, 44), span(0, 1, span::Kind::Everyone(Everyone {}))];
        assert_eq!(spans_shape(body, &spans[..2]).map(|shaped| shaped.wire()), Ok(spans[..2].to_vec()));
        assert_eq!(spans_shape(body, &spans), Err(SpanError::Overlapping), "out of order");
        assert_eq!(spans_shape(body, &[user(6, 10), user(9, 12)]), Err(SpanError::Overlapping));
        assert_eq!(spans_shape(body, &[user(6, 10), user(10, 12)]).map(|shaped| shaped.len()), Ok(2), "touching");
        assert_eq!(spans_shape(body, &[]), Ok(Spans::NONE));
    }

    #[test]
    fn a_span_out_of_range_empty_or_naming_nothing_is_refused() {
        let body = "a 🙂 b";
        assert_eq!(spans_shape(body, &[user(4, 5)]).map(|shaped| shaped.len()), Ok(1), "scalars, not bytes");
        assert_eq!(spans_shape(body, &[user(4, 6)]), Err(SpanError::PastTheEnd));
        assert_eq!(spans_shape(body, &[user(2, 2)]), Err(SpanError::Empty));
        assert_eq!(spans_shape(body, &[Span { start: 0, end: 1, kind: None }]), Err(SpanError::NoKind));
        assert_eq!(spans_shape(body, &[span(0, 1, span::Kind::RoleId(String::new()))]), Err(SpanError::NoId));
    }

    #[test]
    fn more_spans_than_the_most_are_refused() {
        let body = "a".repeat(2 * MAX_SPANS + 2);
        let spans = |count: usize| -> Vec<Span> { (0u32..).take(count).map(|at| user(2 * at, 2 * at + 1)).collect() };
        let most = spans(MAX_SPANS);
        assert_eq!(spans_shape(&body, &most).map(|shaped| shaped.len()), Ok(MAX_SPANS));
        let over = spans(MAX_SPANS + 1);
        assert_eq!(spans_shape(&body, &over), Err(SpanError::TooMany));
    }

    #[test]
    fn a_link_is_its_own_text_and_reads_as_a_web_url() {
        for good in ["https://example.invalid", "HTTP://example.invalid/x"] {
            let length = u32::try_from(good.chars().count()).expect("short");
            assert!(spans_shape(good, &[link(0, length)]).is_ok(), "{good}");
        }
        for bad in ["click here", "ftp://example.invalid", "https://", "https:///path", "javascript:alert(1)"] {
            let length = u32::try_from(bad.chars().count()).expect("short");
            assert_eq!(spans_shape(bad, &[link(0, length)]), Err(SpanError::NotALink), "{bad}");
        }
        let masked = "see https://example.invalid now";
        assert_eq!(spans_shape(masked, &[link(0, 31)]), Err(SpanError::NotALink), "a link's run is its target alone");
    }

    #[test]
    fn a_stored_link_is_read_by_its_shape_alone_and_a_send_by_its_scheme_too() {
        let invite = "chatkord://example.invalid/invite";
        let length = u32::try_from(invite.chars().count()).expect("short");
        assert_eq!(spans_shape(invite, &[link(0, length)]), Err(SpanError::NotALink), "a send's link is a web URL");
        let stored = spans_stored(invite, &[link(0, length)]).expect("a stored row's shape");
        assert_eq!(stored.links(invite), vec![invite]);
        assert_eq!(spans_stored(invite, &[link(0, length + 1)]), Err(SpanError::PastTheEnd), "still in range");
    }

    #[test]
    fn files_are_whole_ids_at_most_ten_each_once() {
        let ids_of = |count: usize| -> Vec<u8> { (0u8..).take(count).flat_map(|at| [at; FileId::LEN]).collect() };
        let ids = ids_of(MAX_FILES);
        assert_eq!(files_shape(&ids).map(|files| files.len()), Ok(MAX_FILES));
        assert_eq!(files_shape(&[]), Ok(Vec::new()));
        assert_eq!(files_shape(&ids[..FileId::LEN + 1]), Err(FilesError::Length));
        let over = ids_of(MAX_FILES + 1);
        assert_eq!(files_shape(&over), Err(FilesError::TooMany));
        assert_eq!(files_shape(&[[7u8; FileId::LEN], [7u8; FileId::LEN]].concat()), Err(FilesError::Repeated));
    }

    #[test]
    fn a_reaction_is_a_unicode_emoji_or_a_custom_id() {
        let key = |emoji| ReactionKey { emoji: Some(emoji) };
        let smile = key(reaction_key::Emoji::Unicode("🙂".into()));
        assert_eq!(reaction_shape(Some(&smile)), Ok(Reacted::Unicode("🙂".into())));
        assert_eq!(reaction_shape(Some(&smile)).map(|reacted| reacted.wire()), Ok(smile));
        assert_eq!(
            reaction_shape(Some(&key(reaction_key::Emoji::CustomId("7".into())))),
            Ok(Reacted::Custom("7".into()))
        );
        assert_eq!(reaction_shape(Some(&key(reaction_key::Emoji::CustomId(String::new())))), Err(ReactionRefused));
        assert_eq!(reaction_shape(Some(&key(reaction_key::Emoji::Unicode("ok".into())))), Err(ReactionRefused));
        assert_eq!(reaction_shape(Some(&ReactionKey { emoji: None })), Err(ReactionRefused));
        assert_eq!(reaction_shape(None), Err(ReactionRefused));
    }
}
