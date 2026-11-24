//! What a person typed to search the messages for, as both ends read it: bounded, shaped as a name is, and split into
//! the words a message must hold. No word of it is syntax to anything.

use super::{TextRefused, text_shape};

/// The most Unicode scalar values a search's query holds; a longer one is refused, never cut.
pub const MAX_SEARCH_CHARS: usize = 100;

/// Why a search's query was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryRefused {
    /// Over [`MAX_SEARCH_CHARS`].
    TooLong,
    /// A control or direction character.
    ControlCharacter,
    /// No word in it: empty, white space, or only marks no message is found by.
    NoWord,
}

/// A query proved in shape: at most [`MAX_SEARCH_CHARS`] scalar values, no control or direction character, and at
/// least one word, a run between white space that holds a letter or a digit. Made only by [`SearchQuery::parse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchQuery(Vec<String>);

impl SearchQuery {
    /// The query `typed` makes, refused when it is out of shape or holds no word.
    pub fn parse(typed: &str) -> Result<Self, QueryRefused> {
        let text = text_shape(typed, MAX_SEARCH_CHARS).map_err(|refused| match refused {
            TextRefused::Empty => QueryRefused::NoWord,
            TextRefused::ControlCharacter => QueryRefused::ControlCharacter,
            TextRefused::TooLong => QueryRefused::TooLong,
        })?;
        let words: Vec<String> = text
            .split_whitespace()
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .map(str::to_string)
            .collect();
        if words.is_empty() {
            return Err(QueryRefused::NoWord);
        }
        Ok(Self(words))
    }

    /// Its words in the order typed, at least one; the last is the one still being typed.
    pub fn words(&self) -> &[String] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_is_the_words_typed_in_order() {
        assert_eq!(
            SearchQuery::parse("  Hello   wor-ld \"q\" ").map(|query| query.words().to_vec()),
            Ok(vec!["Hello".into(), "wor-ld".into(), "\"q\"".into()])
        );
        assert_eq!(
            SearchQuery::parse("- * :").map(|query| query.words().len()),
            Err(QueryRefused::NoWord),
            "marks alone"
        );
        assert_eq!(SearchQuery::parse("a - b").map(|query| query.words().len()), Ok(2), "a mark alone is no word");
    }

    #[test]
    fn a_query_over_the_bound_or_with_a_control_character_or_no_word_is_refused_not_cut() {
        assert_eq!(SearchQuery::parse(&"a".repeat(MAX_SEARCH_CHARS)).map(|query| query.words().len()), Ok(1));
        assert_eq!(SearchQuery::parse(&"a".repeat(MAX_SEARCH_CHARS + 1)), Err(QueryRefused::TooLong));
        assert_eq!(
            SearchQuery::parse(&"é".repeat(MAX_SEARCH_CHARS)).map(|query| query.words().len()),
            Ok(1),
            "scalars, not bytes"
        );
        assert_eq!(SearchQuery::parse("a\nb"), Err(QueryRefused::ControlCharacter));
        assert_eq!(SearchQuery::parse("a\u{202E}b"), Err(QueryRefused::ControlCharacter));
        assert_eq!(SearchQuery::parse(""), Err(QueryRefused::NoWord));
        assert_eq!(SearchQuery::parse("   "), Err(QueryRefused::NoWord));
    }
}
