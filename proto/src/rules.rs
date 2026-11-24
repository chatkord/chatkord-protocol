//! The shapes both ends check: a username, a name others read, a chosen password's length and commonness, a token's
//! spelling, an invite's terms, a picked colour, the grant words a role holds and a channel overwrites, the roles'
//! order, the most a member's streams send, a status's longest end and pace, and a message's spans, files, reactions,
//! pins and the server's emoji. The server refuses and gates by them; the client reads them, so one owner keeps the
//! two together.

use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

mod emoji;
mod hosting;
mod link;
mod message;
mod preview;
mod search;

pub use emoji::{
    EMOJI_SIDE, EMOJI_UPLOAD_BYTES, EmojiName, EmojiNameRefused, MAX_EMOJI, MAX_EMOJI_NAME, MAX_UNICODE_EMOJI_BYTES,
    MAX_UNICODE_EMOJI_CHARS, MIN_EMOJI_NAME, UnicodeEmojiRefused, emoji_name, unicode_emoji,
};
pub use hosting::{
    Hosting, HostingField, HostingRefused, MAX_PUBLIC_ADDRESS_BYTES, PublicAddress, PublicAddressRefused, Switch,
    SwitchSet,
};
pub use link::{LinkTarget, Scheme, find_links};
pub use message::{
    COUNT_CAP, FilesError, MAX_FILES, MAX_PINS, MAX_REACTIONS, MAX_SPANS, PINS_PAGE, Reacted, ReactionRefused,
    SpanError, SpanKind, Spanned, Spans, TYPING_EVERY, TYPING_LAPSE, files_shape, reaction_shape, spans_shape,
    spans_stored,
};
pub use preview::{MAX_PREVIEW_DESCRIPTION_CHARS, MAX_PREVIEW_TITLE_CHARS, PreviewRefused, Previewed, preview_shape};
pub use search::{MAX_SEARCH_CHARS, QueryRefused, SearchQuery};

/// The longest a username may be, in bytes (it is ASCII).
pub const MAX_HANDLE_BYTES: usize = 32;

/// The shortest a username may be, in bytes.
pub const MIN_HANDLE_BYTES: usize = 2;

/// The longest a name others read may be, in characters: a display name, a server's name.
pub const MAX_NAME_CHARS: usize = 64;

/// The most bits a second a screen share is encoded at, whatever its size: an encoder's envelope is held to it
/// wherever its rows rise past it, and a server caps and watches what a member sends by it.
pub const SHARE_MAX_BITS_PER_SECOND: u64 = 40_000_000;

/// The most bits a second a camera is encoded at, whatever its size: a camera's envelope, a screen's measured rows, is
/// held to it wherever they rise past it. A server caps and watches what a member sends by it.
pub const CAMERA_MAX_BITS_PER_SECOND: u64 = 5_000_000;

/// The most bits a second one sound is sent at: Opus's own maximum (RFC 6716), whatever its sender asks of it.
pub const SOUND_MAX_BITS_PER_SECOND: u64 = 510_000;

/// The longest a status choice lasts before it ends by itself (`SetStatus.until`): two days, so until tomorrow at 08:00
/// local fits from any hour in any zone.
pub const MAX_STATUS_FOR: i64 = 2 * 24 * 60 * 60;

/// The most status choices one account makes in a minute; one more is refused.
pub const MAX_STATUS_CHANGES_PER_MINUTE: u32 = 10;

/// The most people one invite admits.
pub const MAX_INVITE_USES: u32 = 100;

/// The most days an invite lives.
pub const MAX_INVITE_DAYS: u32 = 30;

/// The most working invites a server keeps; past it a new one from a member is refused.
pub const MAX_LIVE_INVITES: u32 = 100;

/// The most channels a server holds, so the list every session is sent stays far below a frame.
pub const MAX_CHANNELS: u32 = 500;

/// The most members holding their own overwrite in one channel; a member's first past it is refused.
pub const MAX_MEMBER_OVERWRITES: u32 = 100;

/// The most categories a server holds, told whole in every channel list.
pub const MAX_CATEGORIES: u32 = 50;

/// The most roles a server holds, the default and owner ones included, as Discord allows: one list holds them all.
pub const MAX_ROLES: u32 = 250;

/// The fewest characters a password someone is setting may have, counted after Unicode normalisation (NIST SP
/// 800-63B-4 section 3.1.1.2: 15 for a single-factor password).
pub const MIN_CHOSEN_CHARS: usize = 15;

/// The common passwords of at least [`MIN_CHOSEN_CHARS`] characters, NFC-normalised and lowercased, one a line in
/// byte order: two of SecLists' lists (MIT, `proto/data/LICENSE-SecLists.txt`), made by `scripts/deps/passwords.py`.
const COMMON: &str = include_str!("../data/common-passwords.txt");

