//! How a member is shown to the others and what their account chose, as both ends read it: the server publishes
//! [`shown`] for every member and keeps each account's [`Chosen`]; a window reads the same rule, so its own Do not
//! disturb and what others are told of it agree.

use crate::chatkord_v1::{Status, StatusChoice};
use crate::rules::MAX_STATUS_FOR;

/// What an account picks to be shown as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Online while signed in, away while idle on every device and in no voice place.
    Online,
    /// Away while signed in.
    Away,
    /// Not to be disturbed: the account's devices alert nothing.
    DoNotDisturb,
    /// Offline to every other member; the account still receives everything.
    Invisible,
}

impl Choice {
    /// The choice the wire names; none for `STATUS_CHOICE_UNSPECIFIED`.
    pub const fn from_wire(choice: StatusChoice) -> Option<Self> {
        match choice {
            StatusChoice::Unspecified => None,
            StatusChoice::Online => Some(Self::Online),
            StatusChoice::Away => Some(Self::Away),
            StatusChoice::DoNotDisturb => Some(Self::DoNotDisturb),
            StatusChoice::Invisible => Some(Self::Invisible),
        }
    }

    /// The wire's name for it.
    pub const fn to_wire(self) -> StatusChoice {
        match self {
            Self::Online => StatusChoice::Online,
            Self::Away => StatusChoice::Away,
            Self::DoNotDisturb => StatusChoice::DoNotDisturb,
            Self::Invisible => StatusChoice::Invisible,
        }
    }
}

/// An account's choice and when it ends by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chosen {
    /// What was picked.
    pub choice: Choice,
    /// When it ends and the account is [`Chosen::ONLINE`] again, in seconds since the Unix epoch; none, until changed.
    pub until: Option<i64>,
}

/// Why a `SetStatus` was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// `STATUS_CHOICE_UNSPECIFIED`.
    Unspecified,
    /// An end for Online, which has nothing to end.
    EndForOnline,
    /// An end at or before now.
    EndPassed,
    /// An end past [`MAX_STATUS_FOR`] from now.
    EndTooFar,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unspecified => "no status was chosen",
            Self::EndForOnline => "online has no end",
            Self::EndPassed => "the status ends before now",
            Self::EndTooFar => "the status ends past the longest a status lasts",
        })
    }
}

impl std::error::Error for Refused {}

impl Chosen {
    /// Online with no end: every account's choice until it makes one, and after the server starts.
    pub const ONLINE: Chosen = Chosen { choice: Choice::Online, until: None };

    /// The choice a `SetStatus` of `choice` until `until` makes at `now` (seconds since the Unix epoch): refused when it
    /// names no choice, gives Online an end, or ends at or before `now` or past [`MAX_STATUS_FOR`] after it.
    pub fn read(choice: StatusChoice, until: Option<i64>, now: i64) -> Result<Self, Refused> {
        let choice = Choice::from_wire(choice).ok_or(Refused::Unspecified)?;
        if let Some(end) = until {
            if choice == Choice::Online {
                return Err(Refused::EndForOnline);
            }
            if end <= now {
                return Err(Refused::EndPassed);
            }
            if end > now.saturating_add(MAX_STATUS_FOR) {
                return Err(Refused::EndTooFar);
            }
        }
        Ok(Self { choice, until })
    }

    /// The choice in force at `now`: [`Chosen::ONLINE`] once its end has come.
    pub const fn at(self, now: i64) -> Chosen {
        match self.until {
            Some(end) if end <= now => Self::ONLINE,
            _ => self,
        }
    }

    /// Do not disturb is in force at `now`.
    pub fn quiet(self, now: i64) -> bool {
        self.at(now).choice == Choice::DoNotDisturb
    }
}

/// How a member is shown to every other member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Signed in on no device, or invisible.
    Offline,
    /// Signed in and about.
    Online,
    /// Away by choice, or idle everywhere while in no voice place.
    Away,
    /// Not to be disturbed.
    DoNotDisturb,
}

impl Shown {
    /// The status the wire names; none for `STATUS_UNSPECIFIED`, which no server keeping the protocol sends.
    pub const fn from_wire(status: Status) -> Option<Self> {
        match status {
            Status::Unspecified => None,
            Status::Offline => Some(Self::Offline),
            Status::Online => Some(Self::Online),
            Status::Away => Some(Self::Away),
            Status::DoNotDisturb => Some(Self::DoNotDisturb),
        }
    }

    /// The wire's name for it.
    pub const fn to_wire(self) -> Status {
        match self {
            Self::Offline => Status::Offline,
            Self::Online => Status::Online,
            Self::Away => Status::Away,
            Self::DoNotDisturb => Status::DoNotDisturb,
        }
    }

    /// The member is signed in and shown so: any status but [`Shown::Offline`].
    pub const fn online(self) -> bool {
        !matches!(self, Self::Offline)
    }
}

