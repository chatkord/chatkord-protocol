//! The server's own emoji and the Unicode ones a reaction names: a custom emoji's name, how many a server keeps and
//! the picture's bounds, and the shape a Unicode emoji has. The server holds no emoji database, so it checks the
//! shape alone; the client's picker offers only emoji its database knows.

use std::ops::RangeInclusive;

/// The most custom emoji one server holds, as Discord's highest tier allows.
pub const MAX_EMOJI: usize = 250;

/// The shortest a custom emoji's name may be, in bytes (it is ASCII).
pub const MIN_EMOJI_NAME: usize = 2;

/// The longest a custom emoji's name may be, in bytes.
pub const MAX_EMOJI_NAME: usize = 32;

/// A picture sent to be a custom emoji: at most this many bytes, refused before it is read.
pub const EMOJI_UPLOAD_BYTES: u64 = 256 * 1024;

/// A kept custom emoji is a PNG of this many pixels on a side, made by the server.
pub const EMOJI_SIDE: u32 = 128;

/// The longest a Unicode emoji a reaction names may be, in bytes.
pub const MAX_UNICODE_EMOJI_BYTES: usize = 32;

/// The most scalar values a Unicode emoji a reaction names may hold: a family of four with their skin tones fits.
pub const MAX_UNICODE_EMOJI_CHARS: usize = 16;

/// A custom emoji's name: 2 to 32 of ASCII letters, digits and `_`. Two names that differ only in case are one name
/// to the store, which keeps them unique so.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EmojiName(String);

/// Why a custom emoji's name was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiNameRefused {
    /// Under [`MIN_EMOJI_NAME`].
    TooShort,
    /// Over [`MAX_EMOJI_NAME`].
    TooLong,
    /// A character other than ASCII letters, digits and `_`.
    Characters,
}

impl EmojiName {
    /// The name as it is written, without its colons.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EmojiName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A custom emoji's name as asked for, untrimmed: refused whole when out of shape.
pub fn emoji_name(text: &str) -> Result<EmojiName, EmojiNameRefused> {
    if text.len() < MIN_EMOJI_NAME {
        return Err(EmojiNameRefused::TooShort);
    }
    if text.len() > MAX_EMOJI_NAME {
        return Err(EmojiNameRefused::TooLong);
    }
    if !text.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        return Err(EmojiNameRefused::Characters);
    }
    Ok(EmojiName(text.to_string()))
}

/// The scalar values an emoji is drawn from: the emoji blocks (pictographs, regional indicators and skin-tone
/// modifiers among them) and the older symbols Unicode gives an emoji presentation.
const PICTOGRAPHS: [RangeInclusive<char>; 31] = [
    '\u{00A9}'..='\u{00A9}',
    '\u{00AE}'..='\u{00AE}',
    '\u{203C}'..='\u{203C}',
    '\u{2049}'..='\u{2049}',
    '\u{2122}'..='\u{2122}',
    '\u{2139}'..='\u{2139}',
    '\u{2194}'..='\u{2199}',
    '\u{21A9}'..='\u{21AA}',
    '\u{231A}'..='\u{231B}',
    '\u{2328}'..='\u{2328}',
    '\u{23CF}'..='\u{23CF}',
    '\u{23E9}'..='\u{23F3}',
    '\u{23F8}'..='\u{23FA}',
    '\u{24C2}'..='\u{24C2}',
    '\u{25AA}'..='\u{25AB}',
    '\u{25B6}'..='\u{25B6}',
    '\u{25C0}'..='\u{25C0}',
    '\u{25FB}'..='\u{25FE}',
    '\u{2600}'..='\u{27BF}',
    '\u{2934}'..='\u{2935}',
    '\u{2B05}'..='\u{2B07}',
    '\u{2B1B}'..='\u{2B1C}',
    '\u{2B50}'..='\u{2B50}',
    '\u{2B55}'..='\u{2B55}',
    '\u{3030}'..='\u{3030}',
    '\u{303D}'..='\u{303D}',
    '\u{3297}'..='\u{3297}',
    '\u{3299}'..='\u{3299}',
    '\u{1F000}'..='\u{1F0FF}',
    '\u{1F100}'..='\u{1F2FF}',
    '\u{1F300}'..='\u{1FAFF}',
];

/// Joins two emoji into one.
const JOINER: char = '\u{200D}';
/// Asks for the emoji presentation of what it follows.
const EMOJI_SELECTOR: char = '\u{FE0F}';
/// Draws what it follows on a keycap.
const KEYCAP: char = '\u{20E3}';
/// The tag characters a subdivision's flag is spelled with, its cancel tag last.
const TAGS: RangeInclusive<char> = '\u{E0020}'..='\u{E007F}';