/// Whether `password` is one a guesser tries first: a chosen password is compared against a list of common ones (NIST
/// SP 800-63B-4 section 3.1.1.2), ignoring case and Unicode spelling.
pub fn is_common(password: &str) -> bool {
    static LINES: LazyLock<Vec<&'static str>> = LazyLock::new(|| COMMON.lines().collect());
    let folded = password.nfc().collect::<String>().to_lowercase();
    LINES.binary_search(&folded.as_str()).is_ok()
}

/// How long a session token or an invite code is once written out: 256 bits in URL-safe base64, unpadded.
pub const TOKEN_LEN: usize = 43;

/// Why a username was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleRefused {
    /// Under [`MIN_HANDLE_BYTES`].
    TooShort,
    /// Over [`MAX_HANDLE_BYTES`].
    TooLong,
    /// A character other than ASCII letters, digits, '.', '_' and '-', or a first one that is not a letter or digit.
    Characters,
}

/// A username: ASCII letters, digits, '.', '_' or '-', 2 to 32 of them, starting with a letter or a digit. Other
/// scripts are refused because look-alike letters make two names that read the same.
pub fn handle_shape(text: &str) -> Result<(), HandleRefused> {
    if text.len() < MIN_HANDLE_BYTES {
        return Err(HandleRefused::TooShort);
    }
    if text.len() > MAX_HANDLE_BYTES {
        return Err(HandleRefused::TooLong);
    }
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-');
    if !text.bytes().all(allowed) || !text.bytes().next().is_some_and(|byte| byte.is_ascii_alphanumeric()) {
        return Err(HandleRefused::Characters);
    }
    Ok(())
}

/// Why a name others read was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRefused {
    /// Nothing but white space.
    Empty,
    /// A control character, or one of [`DIRECTION_CONTROLS`].
    ControlCharacter,
    /// Over the bound its kind of name has.
    TooLong,
}

/// Characters that reorder the text around them, so a name holding one can read as another name (the CVE-2021-42574
/// set): every one Unicode category Cf, which `char::is_control` (category Cc) does not cover.
pub const DIRECTION_CONTROLS: [char; 12] = [
    '\u{061C}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}',
    '\u{2067}', '\u{2068}', '\u{2069}',
];

/// A name others read, trimmed: not empty, no control or text-direction character, at most `max` characters.
pub fn text_shape(text: &str, max: usize) -> Result<&str, TextRefused> {
    let text = text.trim();
    if text.is_empty() {
        return Err(TextRefused::Empty);
    }
    if text.chars().any(|character| character.is_control() || DIRECTION_CONTROLS.contains(&character)) {
        return Err(TextRefused::ControlCharacter);
    }
    if text.chars().count() > max {
        return Err(TextRefused::TooLong);
    }
    Ok(text)
}

/// The most Unicode scalar values a message's body holds; a longer one is refused, never cut.
pub const MAX_MESSAGE_CHARS: usize = 4_000;

/// The most bytes a message's body holds: four for each scalar, checked before a scalar is counted.
pub const MAX_MESSAGE_BYTES: usize = MAX_MESSAGE_CHARS * 4;

/// The most messages one page of a channel's history holds.
pub const MAX_PAGE: u32 = 100;

/// The most changes one catch-up tells; a channel changed more since is reloaded instead.
pub const MAX_CATCH_UP: u32 = 500;

/// The most conversations one account opens in an hour; one more is refused, never made. An existing one opened again
/// counts for nothing.
pub const MAX_CONVERSATIONS_OPENED_PER_HOUR: u32 = 20;

/// The most conversations one list holds; `Conversations.more` says there were more.
pub const MAX_CONVERSATIONS_LISTED: u32 = 100;

/// The most threads one account follows; a follow past it is refused, and starting, replying or being named in a
/// reply follows only while there is room.
pub const MAX_FOLLOWED: u32 = 500;

/// The most followed threads one list holds, the most recently active first.
pub const MAX_FOLLOWED_LISTED: u32 = 100;

/// The most Unicode scalar values of a message a list's row shows (`Preview.text`).
pub const PREVIEW_CHARS: usize = 100;

/// The start of a message a list's row shows: its first line that holds anything, trimmed, cut after
/// [`PREVIEW_CHARS`] Unicode scalar values.
pub fn preview_text(body: &str) -> String {
    body.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default().chars().take(PREVIEW_CHARS).collect()
}

/// The words a member of a conversation holds in it, and no other: its roles, ownership and every channel overwrite
/// are beside the point there.
pub const CONVERSATION_GRANTS: Grants =
    Grants::of(Grant::ChannelRead).with(Grant::SendMessage).with(Grant::AttachFiles).with(Grant::AddReactions);

