//! Whether a message alerts its reader: the one rule both ends read, so a server deciding a phone's push and a window
//! deciding its own popup agree, and a server counting a channel's mentions and a window counting the messages it is
//! told agree. The rule is two halves: [`tell`] is the server's, what a message is to the account, and [`alerts`]
//! is the device's, whether this device shows it now. Each server keeps a member's levels, mutes and suppressions for
//! its own account (`notify.proto`) and its status (`status`); neither is asked here, both are handed in.

use crate::chatkord_v1::{ChannelNotify, NotifyLevel, ServerNotify, mute};
use crate::rules::{SpanKind, Spans};
use std::time::Duration;

/// How long every device of an account must have been idle for each of them to alert: past it, the account is away
/// from all of them, and an alert on each is the best chance to be seen.
pub const IDLE_EVERYWHERE: Duration = Duration::from_secs(10 * 60);

/// What a server knows of one signed-in device's input (`Activity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    /// Input is arriving, or arrived within the limit the device keeps.
    Active,
    /// No input for the device's limit, and idle for this long since it said so.
    Idle {
        /// How long ago the device told the server it was idle.
        for_how_long: Duration,
    },
}

impl Device {
    /// How recent this device's input is, the most recent least: an active device is more recent than any idle one, and
    /// of idle devices the one that went idle last.
    const fn recency(self) -> (bool, Duration) {
        match self {
            Self::Active => (false, Duration::ZERO),
            Self::Idle { for_how_long } => (true, for_how_long),
        }
    }
}

/// Whether the device `this` alerts for an account whose other signed-in devices are `others`: it is alone, or it had
/// the most recent input (a tie alerts each of the tied), or every device has been idle at least
/// [`IDLE_EVERYWHERE`], when each alerts.
pub fn alerts_here(this: Device, others: &[Device]) -> bool {
    let most_recent = others.iter().map(|other| other.recency()).fold(this.recency(), Ord::min);
    let all_idle_long = most_recent.0 && most_recent.1 >= IDLE_EVERYWHERE;
    this.recency() == most_recent || all_idle_long
}

/// How much of a server or a channel alerts, as the wire's `NotifyLevel` names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level {
    /// Every message.
    All,
    /// A mention of the reader, and `@everyone`, `@here` or a role of theirs unless suppressed. The default for a
    /// server.
    #[default]
    Mentions,
    /// Nothing alerts, mentions included.
    Nothing,
}

impl Level {
    /// Every level, which the store's vocabulary is written from.
    pub const ALL: [Level; 3] = [Self::All, Self::Mentions, Self::Nothing];

    /// The word a row keeps for it.
    pub const fn word(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Mentions => "mentions",
            Self::Nothing => "nothing",
        }
    }

    /// The wire's name for it.
    pub const fn wire(self) -> NotifyLevel {
        match self {
            Self::All => NotifyLevel::All,
            Self::Mentions => NotifyLevel::Mentions,
            Self::Nothing => NotifyLevel::Nothing,
        }
    }

    /// The level the wire's number names; refused for `UNSPECIFIED` and a number this build does not know.
    pub fn of_wire(number: i32) -> Result<Level, NotifyRefused> {
        match NotifyLevel::try_from(number).map_err(|_| NotifyRefused)? {
            NotifyLevel::All => Ok(Self::All),
            NotifyLevel::Mentions => Ok(Self::Mentions),
            NotifyLevel::Nothing => Ok(Self::Nothing),
            NotifyLevel::Unspecified => Err(NotifyRefused),
        }
    }
}

/// A mute, as the wire's `Mute` says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mute {
    /// Until then, in seconds since the epoch.
    Until(i64),
    /// Until the reader turns it off.
    Forever,
}

impl Mute {
    /// Whether it silences at `now`.
    pub const fn holds_at(self, now: i64) -> bool {
        match self {
            Self::Until(until) => until > now,
            Self::Forever => true,
        }
    }

    /// The wire's form of it.
    pub fn wire(self) -> crate::chatkord_v1::Mute {
        let span = match self {
            Self::Until(until) => mute::Span::Until(until),
            Self::Forever => mute::Span::Forever(crate::chatkord_v1::Forever {}),
        };
        crate::chatkord_v1::Mute { span: Some(span) }
    }

