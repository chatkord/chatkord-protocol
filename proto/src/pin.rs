//! A server's key as a client trusts it on first use and the server names it in its log: the SHA-256 of its
//! certificate's SubjectPublicKeyInfo, so a certificate renewed on the same key keeps the same pin.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The SHA-256 of a certificate's SubjectPublicKeyInfo. Each end hashes with its own crypto; this type only carries,
/// writes and reads the digest, as 64 lower-case hexadecimal digits where it is kept and as 16 upper-case groups of
/// four where a person reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyPin([u8; KeyPin::LEN]);

/// Why a kept pin was refused: anything but 64 lower-case hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPinRefused;

impl KeyPin {
    /// The digest's length in bytes.
    pub const LEN: usize = 32;

    /// Digits in one group of [`KeyPin`]'s `Display`.
    const GROUP: usize = 4;

    /// Groups on one of [`KeyPin::rows`].
    const ROW: usize = 4;

    /// The pin a SHA-256 digest of a SubjectPublicKeyInfo is.
    pub const fn from_digest(digest: [u8; KeyPin::LEN]) -> KeyPin {
        KeyPin(digest)
    }

    /// Exactly 64 lower-case hexadecimal digits, as [`KeyPin`] is kept; anything else is refused whole.
    pub fn parse(text: &str) -> Result<KeyPin, KeyPinRefused> {
        if text.len() != 2 * KeyPin::LEN || !text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(KeyPinRefused);
        }
        let mut bytes = [0u8; KeyPin::LEN];
        for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
            let pair = std::str::from_utf8(pair).map_err(|_| KeyPinRefused)?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| KeyPinRefused)?;
        }
        Ok(KeyPin(bytes))
    }

    /// The pin as a person reads it, in four rows of four groups: the same groups as `Display`, broken for a narrow
    /// box.
    pub fn rows(&self) -> impl Iterator<Item = String> + '_ {
        let text = self.to_string();
        let groups: Vec<String> = text.split(' ').map(str::to_owned).collect();
        (0..groups.len().div_ceil(KeyPin::ROW)).map(move |row| {
            groups.iter().skip(row * KeyPin::ROW).take(KeyPin::ROW).cloned().collect::<Vec<_>>().join(" ")
        })
    }

    fn kept(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// Sixteen groups of four upper-case hexadecimal digits, one space between: what the window shows and the server logs.
impl fmt::Display for KeyPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits: String = self.0.iter().map(|byte| format!("{byte:02X}")).collect();
        for (n, group) in digits.as_bytes().chunks(KeyPin::GROUP).enumerate() {
            if n > 0 {
                f.write_str(" ")?;
            }
            f.write_str(std::str::from_utf8(group).map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl Serialize for KeyPin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.kept())
    }
}

impl<'de> Deserialize<'de> for KeyPin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<KeyPin, D::Error> {
        let text = String::deserialize(deserializer)?;
        KeyPin::parse(&text).map_err(|_| serde::de::Error::custom("not 64 lower-case hexadecimal digits"))
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyPin, KeyPinRefused};

    const DIGEST: [u8; KeyPin::LEN] = [
        0xb4, 0x1c, 0x9e, 0x07, 0x33, 0xaf, 0x5d, 0x20, 0x8c, 0x6e, 0xf1, 0xa9, 0x04, 0x73, 0x2b, 0x5e, 0xd9, 0xc2,
        0x6f, 0x18, 0xa0, 0x7b, 0x4e, 0x95, 0x13, 0xf6, 0xc8, 0xd4, 0x7a, 0x20, 0xe3, 0xb1,
    ];

    #[test]
    fn a_person_reads_sixteen_upper_case_groups_in_four_rows() {
        let pin = KeyPin::from_digest(DIGEST);
        assert_eq!(pin.to_string(), "B41C 9E07 33AF 5D20 8C6E F1A9 0473 2B5E D9C2 6F18 A07B 4E95 13F6 C8D4 7A20 E3B1");
        let rows: Vec<String> = pin.rows().collect();
        assert_eq!(rows, ["B41C 9E07 33AF 5D20", "8C6E F1A9 0473 2B5E", "D9C2 6F18 A07B 4E95", "13F6 C8D4 7A20 E3B1"]);
    }

    #[test]
    fn a_pin_is_kept_as_lower_case_hex_and_read_back_whole() {
        let pin = KeyPin::from_digest(DIGEST);
        let kept = serde_json::to_string(&pin).expect("written");
        assert_eq!(kept, "\"b41c9e0733af5d208c6ef1a904732b5ed9c26f18a07b4e9513f6c8d47a20e3b1\"");
        assert_eq!(serde_json::from_str::<KeyPin>(&kept).expect("read"), pin);
    }

    #[test]
    fn anything_but_sixty_four_lower_case_digits_is_refused() {
        let upper = "B41C9E0733AF5D208C6EF1A904732B5ED9C26F18A07B4E9513F6C8D47A20E3B1";
        for text in ["", "b41c", upper, &format!("{}0", upper.to_lowercase()), &"g".repeat(64)] {
            assert_eq!(KeyPin::parse(text), Err(KeyPinRefused), "{text}");
        }
    }
}