/// The words a conversation whose other member is gone takes from [`CONVERSATION_GRANTS`]: it is read, not written.
pub const CONVERSATION_WRITING: Grants =
    Grants::of(Grant::SendMessage).with(Grant::AttachFiles).with(Grant::AddReactions);

/// A send's nonce, in bytes: drawn by the client, so a resend after a lost reply is known for the same send.
pub const NONCE_BYTES: usize = 16;

/// The direction controls a message refuses: embeddings and overrides, which reorder the text after them past the
/// message's own end (the Trojan Source set). Marks and isolates, which stay inside what they wrap, are kept.
pub const DIRECTION_OVERRIDES: [char; 5] = ['\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}'];

/// Why a message's body was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyRefused {
    /// Nothing but white space.
    Empty,
    /// A control character other than a new line or a tab.
    ControlCharacter,
    /// One of [`DIRECTION_OVERRIDES`].
    DirectionOverride,
    /// Over [`MAX_MESSAGE_BYTES`] or [`MAX_MESSAGE_CHARS`].
    TooLong,
}

/// What a message carries beside its body, which decides whether the body may be left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carries {
    /// Its body alone, so it needs one.
    Nothing,
    /// Files, which say enough: its body may be the empty string.
    Files,
}

/// A message's body as sent: new lines and tabs kept, the text not trimmed; refused when over its bounds, holding
/// another control character or a direction override, or empty unless files go with it. White space alone is never
/// a body.
pub fn message_shape(body: &str, carries: Carries) -> Result<&str, BodyRefused> {
    if body.len() > MAX_MESSAGE_BYTES || body.chars().count() > MAX_MESSAGE_CHARS {
        return Err(BodyRefused::TooLong);
    }
    if body.is_empty() && carries == Carries::Files {
        return Ok(body);
    }
    if body.trim().is_empty() {
        return Err(BodyRefused::Empty);
    }
    if body.chars().any(|character| character.is_control() && !matches!(character, '\n' | '\t')) {
        return Err(BodyRefused::ControlCharacter);
    }
    if body.chars().any(|character| DIRECTION_OVERRIDES.contains(&character)) {
        return Err(BodyRefused::DirectionOverride);
    }
    Ok(body)
}

/// Whether `text` is spelled as a token or an invite code is: [`TOKEN_LEN`] characters of URL-safe base64.
pub fn token_shaped(text: &str) -> bool {
    text.len() == TOKEN_LEN && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A colour a server or a person picked. Written `#RRGGBB` in upper case, the one spelling the store keeps and the
/// wire carries, so one colour is never two strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Colour([u8; 3]);

/// Why a written colour was refused: anything but six hexadecimal digits, with or without a leading `#`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColourRefused;

impl Colour {
    /// The colour of these three channels.
    pub const fn new(red: u8, green: u8, blue: u8) -> Colour {
        Colour([red, green, blue])
    }

    /// `#RRGGBB` or `RRGGBB`, either case. Anything else, of any length, is refused whole.
    pub fn parse(text: &str) -> Result<Colour, ColourRefused> {
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ColourRefused);
        }
        let [_, red, green, blue] = u32::from_str_radix(digits, 16).map_err(|_| ColourRefused)?.to_be_bytes();
        Ok(Colour([red, green, blue]))
    }

    /// Red, green and blue, in that order.
    pub const fn rgb(self) -> [u8; 3] {
        self.0
    }
}

impl std::fmt::Display for Colour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [red, green, blue] = self.0;
        write!(f, "#{red:02X}{green:02X}{blue:02X}")
    }
}

/// What a channel is for, fixed when it is made: the store keeps its word and the wire its `ChannelKind`. A kind and
/// not a set of capabilities, so no channel holds neither messages nor a voice room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Messages, and the webhooks that post them.
    Text,
    /// A voice room members join to talk and share, and its messages beside the call.
    Voice,
}

/// What a channel may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holds {
    /// Messages: sent, edited, deleted and read back.
    Messages,
    /// A voice room `VoiceJoin` enters.
    VoiceRoom,
    /// Webhooks, which post into it as a program.
    Webhooks,
}

impl Kind {
    /// Every kind, in declaration order.
    pub const ALL: [Kind; 2] = [Self::Text, Self::Voice];

    /// The one rule of what each kind holds, which the server's routes and the client read alike.
    pub const fn holds(self, what: Holds) -> bool {
        match (self, what) {
            (Self::Text, Holds::Messages | Holds::Webhooks) | (Self::Voice, Holds::Messages | Holds::VoiceRoom) => true,
            (Self::Text, Holds::VoiceRoom) | (Self::Voice, Holds::Webhooks) => false,
        }
    }