    /// The mute the wire states; none for none, refused for one that says neither until when nor forever.
    pub fn of_wire(mute: Option<&crate::chatkord_v1::Mute>) -> Result<Option<Mute>, NotifyRefused> {
        match mute.map(|mute| mute.span.as_ref()) {
            None => Ok(None),
            Some(Some(mute::Span::Until(until))) => Ok(Some(Self::Until(*until))),
            Some(Some(mute::Span::Forever(_))) => Ok(Some(Self::Forever)),
            Some(None) => Err(NotifyRefused),
        }
    }
}

/// Why settings the wire states were refused: a level unspecified or unknown, or a mute that says nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotifyRefused;

/// The reader's settings for one server, kept by that server for their account.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServerSettings {
    /// Every channel's level unless the channel sets its own.
    pub level: Level,
    /// `@everyone` and `@here` count as no mention.
    pub suppress_everyone: bool,
    /// A mention of one of the reader's roles counts as no mention.
    pub suppress_roles: bool,
    /// The whole server is muted; none, it is not.
    pub mute: Option<Mute>,
}

impl ServerSettings {
    /// The settings the wire states; absent reads as the default.
    pub fn of_wire(settings: Option<&ServerNotify>) -> Result<ServerSettings, NotifyRefused> {
        let Some(settings) = settings else { return Ok(Self::default()) };
        Ok(Self {
            level: Level::of_wire(settings.level)?,
            suppress_everyone: settings.suppress_everyone,
            suppress_roles: settings.suppress_roles,
            mute: Mute::of_wire(settings.mute.as_ref())?,
        })
    }

    /// The wire's form of them.
    pub fn wire(self) -> ServerNotify {
        ServerNotify {
            level: self.level.wire().into(),
            suppress_everyone: self.suppress_everyone,
            suppress_roles: self.suppress_roles,
            mute: self.mute.map(Mute::wire),
        }
    }
}

/// The reader's settings for one channel, kept with their server settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChannelSettings {
    /// Its own level; none follows the server's.
    pub level: Option<Level>,
    /// The channel is muted; none, it is not.
    pub mute: Option<Mute>,
}

impl ChannelSettings {
    /// The settings the wire states for its channel, which the caller reads from `channel_id`.
    pub fn of_wire(settings: &ChannelNotify) -> Result<ChannelSettings, NotifyRefused> {
        Ok(Self {
            level: settings.level.map(Level::of_wire).transpose()?,
            mute: Mute::of_wire(settings.mute.as_ref())?,
        })
    }

    /// The wire's form of them, for `channel_id`.
    pub fn wire(self, channel_id: String) -> ChannelNotify {
        ChannelNotify {
            channel_id,
            level: self.level.map(|level| level.wire().into()),
            mute: self.mute.map(Mute::wire),
        }
    }

    /// Whether these are no setting at all: the channel follows the server and is not muted, so none is kept.
    pub const fn follows(self) -> bool {
        self.level.is_none() && self.mute.is_none()
    }
}

/// What a message says of its reader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mentions {
    /// The reader wrote it.
    pub from_reader: bool,
    /// It names the reader, or replies to them mentioning its author.
    pub reader: bool,
    /// It names `@everyone`, or `@here` while the reader was signed in.
    pub everyone: bool,
    /// It names a role the reader holds.
    pub role: bool,
}

/// A message as [`mentions_of`] reads it.
#[derive(Clone, Copy, Debug)]
pub struct Said<'a> {
    /// What its body names.
    pub spans: &'a Spans,
    /// Who wrote it, by user id; none for a former author.
    pub author: Option<&'a str>,
    /// The author of the message it replies to, by user id, when the reply mentions them; none otherwise.
    pub mentions_replied: Option<&'a str>,
}

/// The reader a message is read for.
#[derive(Clone, Copy, Debug)]
pub struct Reader<'a> {
    /// Their user id.
    pub user_id: &'a str,
    /// The ids of the roles they hold.
    pub role_ids: &'a [String],
    /// They were signed in when the message was sent, which `@here` asks.
    pub online_at_send: bool,
}

