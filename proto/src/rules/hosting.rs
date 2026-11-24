//! Where and how a server is hosted, and which of its features an administrator may switch: the one set of bounds the
//! store, the server and the window read, so a value is refused in the same words wherever it is typed.

use crate::chatkord_v1::{Hosting as WireHosting, Switch as WireSwitch};
use std::fmt;
use std::num::NonZeroU16;

/// The longest public address: a 253-byte host name, a colon and a port.
pub const MAX_PUBLIC_ADDRESS_BYTES: usize = 259;

/// Where clients reach this server, `host[:port]`, as a join link names it. Made only by [`PublicAddress::parse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicAddress(String);

/// Text that is not a public address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicAddressRefused;

impl fmt::Display for PublicAddressRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("The public address must be a host and an optional port, like example.invalid:8443.")
    }
}

impl PublicAddress {
    /// The address as written, trimmed.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A host name, an IPv4 address or a bracketed IPv6 one, and an optional port; no scheme or path, at most
    /// [`MAX_PUBLIC_ADDRESS_BYTES`].
    pub fn parse(text: &str) -> Result<Self, PublicAddressRefused> {
        let text = text.trim();
        let plain = |part: &str| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':'))
        };
        let (host, port) = match text.strip_prefix('[') {
            Some(bracketed) => {
                let (inside, after) = bracketed.split_once(']').ok_or(PublicAddressRefused)?;
                let port = match after {
                    "" => None,
                    _ => Some(after.strip_prefix(':').ok_or(PublicAddressRefused)?),
                };
                (inside.parse::<std::net::Ipv6Addr>().map_err(|_| PublicAddressRefused)?.to_string(), port)
            }
            None => match text.split_once(':') {
                Some((host, port)) => (host.to_string(), Some(port)),
                None => (text.to_string(), None),
            },
        };
        let port_ok = port.is_none_or(|port| port.parse::<NonZeroU16>().is_ok());
        if text.len() > MAX_PUBLIC_ADDRESS_BYTES
            || !plain(&host)
            || (!text.starts_with('[') && host.contains(':'))
            || !port_ok
        {
            return Err(PublicAddressRefused);
        }
        Ok(Self(text.to_string()))
    }
}

/// One of the numbers of [`Hosting`], each held between a minimum and a ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostingField {
    /// One account's files, and one webhook's, in MiB; set to none, they have no limit.
    AccountFilesMax,
    /// One attachment, in MiB.
    AttachmentMax,
}

const MIB: u64 = 1024 * 1024;
const TIB_IN_MIB: u32 = 1024 * 1024;
const ATTACHMENT_CEILING_MIB: u32 = 4096;
const DEFAULT_ATTACHMENT_MIB: u32 = 512;
const DEFAULT_ACCOUNT_FILES_MIB: u32 = 10 * 1024;
const _: () = assert!(
    ATTACHMENT_CEILING_MIB as u64 * MIB == crate::files::ATTACHMENT_MOST_BYTES,
    "the attachment ceiling is the most any server takes"
);

impl HostingField {
    /// Every field, in the order they are checked.
    pub const ALL: [HostingField; 2] = [Self::AccountFilesMax, Self::AttachmentMax];

    /// The field's name on the wire (`Hosting`'s).
    pub const fn name(self) -> &'static str {
        match self {
            Self::AccountFilesMax => "account_files_max_mib",
            Self::AttachmentMax => "attachment_max_mib",
        }
    }

    /// The least the field may be when it is set.
    pub const fn minimum(self) -> u32 {
        1
    }

    /// The most the field may be.
    pub const fn ceiling(self) -> u32 {
        match self {
            Self::AccountFilesMax => TIB_IN_MIB,
            Self::AttachmentMax => ATTACHMENT_CEILING_MIB,
        }
    }

    /// What the field is when no one set it.
    pub const fn default_value(self) -> u32 {
        match self {
            Self::AccountFilesMax => DEFAULT_ACCOUNT_FILES_MIB,
            Self::AttachmentMax => DEFAULT_ATTACHMENT_MIB,
        }
    }

    /// Whether an owner may set the field to none, which is no limit.
    pub const fn may_be_unlimited(self) -> bool {
        match self {
            Self::AccountFilesMax => true,
            Self::AttachmentMax => false,
        }
    }

    /// The field's value in `wire`.
    pub fn of(self, wire: &WireHosting) -> Option<u32> {
        match self {
            Self::AccountFilesMax => wire.account_files_max_mib,
            Self::AttachmentMax => Some(wire.attachment_max_mib),
        }
    }

    /// `wire` with the field set to `value`, unchecked: [`Hosting::parse`] judges the whole. An attachment cannot be
    /// unset, so none leaves it at zero, which is below its minimum.
    pub fn set(self, wire: &mut WireHosting, value: Option<u32>) {
        match self {
            Self::AccountFilesMax => wire.account_files_max_mib = value,
            Self::AttachmentMax => wire.attachment_max_mib = value.unwrap_or(0),
        }
    }
}