    /// The word a channel's row keeps for it.
    pub const fn word(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Voice => "voice",
        }
    }

    /// The wire's name for it.
    pub const fn wire(self) -> crate::chatkord_v1::ChannelKind {
        match self {
            Self::Text => crate::chatkord_v1::ChannelKind::Text,
            Self::Voice => crate::chatkord_v1::ChannelKind::Voice,
        }
    }

    /// The kind the wire's number names; none for `UNSPECIFIED` and a number this build does not know.
    pub fn from_wire(number: i32) -> Option<Self> {
        match crate::chatkord_v1::ChannelKind::try_from(number).ok()? {
            crate::chatkord_v1::ChannelKind::Text => Some(Self::Text),
            crate::chatkord_v1::ChannelKind::Voice => Some(Self::Voice),
            crate::chatkord_v1::ChannelKind::Unspecified => None,
        }
    }

    /// The kind a word names; none for a word that is no kind's.
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }
}

/// A grant's word, as the store keeps it and `LoginAck.grants` carries it: the whole vocabulary, which the store's
/// `grant_words` table is written from at every open. A word says what its holder may do; who may act on whom is the
/// roles' order ([`Place`]), never a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grant {
    /// See a channel, read its messages and hear its voice room.
    ChannelRead,
    /// React to a message with an emoji.
    AddReactions,
    /// Send files with a message.
    AttachFiles,
    /// Make, list and revoke invites.
    InviteMember,
    /// Make, rename, move and delete channels and categories; with [`Grant::ManageRoles`], set a channel's overwrites.
    ManageChannel,
    /// Add, rename and delete the server's own emoji.
    ManageEmoji,
    /// List the members with their usernames, roles and words; kick, ban or unban one; give one a temporary password.
    ManageMembers,
    /// Delete another member's message.
    ManageMessages,
    /// Make, rename, recolour, move and delete roles below one's place, give or take their words, and give or take one;
    /// make an invite that grants a role holding no word the asker lacks.
    ManageRoles,
    /// Name, colour and picture the server, switch its features and set its hosting facts.
    ManageServer,
    /// Make, regenerate and delete a channel's webhooks.
    ManageWebhooks,
    /// Name `@everyone`, `@here` or a role in a message.
    MentionEveryone,
    /// Mute a member for everyone, or take them out of a voice room.
    ModerateVoice,
    /// Pin and unpin messages.
    PinMessages,
    /// Send messages, and edit and delete one's own.
    SendMessage,
    /// Send one's camera in a voice room.
    ShareCamera,
    /// Send one's screen and its sound in a voice room.
    ShareScreen,
    /// Join a voice room and send one's microphone.
    StartVoice,
}

impl Grant {
    /// Every grant, in the order [`Grants::words`] writes them.
    pub const ALL: [Grant; 18] = [
        Self::AddReactions,
        Self::AttachFiles,
        Self::ChannelRead,
        Self::InviteMember,
        Self::ManageChannel,
        Self::ManageEmoji,
        Self::ManageMembers,
        Self::ManageMessages,
        Self::ManageRoles,
        Self::ManageServer,
        Self::ManageWebhooks,
        Self::MentionEveryone,
        Self::ModerateVoice,
        Self::PinMessages,
        Self::SendMessage,
        Self::ShareCamera,
        Self::ShareScreen,
        Self::StartVoice,
    ];

    /// The grant's word on the wire and in the store.
    pub const fn word(self) -> &'static str {
        match self {
            Self::AddReactions => "ADD_REACTIONS",
            Self::AttachFiles => "ATTACH_FILES",
            Self::ChannelRead => "CHANNEL_READ",
            Self::InviteMember => "INVITE_MEMBER",
            Self::ManageChannel => "MANAGE_CHANNEL",
            Self::ManageEmoji => "MANAGE_EMOJI",
            Self::ManageMembers => "MANAGE_MEMBERS",
            Self::ManageMessages => "MANAGE_MESSAGES",
            Self::ManageRoles => "MANAGE_ROLES",
            Self::ManageServer => "MANAGE_SERVER",
            Self::ManageWebhooks => "MANAGE_WEBHOOKS",
            Self::MentionEveryone => "MENTION_EVERYONE",
            Self::ModerateVoice => "MODERATE_VOICE",
            Self::PinMessages => "PIN_MESSAGES",
            Self::SendMessage => "SEND_MESSAGE",
            Self::ShareCamera => "SHARE_CAMERA",
            Self::ShareScreen => "SHARE_SCREEN",
            Self::StartVoice => "START_VOICE",
        }
    }

    /// The grant `word` names, or none for a word this build does not know.
    pub fn of_word(word: &str) -> Option<Grant> {
        Self::ALL.into_iter().find(|grant| grant.word() == word)
    }

    /// Whether a channel's overwrites may allow or deny it: what a member does in one channel, never what they do to
    /// the server or to another member.
    pub const fn in_channel(self) -> bool {
        match self {
            Self::AddReactions
            | Self::AttachFiles
            | Self::ChannelRead
            | Self::ManageMessages
            | Self::MentionEveryone
            | Self::PinMessages
            | Self::SendMessage
            | Self::ShareCamera
            | Self::ShareScreen
            | Self::StartVoice => true,
            Self::InviteMember
            | Self::ManageChannel
            | Self::ManageEmoji
            | Self::ManageMembers
            | Self::ManageRoles
            | Self::ManageServer
            | Self::ManageWebhooks
            | Self::ModerateVoice => false,
        }
    }

    const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// The grants an account holds, read from the words a server sent. A word this build does not know is passed