/// What `said` says of `reader`: the one count of mentions both a server's and a window's counts read.
pub fn mentions_of(said: Said<'_>, reader: Reader<'_>) -> Mentions {
    let mut mentions = Mentions {
        from_reader: said.author == Some(reader.user_id),
        reader: said.mentions_replied == Some(reader.user_id),
        ..Mentions::default()
    };
    for spanned in said.spans {
        match &spanned.kind {
            SpanKind::User(id) => mentions.reader |= id == reader.user_id,
            SpanKind::Role(id) => mentions.role |= reader.role_ids.contains(id),
            SpanKind::Everyone => mentions.everyone = true,
            SpanKind::Here => mentions.everyone |= reader.online_at_send,
            SpanKind::Channel(_) | SpanKind::Emoji(_) | SpanKind::Link => {}
        }
    }
    mentions
}

/// How a message marks its channel and the badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Nothing: the reader wrote it, or read it as it came.
    None,
    /// Unread, quietly: the channel is bold and the badge shows a dot.
    Quiet,
    /// Unread and counted: in the channel's count and the badge's number.
    Counted,
    /// In a muted channel or server: no part of the badge; a mention counts on the channel alone.
    Muted {
        /// It mentions the reader (as `Mentions` counts them), so the channel's own count shows it.
        mentioned: bool,
    },
}

/// What this device knows when a message that counts arrives: the device's half of the rule, which no server asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Here {
    /// Seconds since the epoch.
    pub now: i64,
    /// This device alerts for the account: it had the reader's input last, or every device has been idle long enough.
    pub alerts_here: bool,
    /// The account's Do not disturb is in force now (`status::Chosen::quiet`).
    pub quiet: bool,
}

/// The server's half: how `mentions` marks the channel and the badge for a reader whose settings are `server` and
/// `channel`, at `now` in seconds since the epoch. It knows the account and no device, so a server counting mentions
/// and a window counting the messages it is told read the same answer. A message read as it comes is for the caller
/// to leave out, since where the reader is looking is a device's fact.
pub fn tell(server: ServerSettings, channel: ChannelSettings, mentions: Mentions, now: i64) -> Mark {
    if mentions.from_reader {
        return Mark::None;
    }
    let mentioned = mentions.reader
        || (mentions.everyone && !server.suppress_everyone)
        || (mentions.role && !server.suppress_roles);
    let muted = |mute: Option<Mute>| mute.is_some_and(|mute| mute.holds_at(now));
    if muted(server.mute) || muted(channel.mute) {
        return Mark::Muted { mentioned };
    }
    let notable = match channel.level.unwrap_or(server.level) {
        Level::All => true,
        Level::Mentions => mentioned,
        Level::Nothing => false,
    };
    if notable { Mark::Counted } else { Mark::Quiet }
}