fn pictograph(character: char) -> bool {
    PICTOGRAPHS.iter().any(|range| range.contains(&character))
}

fn keycap_base(character: char) -> bool {
    matches!(character, '0'..='9' | '#' | '*')
}

/// Why a reaction's text was refused as a Unicode emoji.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnicodeEmojiRefused;

/// `text` as a Unicode emoji, by its shape alone: 1 to [`MAX_UNICODE_EMOJI_CHARS`] scalar values in at most
/// [`MAX_UNICODE_EMOJI_BYTES`], starting with a pictograph or a keycap's digit, `#` or `*`, each further one a
/// pictograph, a joiner before a pictograph, the emoji selector, a tag, or a keycap after its digit. It accepts what an
/// emoji is made of, not only emoji Unicode names: the client offers real ones, and the server refuses no more.
pub fn unicode_emoji(text: &str) -> Result<&str, UnicodeEmojiRefused> {
    if text.is_empty() || text.len() > MAX_UNICODE_EMOJI_BYTES {
        return Err(UnicodeEmojiRefused);
    }
    let mut held = ['\0'; MAX_UNICODE_EMOJI_CHARS];
    let mut count = 0;
    for character in text.chars() {
        *held.get_mut(count).ok_or(UnicodeEmojiRefused)? = character;
        count += 1;
    }
    let scalars = held.get(..count).ok_or(UnicodeEmojiRefused)?;
    let at = |index: Option<usize>| index.and_then(|index| scalars.get(index).copied());
    for (index, &character) in scalars.iter().enumerate() {
        let (before, after) = (at(index.checked_sub(1)), at(index.checked_add(1)));
        let shaped = if pictograph(character) {
            true
        } else if keycap_base(character) {
            after == Some(KEYCAP) || (after == Some(EMOJI_SELECTOR) && at(index.checked_add(2)) == Some(KEYCAP))
        } else if character == KEYCAP {
            before.is_some_and(keycap_base)
                || (before == Some(EMOJI_SELECTOR) && at(index.checked_sub(2)).is_some_and(keycap_base))
        } else if character == JOINER {
            index > 0 && after.is_some_and(pictograph)
        } else {
            index > 0 && (character == EMOJI_SELECTOR || TAGS.contains(&character))
        };
        if !shaped {
            return Err(UnicodeEmojiRefused);
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_emoji_name_is_two_to_thirty_two_letters_digits_or_underscores() {
        for good in ["ok", "party_parrot", "Cat9", &"a".repeat(MAX_EMOJI_NAME)] {
            assert_eq!(emoji_name(good).map(|name| name.to_string()), Ok(good.to_string()));
        }
        assert_eq!(emoji_name("a"), Err(EmojiNameRefused::TooShort));
        assert_eq!(emoji_name(&"a".repeat(MAX_EMOJI_NAME + 1)), Err(EmojiNameRefused::TooLong));
        for bad in ["no-dash", "sp ace", ":colon:", "ünï"] {
            assert_eq!(emoji_name(bad), Err(EmojiNameRefused::Characters), "{bad}");
        }
    }

    #[test]
    fn a_unicode_emoji_is_shaped_like_one_and_plain_text_is_not() {
        let good =
            ["🙂", "❤️", "👍🏽", "🇬🇷", "1️⃣", "#⃣", "👩‍👩‍👧‍👦", "🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}", "©️"];
        for emoji in good {
            assert_eq!(unicode_emoji(emoji), Ok(emoji), "{emoji:?}");
        }
        let bad = ["", "a", "1", "🙂a", "\u{200D}🙂", "🙂\u{200D}", "\u{FE0F}", "\u{20E3}", "🙂\u{20E3}", " 🙂"];
        for text in bad {
            assert_eq!(unicode_emoji(text), Err(UnicodeEmojiRefused), "{text:?}");
        }
        assert_eq!(unicode_emoji(&"🙂".repeat(8)).map(str::len), Ok(MAX_UNICODE_EMOJI_BYTES), "at the bound");
        assert_eq!(unicode_emoji(&"🙂".repeat(9)), Err(UnicodeEmojiRefused), "past the bytes");
        assert_eq!(unicode_emoji(&"©".repeat(MAX_UNICODE_EMOJI_CHARS + 1)), Err(UnicodeEmojiRefused), "past the count");
    }
}