/// over, so a newer server's words never stop an older client reading the rest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Grants(u32);

const _: () = assert!(Grant::ALL.len() <= u32::BITS as usize, "every grant has its own bit in `Grants`");

impl Grants {
    /// No grant: what an op the vocabulary says nothing about asks.
    pub const NONE: Grants = Grants(0);

    /// `grant` alone.
    pub const fn of(grant: Grant) -> Grants {
        Grants(grant.bit())
    }

    /// The grants these words name; an unknown word is passed over.
    pub fn from_words<S: AsRef<str>>(words: &[S]) -> Grants {
        Grants(words.iter().filter_map(|word| Grant::of_word(word.as_ref())).fold(0, |set, grant| set | grant.bit()))
    }

    /// Whether `grant` is among these.
    pub const fn holds(self, grant: Grant) -> bool {
        self.0 & grant.bit() != 0
    }

    /// Whether every grant of `needed` is among these; none needed is held.
    pub const fn holds_all(self, needed: Grants) -> bool {
        self.0 & needed.0 == needed.0
    }

    /// Every grant: what the owner holds wherever it is.
    pub fn every() -> Grants {
        Grant::ALL.into_iter().collect()
    }

    /// These and `other`'s.
    #[must_use]
    pub const fn union(self, other: Grants) -> Grants {
        Grants(self.0 | other.0)
    }

    /// These without `other`'s.
    #[must_use]
    pub const fn without(self, other: Grants) -> Grants {
        Grants(self.0 & !other.0)
    }

    /// Whether these are no grant at all.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// These and `grant` too.
    #[must_use]
    pub const fn with(self, grant: Grant) -> Grants {
        Grants(self.0 | grant.bit())
    }

    /// The words, in [`Grant::ALL`]'s order, as the wire carries them.
    pub fn words(self) -> Vec<String> {
        Grant::ALL.into_iter().filter(|grant| self.holds(*grant)).map(|grant| grant.word().to_string()).collect()
    }
}

impl FromIterator<Grant> for Grants {
    fn from_iter<I: IntoIterator<Item = Grant>>(grants: I) -> Grants {
        Grants(grants.into_iter().fold(0, |set, grant| set | grant.bit()))
    }
}

/// Where an account stands in the roles' order: the position of its highest role, a lower number ranking higher. The
/// owner role's is [`Place::OWNER`], the default role's [`Place::BOTTOM`]; told on the wire as a `place`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(u32);

impl Place {
    /// The owner role's place, above every other.
    pub const OWNER: Place = Place(0);

    /// The default role's place, below every other: an account holding nothing else acts on no one.
    pub const BOTTOM: Place = Place(u32::MAX);

    /// The place of a role at `position`.
    pub const fn at(position: u32) -> Place {
        Place(position)
    }

    /// Its position, as the store and the wire keep it.
    pub const fn position(self) -> u32 {
        self.0
    }

    /// Whether this is the owner role's place.
    pub const fn is_owner(self) -> bool {
        self.0 == Self::OWNER.0
    }

    /// Whether this place is strictly above `other`: a member acts on a member, and changes a role, only below their
    /// own place. Holding the op's grant word is asked apart.
    pub const fn outranks(self, other: Place) -> bool {
        self.0 < other.0
    }

    /// The place the wire states; absent reads as the bottom, so a malformed one never reads as the owner's.
    pub fn of_wire(place: Option<u32>) -> Place {
        place.map_or(Self::BOTTOM, Place)
    }

    /// The place as the wire states it.
    pub const fn wire(self) -> Option<u32> {
        Some(self.0)
    }
}

/// What a role's overwrite in one channel says of one word; a word it says nothing of is the roles' own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Overwrite {
    /// The word is held in the channel.
    Allow,
    /// The word is not held in the channel.
    Deny,
}

impl Overwrite {
    /// Every overwrite, which the store's vocabulary is written from.
    pub const ALL: [Overwrite; 2] = [Self::Allow, Self::Deny];

