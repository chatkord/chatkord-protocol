//! Which strings a peer supplies may be written into an SDP line, and how a peer's mediasoup parameters are read.

use crate::envelope::MAX_SFU_JSON_BYTES;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// The longest RTCP canonical name either end accepts.
pub const MAX_CNAME: usize = 64;
/// The most format parameters (`a=fmtp`) one codec may carry.
pub const MAX_FMTP_ENTRIES: usize = 32;
/// The longest format-parameter key or string value either end accepts.
pub const MAX_FMTP_TOKEN: usize = 64;

/// Whether `text` is 1 to `max` bytes of `[A-Za-z0-9+/=_.-]`: no space, `;` or line break that could
/// start another attribute or line.
pub fn is_token(text: &str, max: usize) -> bool {
    (1..=max).contains(&text.len())
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'_' | b'.' | b'-'))
}

/// Why a peer's mediasoup parameters were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfuJsonRefused {
    /// Over [`MAX_SFU_JSON_BYTES`].
    TooLong,
    /// Not JSON of the shape asked for.
    NotJson,
    /// A byte outside ASCII, which mediasoup's parameters never hold.
    NotAscii,
}

/// A peer's mediasoup parameters as `T`: refused over [`MAX_SFU_JSON_BYTES`], when they are not JSON of that shape,
/// or when any string in them is not ASCII, as every string mediasoup's parameters carry is.
pub fn sfu_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SfuJsonRefused> {
    if bytes.len() > MAX_SFU_JSON_BYTES {
        return Err(SfuJsonRefused::TooLong);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| SfuJsonRefused::NotJson)?;
    // mediasoup-types 0.10.0 slices a DTLS fingerprint by byte index, so a wider character panics its parse.
    if !all_ascii(&value) {
        return Err(SfuJsonRefused::NotAscii);
    }
    serde_json::from_value(value).map_err(|_| SfuJsonRefused::NotJson)
}

fn all_ascii(value: &Value) -> bool {
    match value {
        Value::String(text) => text.is_ascii(),
        Value::Array(items) => items.iter().all(all_ascii),
        Value::Object(fields) => fields.iter().all(|(key, field)| key.is_ascii() && all_ascii(field)),
        Value::Null | Value::Bool(_) | Value::Number(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peers_parameters_are_bounded_json_of_ascii_strings() {
        assert_eq!(sfu_json::<Value>(br#"{"value":"AB:CD","n":[1,"x"]}"#).map(drop), Ok(()));
        assert_eq!(sfu_json::<Value>(&vec![b' '; MAX_SFU_JSON_BYTES + 1]), Err(SfuJsonRefused::TooLong));
        assert_eq!(sfu_json::<Value>(b"{"), Err(SfuJsonRefused::NotJson));
        assert_eq!(sfu_json::<u8>(br#""x""#), Err(SfuJsonRefused::NotJson));
        for wide in ["{\"value\":\"A\u{1b4}A\"}", r#"{"value":"AƴA"}"#, "{\"k\u{e9}y\":1}", r#"[["é"]]"#] {
            assert_eq!(sfu_json::<Value>(wide.as_bytes()), Err(SfuJsonRefused::NotAscii), "{wide}");
        }
    }

    #[test]
    fn a_token_is_short_and_has_nothing_that_ends_an_attribute() {
        assert!(is_token("profile-level-id", MAX_FMTP_TOKEN));
        assert!(is_token("42e01f", MAX_FMTP_TOKEN));
        assert!(is_token("Zm9vYmFyQmF6cXV4+/==", MAX_CNAME));
        for bad in ["", "a b", "1;x=y", "a\r\nb", "a\tb", "é", "a,b"] {
            assert!(!is_token(bad, MAX_FMTP_TOKEN), "{bad:?}");
        }
        assert!(is_token(&"a".repeat(MAX_CNAME), MAX_CNAME));
        assert!(!is_token(&"a".repeat(MAX_CNAME + 1), MAX_CNAME));
    }
}