/// Why a [`Hosting`] was refused; nothing is changed or cut to make a value fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostingRefused {
    /// The public address is not `host[:port]`.
    PublicAddress,
    /// A number is below its minimum or above its ceiling.
    OutOfRange(HostingField),
    /// A number is above the one it is held within: an attachment within the account's files.
    Above {
        /// The number that is too large.
        field: HostingField,
        /// The number it must not pass.
        within: HostingField,
    },
}

impl fmt::Display for HostingRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicAddress => write!(f, "{PublicAddressRefused}"),
            Self::OutOfRange(field) => {
                write!(f, "{} must be {} to {}.", field.name(), field.minimum(), field.ceiling())
            }
            Self::Above { field, within } => write!(f, "{} cannot be more than {}.", field.name(), within.name()),
        }
    }
}

impl std::error::Error for HostingRefused {}

/// A server's hosting facts: its public address and the files an owner allows, each number held between its
/// [`HostingField`] bounds. Made only by [`Hosting::parse`] or [`Hosting::DEFAULT`], so every one in hand is valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hosting {
    public_address: Option<PublicAddress>,
    account_files_max_mib: Option<u32>,
    attachment_max_mib: u32,
}

impl Default for Hosting {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Hosting {
    /// What a server is before anyone sets anything: no public address, and the default account's files and
    /// attachment.
    pub const DEFAULT: Hosting = Hosting {
        public_address: None,
        account_files_max_mib: Some(HostingField::AccountFilesMax.default_value()),
        attachment_max_mib: HostingField::AttachmentMax.default_value(),
    };

    /// The facts `wire` states, or why they are refused: a number outside its bounds (the first in
    /// [`HostingField::ALL`]'s order), or an attachment above the account's limit.
    pub fn parse(wire: &WireHosting) -> Result<Self, HostingRefused> {
        let public_address = wire
            .public_address
            .as_deref()
            .map(PublicAddress::parse)
            .transpose()
            .map_err(|PublicAddressRefused| HostingRefused::PublicAddress)?;
        for field in HostingField::ALL {
            if let Some(value) = field.of(wire)
                && !(field.minimum()..=field.ceiling()).contains(&value)
            {
                return Err(HostingRefused::OutOfRange(field));
            }
        }
        if let Some(account) = wire.account_files_max_mib
            && wire.attachment_max_mib > account
        {
            return Err(HostingRefused::Above {
                field: HostingField::AttachmentMax,
                within: HostingField::AccountFilesMax,
            });
        }
        Ok(Self {
            public_address,
            account_files_max_mib: wire.account_files_max_mib,
            attachment_max_mib: wire.attachment_max_mib,
        })
    }

    /// The facts as the wire states them.
    pub fn to_wire(&self) -> WireHosting {
        WireHosting {
            public_address: self.public_address.as_ref().map(|address| address.as_str().to_string()),
            account_files_max_mib: self.account_files_max_mib,
            attachment_max_mib: self.attachment_max_mib,
        }
    }

    /// Where clients reach the server, when it is known.
    pub fn public_address(&self) -> Option<&PublicAddress> {
        self.public_address.as_ref()
    }

    /// One account's files, and one webhook's, in bytes; none, there is no limit.
    pub fn account_files_max_bytes(&self) -> Option<u64> {
        self.account_files_max_mib.map(|mib| u64::from(mib) * MIB)
    }

    /// One attachment, in bytes.
    pub fn attachment_max_bytes(&self) -> u64 {
        u64::from(self.attachment_max_mib) * MIB
    }
}

/// A set of switches, one bit each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwitchSet(u8);

const _: () = assert!(Switch::ALL.len() <= u8::BITS as usize, "every switch has its own bit in `SwitchSet`");

impl SwitchSet {
    const fn bit(switch: Switch) -> u8 {
        1 << switch as u8
    }