    /// The word a row keeps for it.
    pub const fn word(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }

    /// The overwrite a row's word names; none for a word that is no overwrite's.
    pub fn of_word(word: &str) -> Option<Overwrite> {
        Self::ALL.into_iter().find(|overwrite| overwrite.word() == word)
    }

    /// The wire's name for it.
    pub const fn wire(self) -> crate::chatkord_v1::OverwriteState {
        match self {
            Self::Allow => crate::chatkord_v1::OverwriteState::Allow,
            Self::Deny => crate::chatkord_v1::OverwriteState::Deny,
        }
    }

    /// What an ask's state number names: an overwrite set, or none to inherit; refused (`Err`) for `UNSPECIFIED`
    /// and a number this build does not know.
    pub fn asked(number: i32) -> Result<Option<Overwrite>, OverwriteRefused> {
        match crate::chatkord_v1::OverwriteState::try_from(number).map_err(|_| OverwriteRefused)? {
            crate::chatkord_v1::OverwriteState::Allow => Ok(Some(Self::Allow)),
            crate::chatkord_v1::OverwriteState::Deny => Ok(Some(Self::Deny)),
            crate::chatkord_v1::OverwriteState::Inherit => Ok(None),
            crate::chatkord_v1::OverwriteState::Unspecified => Err(OverwriteRefused),
        }
    }
}

/// Why an overwrite's state was refused: unspecified, or a number this build does not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverwriteRefused;

/// One layer's allows and denies of a channel's words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sides {
    /// The words this layer allows.
    pub allow: Grants,
    /// The words this layer denies.
    pub deny: Grants,
}

impl Sides {
    /// These, with `word` set to `overwrite`.
    #[must_use]
    pub const fn with(self, word: Grant, overwrite: Overwrite) -> Sides {
        match overwrite {
            Overwrite::Allow => Sides { allow: self.allow.with(word), ..self },
            Overwrite::Deny => Sides { deny: self.deny.with(word), ..self },
        }
    }
}

/// A channel's overwrites as one account meets them: the default role's, its other roles' together, and its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leaning {
    /// The default role's overwrite.
    pub default: Sides,
    /// The account's other roles' overwrites, together.
    pub roles: Sides,
    /// The account's own overwrite as a member.
    pub member: Sides,
}