/// The device's half: whether this device shows a notification and plays its sound for a message the server's half
/// marked `told`. Only a counted message alerts, and only on the device in use outside the account's Do not disturb.
pub const fn alerts(told: Mark, here: Here) -> bool {
    matches!(told, Mark::Counted) && here.alerts_here && !here.quiet
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000;

    fn here() -> Here {
        Here { now: NOW, alerts_here: true, quiet: false }
    }

    fn plain() -> Mentions {
        Mentions::default()
    }

    fn me() -> Mentions {
        Mentions { reader: true, ..Mentions::default() }
    }

    /// Both halves in turn, as a window asks them.
    fn decide(server: ServerSettings, channel: ChannelSettings, mentions: Mentions, here: Here) -> (Mark, bool) {
        let mark = tell(server, channel, mentions, here.now);
        (mark, alerts(mark, here))
    }

    fn alerting(mark: Mark) -> (Mark, bool) {
        (mark, true)
    }

    fn silent(mark: Mark) -> (Mark, bool) {
        (mark, false)
    }

    #[test]
    fn a_server_starts_at_mentions_and_a_channel_follows_it_unless_it_sets_its_own() {
        let (server, channel) = (ServerSettings::default(), ChannelSettings::default());
        assert_eq!(server.level, Level::Mentions);
        assert_eq!(decide(server, channel, plain(), here()), silent(Mark::Quiet), "a plain message is a quiet unread");
        assert_eq!(decide(server, channel, me(), here()), alerting(Mark::Counted), "a mention alerts");
        let all = ChannelSettings { level: Some(Level::All), ..channel };
        assert_eq!(decide(server, all, plain(), here()), alerting(Mark::Counted), "a channel at All");
        let nothing = ChannelSettings { level: Some(Level::Nothing), ..channel };
        assert_eq!(decide(server, nothing, me(), here()), silent(Mark::Quiet), "Nothing: not even a mention");
        let loud = ServerSettings { level: Level::All, ..server };
        assert_eq!(decide(loud, channel, plain(), here()), alerting(Mark::Counted), "a server at All");
    }

    #[test]
    fn everyone_and_a_role_alert_unless_suppressed_and_a_personal_mention_always_counts() {
        let server = ServerSettings::default();
        let everyone = Mentions { everyone: true, ..Mentions::default() };
        let role = Mentions { role: true, ..Mentions::default() };
        let channel = ChannelSettings::default();
        assert_eq!(decide(server, channel, everyone, here()), alerting(Mark::Counted));
        assert_eq!(decide(server, channel, role, here()), alerting(Mark::Counted));
        let quiet = ServerSettings { suppress_everyone: true, suppress_roles: true, ..server };
        assert_eq!(decide(quiet, channel, everyone, here()), silent(Mark::Quiet));
        assert_eq!(decide(quiet, channel, role, here()), silent(Mark::Quiet));
        let both = Mentions { reader: true, everyone: true, ..Mentions::default() };
        assert_eq!(decide(quiet, channel, both, here()), alerting(Mark::Counted), "named, it still counts");
    }

    #[test]
    fn a_mute_silences_until_its_time_and_leaves_the_badge() {
        let channel = ChannelSettings { mute: Some(Mute::Until(NOW + 60)), ..ChannelSettings::default() };
        let server = ServerSettings { level: Level::All, ..ServerSettings::default() };
        assert_eq!(decide(server, channel, plain(), here()), silent(Mark::Muted { mentioned: false }));
        assert_eq!(decide(server, channel, me(), here()), silent(Mark::Muted { mentioned: true }), "counted on it");
        let later = Here { now: NOW + 60, ..here() };
        assert_eq!(decide(server, channel, plain(), later), alerting(Mark::Counted), "the mute ends by itself");
        let forever = ServerSettings { mute: Some(Mute::Forever), ..server };
        let channel = ChannelSettings::default();
        assert_eq!(decide(forever, channel, me(), later), silent(Mark::Muted { mentioned: true }), "a whole server");
    }

    #[test]
    fn nothing_alerts_from_oneself_on_another_device_or_while_quiet() {
        let (server, channel) = (ServerSettings::default(), ChannelSettings::default());
        let own = Mentions { from_reader: true, ..me() };
        assert_eq!(decide(server, channel, own, here()), silent(Mark::None));
        let elsewhere = Here { alerts_here: false, ..here() };
        assert_eq!(decide(server, channel, me(), elsewhere), silent(Mark::Counted), "counted, alerted on the other");
        let quiet = Here { quiet: true, ..here() };
        assert_eq!(decide(server, channel, me(), quiet), silent(Mark::Counted), "the account's do not disturb");
    }

    #[test]
    fn a_muted_channel_still_counts_a_personal_mention_and_here_asks_who_was_signed_in() {
        let spans = |kinds: &[crate::chatkord_v1::span::Kind]| {
            let body = "x".repeat(kinds.len() * 2);
            let wire: Vec<crate::chatkord_v1::Span> = (0u32..)
                .zip(kinds)
                .map(|(at, kind)| crate::chatkord_v1::Span { start: 2 * at, end: 2 * at + 1, kind: Some(kind.clone()) })
                .collect();
            (crate::rules::spans_shape(&body, &wire).expect("in shape"), body)
        };
        use crate::chatkord_v1::span::Kind;
        let roles = ["mods".to_string()];
        let reader = Reader { user_id: "ada", role_ids: &roles, online_at_send: false };
        let (named, _) = spans(&[Kind::UserId("ada".into()), Kind::RoleId("mods".into())]);
        let said = Said { spans: &named, author: Some("bo"), mentions_replied: None };
        let read = mentions_of(said, reader);
        assert_eq!(read, Mentions { reader: true, role: true, ..Mentions::default() });
        let muted = ChannelSettings { mute: Some(Mute::Forever), ..ChannelSettings::default() };
        assert_eq!(decide(ServerSettings::default(), muted, read, here()), silent(Mark::Muted { mentioned: true }));
        let (here_only, _) = spans(&[Kind::Here(crate::chatkord_v1::Here {})]);
        let said = Said { spans: &here_only, author: Some("bo"), mentions_replied: None };
        assert!(!mentions_of(said, reader).everyone, "away at the send");
        assert!(mentions_of(said, Reader { online_at_send: true, ..reader }).everyone);
        let reply = Said { spans: &Spans::NONE, author: Some("ada"), mentions_replied: Some("ada") };
        assert_eq!(mentions_of(reply, reader), Mentions { from_reader: true, reader: true, ..Mentions::default() });
    }

    #[test]
    fn the_server_half_knows_no_device_and_the_device_half_alerts_only_a_counted_mark() {
        let (server, channel) = (ServerSettings::default(), ChannelSettings::default());
        assert_eq!(tell(server, channel, me(), NOW), Mark::Counted);
        assert_eq!(tell(server, channel, Mentions { from_reader: true, ..me() }, NOW), Mark::None);
        let mute = ChannelSettings { mute: Some(Mute::Until(NOW + 1)), ..channel };
        assert_eq!(tell(server, mute, me(), NOW), Mark::Muted { mentioned: true });
        assert_eq!(tell(server, mute, me(), NOW + 1), Mark::Counted, "the mute ends at its time");
        for mark in [Mark::None, Mark::Quiet, Mark::Muted { mentioned: true }] {
            assert!(!alerts(mark, here()), "{mark:?} never alerts, whatever the device");
        }
        assert!(alerts(Mark::Counted, here()));
    }

    fn idle(seconds: u64) -> Device {
        Device::Idle { for_how_long: Duration::from_secs(seconds) }
    }

    #[test]
    fn a_device_alone_alerts() {
        assert!(alerts_here(Device::Active, &[]));
        assert!(alerts_here(idle(3_600), &[]), "alone, however long idle");
    }

    #[test]
    fn the_device_with_the_most_recent_input_alerts_and_the_others_do_not() {
        assert!(alerts_here(Device::Active, &[idle(30)]));
        assert!(!alerts_here(idle(30), &[Device::Active]));
        assert!(alerts_here(idle(30), &[idle(90), idle(600 - 1)]), "went idle last");
        assert!(!alerts_here(idle(90), &[idle(30)]));
        assert!(alerts_here(Device::Active, &[Device::Active]), "a tie alerts each");
        assert!(!alerts_here(idle(0), &[Device::Active]), "active beats idle even at the same instant");
        assert!(alerts_here(idle(60), &[idle(60)]));
    }

    #[test]
    fn every_device_alerts_once_all_have_been_idle_ten_minutes() {
        let limit = IDLE_EVERYWHERE.as_secs();
        assert_eq!(limit, 600, "ten minutes");
        assert!(!alerts_here(idle(limit + 5), &[idle(limit - 1)]), "one not yet ten minutes idle holds the rest back");
        assert!(alerts_here(idle(limit + 5), &[idle(limit)]), "all at or past the limit");
        assert!(alerts_here(idle(limit * 3), &[idle(limit), idle(limit + 1)]));
        assert!(!alerts_here(idle(limit + 5), &[Device::Active]), "an active device is the one in use");
    }

    #[test]
    fn settings_go_both_ways_on_the_wire_and_an_unsaid_level_or_mute_is_refused() {
        let server = ServerSettings {
            level: Level::All,
            suppress_everyone: true,
            suppress_roles: false,
            mute: Some(Mute::Forever),
        };
        assert_eq!(ServerSettings::of_wire(Some(&server.wire())), Ok(server));
        assert_eq!(ServerSettings::of_wire(None), Ok(ServerSettings::default()));
        let unsaid = ServerNotify::default();
        assert_eq!(ServerSettings::of_wire(Some(&unsaid)), Err(NotifyRefused));
        let channel = ChannelSettings { level: None, mute: Some(Mute::Until(NOW)) };
        assert_eq!(ChannelSettings::of_wire(&channel.wire("7".into())), Ok(channel));
        let empty_mute =
            ChannelNotify { mute: Some(crate::chatkord_v1::Mute { span: None }), ..ChannelNotify::default() };
        assert_eq!(ChannelSettings::of_wire(&empty_mute), Err(NotifyRefused));
        assert!(ChannelSettings::default().follows() && !channel.follows());
    }
}
