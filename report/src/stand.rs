//! A stand-in for an id: the same within one report, and nothing the id's text can be read back from.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::BuildHasher;

use crate::Fact;

/// The most stand-ins one report makes: past it a new id has none.
pub const MAX_STANDS: usize = 256;

/// The longest kind, in bytes: a report's row bound counts on it.
const MAX_KIND: usize = 32;

/// `kind-N`, where N counts the ids of that kind in the order the report met them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stand {
    kind: &'static str,
    number: u32,
}

impl fmt::Display for Stand {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}-{}", self.kind, self.number)
    }
}

/// The stand-ins of one report. It keeps a keyed hash of each id, drawn fresh for this value, never the id's text,
/// and is dropped with the report.
#[derive(Debug)]
pub struct Stands {
    key: RandomState,
    seen: HashMap<u64, Stand>,
    per_kind: HashMap<&'static str, u32>,
}

impl Default for Stands {
    fn default() -> Self {
        Self::new()
    }
}

impl Stands {
    /// No id met yet.
    #[must_use]
    pub fn new() -> Self {
        Self { key: RandomState::new(), seen: HashMap::new(), per_kind: HashMap::new() }
    }

    /// The stand-in for `id` of `kind` (a word like `member` or `server`): the same one each time within this value, a
    /// new number for each id first seen. `None` once [`MAX_STANDS`] are made, or when `kind` is empty or over 32
    /// bytes.
    pub fn of(&mut self, kind: &'static str, id: &str) -> Option<Stand> {
        if kind.is_empty() || kind.len() > MAX_KIND {
            return None;
        }
        let hash = self.key.hash_one((kind, id));
        if let Some(stand) = self.seen.get(&hash) {
            return Some(stand.clone());
        }
        if self.seen.len() >= MAX_STANDS {
            return None;
        }
        let number = self.per_kind.entry(kind).or_insert(0);
        *number += 1;
        let stand = Stand { kind, number: *number };
        self.seen.insert(hash, stand.clone());
        Some(stand)
    }

    /// [`Stands::of`] as a fact: [`Fact::Withheld`] when it makes none.
    pub fn fact(&mut self, kind: &'static str, id: &str) -> Fact {
        self.of(kind, id).map_or(Fact::Withheld, Fact::Stand)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADA: &str = "ada";
    const BOB: &str = "bob";

    #[test]
    fn stand_ins_count_in_first_seen_order_per_kind_and_hold() {
        let mut stands = Stands::new();
        let ada = stands.of("member", ADA).unwrap();
        let bob = stands.of("member", BOB).unwrap();
        let server = stands.of("server", ADA).unwrap();
        assert_eq!([ada.to_string(), bob.to_string(), server.to_string()], ["member-1", "member-2", "server-1"]);
        assert_eq!(stands.of("member", ADA), Some(ada));
    }

    #[test]
    fn a_stand_in_never_holds_the_id_and_a_second_report_numbers_afresh() {
        let id = "ada-secret-id-0123456789";
        let mut stands = Stands::new();
        let stand = stands.of("member", id).unwrap();
        assert!(!format!("{stand} {stand:?} {stands:?}").contains("secret"));
        assert_eq!(Stands::new().of("member", "another").unwrap().to_string(), "member-1");
    }

    #[test]
    fn the_limit_and_a_bad_kind_refuse() {
        let mut stands = Stands::new();
        for number in 0..MAX_STANDS {
            assert!(stands.of("member", &number.to_string()).is_some());
        }
        assert_eq!(stands.of("member", "one more"), None);
        assert!(stands.of("member", "0").is_some(), "one already made still answers");
        assert_eq!(stands.of("", "x"), None);
        assert_eq!(stands.of("a-kind-that-is-far-too-long-to-print", "x"), None);
    }
}