/// The words an account holds in a channel: its roles' words (`base`), then the default role's overwrite, then its
/// other roles' together, an allow over a deny, then its own, a deny then an allow; the owner holds every word.
pub fn resolve(owner: bool, base: Grants, leaning: Leaning) -> Grants {
    if owner {
        return Grants::every();
    }
    let Leaning { default, roles, member } = leaning;
    base.without(default.deny)
        .union(default.allow)
        .without(roles.deny)
        .union(roles.allow)
        .without(member.deny)
        .union(member.allow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_member_s_own_overwrite_is_resolved_last_and_the_owner_holds_every_word() {
        let read = Grant::ChannelRead;
        let hidden = Leaning { default: Sides::default().with(read, Overwrite::Deny), ..Leaning::default() };
        assert!(!resolve(false, Grants::of(read), hidden).holds(read), "the default role's deny takes the roles' word");
        let role_allowed = Leaning { roles: Sides::default().with(read, Overwrite::Allow), ..hidden };
        assert!(resolve(false, Grants::NONE, role_allowed).holds(read), "a role's allow gives it back");
        let member_denied = Leaning { member: Sides::default().with(read, Overwrite::Deny), ..role_allowed };
        assert!(!resolve(false, Grants::NONE, member_denied).holds(read), "the member's own deny is last");
        let member_allowed = Leaning { member: Sides::default().with(read, Overwrite::Allow), ..hidden };
        assert!(resolve(false, Grants::NONE, member_allowed).holds(read), "the member's own allow is last");
        assert_eq!(resolve(true, Grants::NONE, member_denied), Grants::every());
    }

    #[test]
    fn a_text_channel_holds_messages_and_webhooks_and_a_voice_channel_a_room_and_messages() {
        let held = |kind: Kind| [Holds::Messages, Holds::VoiceRoom, Holds::Webhooks].map(|what| kind.holds(what));
        assert_eq!(held(Kind::Text), [true, false, true]);
        assert_eq!(held(Kind::Voice), [true, true, false]);
        assert!(Kind::ALL.into_iter().all(|kind| Kind::from_word(kind.word()) == Some(kind)));
    }

    #[test]
    fn a_common_password_is_known_whatever_its_case_and_an_uncommon_one_is_not() {
        assert!(is_common("passwordpassword") && is_common("PasswordPassword"), "case is ignored");
        assert!(is_common("1q2w3e4r5t6y7u8i9o0p"));
        assert!(!is_common("cobalt otter gathers moss"), "a phrase no list holds");
        assert!(COMMON.lines().all(|line| line.chars().count() >= MIN_CHOSEN_CHARS), "only what could be chosen");
        let lines: Vec<&str> = COMMON.lines().collect();
        assert!(lines.windows(2).all(|pair| pair[0] < pair[1]), "in byte order, each once, for the search");
    }

    #[test]
    fn an_override_control_is_refused_an_isolate_kept() {
        assert_eq!(
            message_shape("first line\n\tsecond", Carries::Nothing),
            Ok("first line\n\tsecond"),
            "new lines and tabs stay"
        );
        for kept in ["\u{2067}\u{05e9}\u{05dc}\u{05d5}\u{05dd}\u{2069} hello", "a\u{200F}b", "\u{2068}x\u{2069}"] {
            assert_eq!(
                message_shape(kept, Carries::Nothing),
                Ok(kept),
                "an isolate or a mark stays inside what it wraps"
            );
        }
        for spoof in DIRECTION_OVERRIDES {
            assert_eq!(
                message_shape(&format!("ok {spoof} ok"), Carries::Nothing),
                Err(BodyRefused::DirectionOverride),
                "{spoof:?}"
            );
        }
        assert_eq!(message_shape("a\rb", Carries::Nothing), Err(BodyRefused::ControlCharacter));
        assert_eq!(message_shape(" \n\t ", Carries::Nothing), Err(BodyRefused::Empty));
        assert_eq!(message_shape("", Carries::Nothing), Err(BodyRefused::Empty));
        assert_eq!(message_shape("", Carries::Files), Ok(""), "files say enough");
        assert_eq!(message_shape(" \n", Carries::Files), Err(BodyRefused::Empty), "white space is never a body");
    }

    #[test]
    fn a_body_over_the_bound_is_refused_not_cut() {
        let most = "\u{1F600}".repeat(MAX_MESSAGE_CHARS);
        assert_eq!(
            message_shape(&most, Carries::Nothing).map(str::len),
            Ok(MAX_MESSAGE_BYTES),
            "four bytes a scalar, at the bound"
        );
        assert_eq!(message_shape(&format!("{most}a"), Carries::Nothing), Err(BodyRefused::TooLong));
        assert_eq!(message_shape(&"a".repeat(MAX_MESSAGE_CHARS + 1), Carries::Nothing), Err(BodyRefused::TooLong));
    }

    #[test]
    fn a_colour_is_six_hex_digits_and_is_written_one_way() {
        assert_eq!(Colour::parse("#4f6bed"), Ok(Colour::new(0x4F, 0x6B, 0xED)));
        assert_eq!(Colour::parse("FF0000"), Ok(Colour::new(0xFF, 0, 0)));
        assert_eq!(Colour::parse("#4f6bed").map(|colour| colour.to_string()), Ok("#4F6BED".to_string()));
        for bad in
            ["", "#", "#fff", "#12345", "#1234567", "##123456", "#12345g", "+12345", " 123456", "#１２３４５６", "red"]
        {
            assert_eq!(Colour::parse(bad), Err(ColourRefused), "{bad:?}");
        }
    }

    #[test]
    fn a_username_is_ascii_of_two_to_thirty_two_starting_with_a_letter_or_digit() {
        for good in ["Ad", "Ada.Lovelace", "ada_1", "9lives", &"a".repeat(MAX_HANDLE_BYTES)] {
            assert_eq!(handle_shape(good), Ok(()), "{good}");
        }
        assert_eq!(handle_shape("A"), Err(HandleRefused::TooShort));
        assert_eq!(handle_shape(&"a".repeat(MAX_HANDLE_BYTES + 1)), Err(HandleRefused::TooLong));
        for bad in ["Ada Lovelace", ".ada", "-ada", "Ádá", "ада"] {
            assert_eq!(handle_shape(bad), Err(HandleRefused::Characters), "{bad}");
        }
    }

    #[test]
    fn a_name_is_trimmed_bounded_in_characters_and_holds_no_control_or_direction_character() {
        assert_eq!(text_shape("  Lounge ", 64), Ok("Lounge"));
        assert_eq!(text_shape(" \t", 64), Err(TextRefused::Empty));
        assert_eq!(text_shape("a\u{202E}b", 64), Err(TextRefused::ControlCharacter));
        assert_eq!(text_shape("a\nb", 64), Err(TextRefused::ControlCharacter));
        assert_eq!(text_shape(&"é".repeat(64), 64).map(str::len), Ok(128));
        assert_eq!(text_shape(&"é".repeat(65), 64), Err(TextRefused::TooLong));
    }

    #[test]
    fn a_token_is_forty_three_url_safe_characters() {
        let good = "A".repeat(TOKEN_LEN - 2) + "-_";
        assert!(token_shaped(&good));
        assert!(!token_shaped(good.get(1..).expect("ASCII")));
        assert!(!token_shaped(&(good.clone() + "A")));
        assert!(!token_shaped(&good.replace('-', "+")));
        assert!(!token_shaped(&good.replace('-', "=")));
    }

    #[test]
    fn grant_words_go_both_ways_and_an_unknown_word_is_passed_over() {
        for grant in Grant::ALL {
            assert_eq!(Grant::of_word(grant.word()), Some(grant));
        }
        let held = Grants::from_words(&["START_VOICE", "SOMETHING_NEWER", "MANAGE_SERVER", "ADMIN"]);
        assert!(held.holds(Grant::ManageServer) && held.holds(Grant::StartVoice));
        assert!(!held.holds(Grant::InviteMember));
        assert_eq!(held.words(), ["MANAGE_SERVER", "START_VOICE"], "a retired word is no word");
        assert_eq!(Grants::from_words(&held.words()), held);
        assert_eq!(Grant::ALL.into_iter().collect::<Grants>().words().len(), Grant::ALL.len());
    }

    #[test]
    fn a_place_acts_only_on_a_lower_place_and_the_owner_on_everyone_but_itself() {
        let moderator = Place::at(1_000);
        assert!(Place::OWNER.outranks(moderator) && Place::OWNER.outranks(Place::BOTTOM));
        assert!(!Place::OWNER.outranks(Place::OWNER), "the owner acts on no one at its own place");
        assert!(!moderator.outranks(moderator), "two members at one place act on neither");
        assert!(!moderator.outranks(Place::OWNER) && moderator.outranks(Place::at(1_001)));
        assert!(!Place::BOTTOM.outranks(Place::BOTTOM), "a member holding only the default role acts on no one");
        assert_eq!(Place::of_wire(moderator.wire()), moderator, "a place goes both ways on the wire");
        assert_eq!(Place::of_wire(None), Place::BOTTOM, "an absent place is the bottom, never the owner's");
    }

    #[test]
    fn a_channel_overwrites_only_what_a_member_does_in_it() {
        let in_channel: Vec<&str> =
            Grant::ALL.into_iter().filter(|grant| grant.in_channel()).map(Grant::word).collect();
        let expected = [
            "ADD_REACTIONS",
            "ATTACH_FILES",
            "CHANNEL_READ",
            "MANAGE_MESSAGES",
            "MENTION_EVERYONE",
            "PIN_MESSAGES",
            "SEND_MESSAGE",
            "SHARE_CAMERA",
            "SHARE_SCREEN",
            "START_VOICE",
        ];
        assert_eq!(in_channel, expected);
        for overwrite in Overwrite::ALL {
            assert_eq!(Overwrite::asked(overwrite.wire().into()), Ok(Some(overwrite)));
            assert_eq!(Overwrite::of_word(overwrite.word()), Some(overwrite));
        }
        assert_eq!(Overwrite::asked(crate::chatkord_v1::OverwriteState::Inherit.into()), Ok(None));
        assert_eq!(Overwrite::asked(0), Err(OverwriteRefused), "an unspecified state is refused");
        assert_eq!(Overwrite::asked(99), Err(OverwriteRefused), "an unknown state is refused");
        let held = Grants::of(Grant::SendMessage).with(Grant::StartVoice);
        assert_eq!(held.without(Grants::of(Grant::SendMessage)), Grants::of(Grant::StartVoice));
        assert_eq!(Grants::of(Grant::SendMessage).union(Grants::of(Grant::StartVoice)), held);
        assert_eq!(Grants::every().words().len(), Grant::ALL.len());
    }

    #[test]
    fn a_preview_is_the_first_line_that_holds_anything_cut_at_its_bound() {
        assert_eq!(preview_text("  \n\n hello \nworld"), "hello");
        assert_eq!(preview_text(""), "");
        let long = "\u{1F600}".repeat(PREVIEW_CHARS + 5);
        assert_eq!(preview_text(&long).chars().count(), PREVIEW_CHARS);
    }

    #[test]
    fn a_conversation_member_holds_four_words_and_loses_the_writing_ones_when_it_cannot_be_written_to() {
        assert!(CONVERSATION_GRANTS.holds_all(CONVERSATION_WRITING));
        assert_eq!(CONVERSATION_GRANTS.without(CONVERSATION_WRITING), Grants::of(Grant::ChannelRead));
        for withheld in [Grant::ManageMessages, Grant::PinMessages, Grant::MentionEveryone, Grant::ManageChannel] {
            assert!(!CONVERSATION_GRANTS.holds(withheld), "{withheld:?}");
        }
    }
}