    /// The set with `switch` in it too.
    #[must_use]
    pub const fn with(self, switch: Switch) -> Self {
        Self(self.0 | Self::bit(switch))
    }

    /// The set without `switch`.
    #[must_use]
    pub const fn without(self, switch: Switch) -> Self {
        Self(self.0 & !Self::bit(switch))
    }

    /// Whether `switch` is in the set.
    pub const fn contains(self, switch: Switch) -> bool {
        self.0 & Self::bit(switch) != 0
    }

    /// Whether the set holds no switch.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Its switches, in the order of [`Switch::ALL`].
    pub fn iter(self) -> impl Iterator<Item = Switch> {
        Switch::ALL.into_iter().filter(move |switch| self.contains(*switch))
    }
}

/// A part of the server an administrator may switch off: the greeting says it is off, and its ops are refused
/// `NOT_AUTHORIZED`; what it stored is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Switch {
    /// Private conversations between members.
    Conversations,
    /// Threads started on a message.
    Threads,
    /// Link previews on a message.
    LinkPreviews,
    /// Search of messages and of their files' names.
    Search,
    /// The note written when a member joins.
    Welcome,
}

impl Switch {
    /// Every switch, in the order the store writes them.
    pub const ALL: [Switch; 5] = [Self::Conversations, Self::Threads, Self::LinkPreviews, Self::Search, Self::Welcome];