/// How an account that chose `chosen` is shown at `now`: `seated` when signed in on a device, `idle_everywhere` when
/// every device has had no input for [`crate::notify::IDLE_EVERYWHERE`], `in_voice` when it holds a voice place.
pub fn shown(chosen: Chosen, now: i64, seated: bool, idle_everywhere: bool, in_voice: bool) -> Shown {
    if !seated {
        return Shown::Offline;
    }
    match chosen.at(now).choice {
        Choice::Invisible => Shown::Offline,
        Choice::DoNotDisturb => Shown::DoNotDisturb,
        Choice::Away => Shown::Away,
        Choice::Online if idle_everywhere && !in_voice => Shown::Away,
        Choice::Online => Shown::Online,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000;

    #[test]
    fn every_choice_and_status_goes_both_ways_and_unspecified_is_none() {
        for choice in [Choice::Online, Choice::Away, Choice::DoNotDisturb, Choice::Invisible] {
            assert_eq!(Choice::from_wire(choice.to_wire()), Some(choice));
        }
        assert_eq!(Choice::from_wire(StatusChoice::Unspecified), None);
        for status in [Shown::Offline, Shown::Online, Shown::Away, Shown::DoNotDisturb] {
            assert_eq!(Shown::from_wire(status.to_wire()), Some(status));
        }
        assert_eq!(Shown::from_wire(Status::Unspecified), None);
    }

    #[test]
    fn read_refuses_no_choice_an_end_for_online_a_past_end_and_one_too_far() {
        assert_eq!(Chosen::read(StatusChoice::Unspecified, None, NOW), Err(Refused::Unspecified));
        assert_eq!(Chosen::read(StatusChoice::Online, Some(NOW + 60), NOW), Err(Refused::EndForOnline));
        assert_eq!(Chosen::read(StatusChoice::Away, Some(NOW), NOW), Err(Refused::EndPassed), "now is not after now");
        assert_eq!(Chosen::read(StatusChoice::Away, Some(NOW - 1), NOW), Err(Refused::EndPassed));
        let far = NOW + MAX_STATUS_FOR;
        assert_eq!(Chosen::read(StatusChoice::Away, Some(far + 1), NOW), Err(Refused::EndTooFar));
        assert_eq!(
            Chosen::read(StatusChoice::DoNotDisturb, Some(far), NOW),
            Ok(Chosen { choice: Choice::DoNotDisturb, until: Some(far) }),
            "the longest end is kept"
        );
        assert_eq!(Chosen::read(StatusChoice::Online, None, NOW), Ok(Chosen::ONLINE));
        assert_eq!(
            Chosen::read(StatusChoice::Invisible, None, NOW),
            Ok(Chosen { choice: Choice::Invisible, until: None })
        );
    }

    #[test]
    fn until_tomorrow_at_eight_always_fits() {
        const { assert!(MAX_STATUS_FOR >= (24 + 8 + 1) * 60 * 60, "from midnight: a day, eight hours and a clock change") };
    }

    #[test]
    fn an_ended_choice_is_online_and_quiet_holds_only_while_do_not_disturb_lasts() {
        let quiet = Chosen { choice: Choice::DoNotDisturb, until: Some(NOW + 60) };
        assert!(quiet.quiet(NOW));
        assert!(quiet.quiet(NOW + 59));
        assert!(!quiet.quiet(NOW + 60), "ended at its time");
        assert_eq!(quiet.at(NOW + 60), Chosen::ONLINE);
        assert_eq!(quiet.at(NOW), quiet);
        let forever = Chosen { choice: Choice::DoNotDisturb, until: None };
        assert!(forever.quiet(i64::MAX));
        assert!(!Chosen { choice: Choice::Away, until: None }.quiet(NOW));
        assert!(!Chosen::ONLINE.quiet(NOW));
    }

    #[test]
    fn shown_is_offline_unseated_or_invisible_and_away_when_idle_everywhere_outside_voice() {
        let pick = |choice| Chosen { choice, until: None };
        for choice in [Choice::Online, Choice::Away, Choice::DoNotDisturb, Choice::Invisible] {
            assert_eq!(shown(pick(choice), NOW, false, false, false), Shown::Offline, "{choice:?} signed out");
        }
        assert_eq!(shown(pick(Choice::Invisible), NOW, true, false, true), Shown::Offline, "in voice too");
        assert_eq!(shown(pick(Choice::DoNotDisturb), NOW, true, true, false), Shown::DoNotDisturb);
        assert_eq!(shown(pick(Choice::Away), NOW, true, false, true), Shown::Away);
        assert_eq!(shown(Chosen::ONLINE, NOW, true, false, false), Shown::Online);
        assert_eq!(shown(Chosen::ONLINE, NOW, true, true, false), Shown::Away, "idle everywhere");
        assert_eq!(shown(Chosen::ONLINE, NOW, true, true, true), Shown::Online, "idle but in a voice place");
        let ended = Chosen { choice: Choice::Invisible, until: Some(NOW) };
        assert_eq!(shown(ended, NOW, true, false, false), Shown::Online, "an ended choice is online");
        assert!(!Shown::Offline.online() && Shown::Away.online() && Shown::DoNotDisturb.online());
    }
}