    /// The switch's word in the store: its name on the wire.
    pub fn word(self) -> &'static str {
        self.wire().as_str_name()
    }

    /// The switch `word` names, or none for a word this build does not know.
    pub fn of_word(word: &str) -> Option<Switch> {
        Self::ALL.into_iter().find(|switch| switch.word() == word)
    }

    /// Whether the switch is on when no one has set it: every one is.
    pub const fn default_on(self) -> bool {
        true
    }

    /// The switch as the wire names it.
    pub const fn wire(self) -> WireSwitch {
        match self {
            Self::Conversations => WireSwitch::Conversations,
            Self::Threads => WireSwitch::Threads,
            Self::LinkPreviews => WireSwitch::LinkPreviews,
            Self::Search => WireSwitch::Search,
            Self::Welcome => WireSwitch::WelcomeNote,
        }
    }

    /// The switch a wire number names; none for `UNSPECIFIED` or a number this build does not know.
    pub fn of_wire(number: i32) -> Option<Switch> {
        match WireSwitch::try_from(number).ok()? {
            WireSwitch::Unspecified => None,
            WireSwitch::Conversations => Some(Self::Conversations),
            WireSwitch::Threads => Some(Self::Threads),
            WireSwitch::LinkPreviews => Some(Self::LinkPreviews),
            WireSwitch::Search => Some(Self::Search),
            WireSwitch::WelcomeNote => Some(Self::Welcome),
        }
    }

    /// The switches a greeting or a `ServerChanged` says are off, each once; a number this build does not know is
    /// passed over.
    pub fn off(listed: &[i32]) -> Vec<Switch> {
        Self::ALL
            .into_iter()
            .filter(|switch| listed.iter().any(|&number| Self::of_wire(number) == Some(*switch)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire() -> WireHosting {
        Hosting::DEFAULT.to_wire()
    }

    #[test]
    fn a_public_address_is_a_host_and_a_port_and_nothing_else() {
        for good in ["example.invalid", "example.invalid:8443", "192.0.2.7:443", "[2001:db8::7]", "[2001:db8::7]:8443"]
        {
            assert_eq!(PublicAddress::parse(good).map(|address| address.as_str().to_string()), Ok(good.to_string()));
        }
        for bad in [
            "https://example.invalid",
            "example.invalid/v1",
            "example.invalid:0",
            "example.invalid:65536",
            "example.invalid:",
            "2001:db8::7",
            "[2001:db8::7]8443",
            "a@example.invalid",
            "example invalid",
            ":8443",
            "",
        ] {
            assert_eq!(PublicAddress::parse(bad), Err(PublicAddressRefused), "{bad}");
        }
        assert!(PublicAddress::parse(&"a".repeat(MAX_PUBLIC_ADDRESS_BYTES + 1)).is_err());
    }

    #[test]
    fn the_default_is_valid_and_goes_both_ways() {
        assert_eq!(Hosting::parse(&Hosting::DEFAULT.to_wire()), Ok(Hosting::DEFAULT));
        assert_eq!(Hosting::default().attachment_max_bytes(), 512 * MIB);
        let set = WireHosting { public_address: Some("example.invalid:8443".into()), ..wire() };
        let parsed = Hosting::parse(&set).expect("valid");
        assert_eq!(parsed.public_address().map(PublicAddress::as_str), Some("example.invalid:8443"));
        assert_eq!(parsed.to_wire(), set);
    }

    #[test]
    fn the_default_hosting_bounds_an_account_s_files() {
        assert_eq!(Hosting::DEFAULT.account_files_max_bytes(), Some(10 * 1024 * MIB));
        assert_eq!(Hosting::DEFAULT.to_wire().account_files_max_mib, Some(10 * 1024));
        assert_eq!(Hosting::parse(&wire()), Ok(Hosting::DEFAULT));
        let limited = WireHosting { account_files_max_mib: Some(4096), ..wire() };
        let parsed = Hosting::parse(&limited).expect("valid");
        assert_eq!(parsed.account_files_max_bytes(), Some(4096 * MIB));
        assert_eq!(parsed.to_wire(), limited);
        let unlimited = WireHosting { account_files_max_mib: None, ..wire() };
        assert_eq!(Hosting::parse(&unlimited).map(|hosting| hosting.account_files_max_bytes()), Ok(None));
        assert_eq!(
            Hosting::parse(&unlimited).map(|hosting| hosting.to_wire()),
            Ok(unlimited),
            "none is an owner's choice"
        );
        let may_be_none: Vec<_> = HostingField::ALL.into_iter().filter(|field| field.may_be_unlimited()).collect();
        assert_eq!(may_be_none, [HostingField::AccountFilesMax], "the wire's one optional number");
    }

    #[test]
    fn hosting_values_above_their_ceiling_are_refused_never_cut() {
        for field in HostingField::ALL {
            let at = |value: u32| {
                let mut hosting = wire();
                field.set(&mut hosting, Some(value));
                assert_eq!(field.of(&hosting), Some(value));
                hosting
            };
            assert_eq!(Hosting::parse(&at(field.ceiling() + 1)), Err(HostingRefused::OutOfRange(field)), "{field:?}");
            assert_eq!(
                Hosting::parse(&at(field.minimum() - 1)),
                Err(HostingRefused::OutOfRange(field)),
                "{field:?} below its minimum"
            );
            assert_eq!(Hosting::parse(&at(u32::MAX)), Err(HostingRefused::OutOfRange(field)), "{field:?}");
        }
    }

    #[test]
    fn an_attachment_is_held_within_a_set_account_limit_and_within_nothing_else() {
        let attachment = WireHosting { account_files_max_mib: Some(40), attachment_max_mib: 41, ..wire() };
        assert_eq!(
            Hosting::parse(&attachment),
            Err(HostingRefused::Above { field: HostingField::AttachmentMax, within: HostingField::AccountFilesMax })
        );
        let equal = WireHosting { account_files_max_mib: Some(41), attachment_max_mib: 41, ..wire() };
        assert!(Hosting::parse(&equal).is_ok());
        let unlimited = WireHosting { account_files_max_mib: None, attachment_max_mib: 4096, ..wire() };
        assert!(Hosting::parse(&unlimited).is_ok());
    }

    #[test]
    fn a_refused_public_address_is_its_own_refusal() {
        let bad = WireHosting { public_address: Some("https://example.invalid".into()), ..wire() };
        assert_eq!(Hosting::parse(&bad), Err(HostingRefused::PublicAddress));
        assert!(
            HostingRefused::OutOfRange(HostingField::AttachmentMax)
                .to_string()
                .contains("attachment_max_mib must be 1 to 4096")
        );
    }

    #[test]
    fn every_default_lies_within_its_bounds() {
        for field in HostingField::ALL {
            assert!((field.minimum()..=field.ceiling()).contains(&field.default_value()), "{field:?}");
        }
    }

    #[test]
    fn switches_go_both_ways_and_a_number_no_switch_holds_is_passed_over() {
        for switch in Switch::ALL {
            assert_eq!(Switch::of_word(switch.word()), Some(switch));
            assert_eq!(Switch::of_wire(switch.wire() as i32), Some(switch));
            assert!(switch.default_on());
        }
        assert_eq!(Switch::of_word("WEBHOOKS"), None);
        assert_eq!(Switch::of_wire(0), None);
        assert_eq!(Switch::off(&[WireSwitch::Search as i32, 999, 0, WireSwitch::Search as i32]), [Switch::Search]);
    }
}
