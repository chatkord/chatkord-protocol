//! The closed set of what a log line or a report may say, and the one trait that lets a value into it.

use std::fmt;

use crate::{Product, Stand};

/// The longest word, in bytes: a report's row bound counts on it.
pub(crate) const MAX_WORD: usize = 64;

/// A word written in the source: made only by [`word!`](crate::word), whose argument must be a string literal, so no
/// runtime text can become one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Word(&'static str);

impl Word {
    /// The word of a literal; use [`word!`](crate::word), which checks at build time that its argument is a literal.
    ///
    /// # Panics
    ///
    /// At build time, when the macro evaluates it in a constant, if the text is empty, longer than 64 bytes or holds a
    /// byte that is not printable ASCII.
    #[doc(hidden)]
    #[must_use]
    pub const fn __literal(text: &'static str) -> Self {
        let bytes = text.as_bytes();
        assert!(!bytes.is_empty() && bytes.len() <= MAX_WORD, "a word is 1 to 64 bytes");
        let mut at = 0;
        while at < bytes.len() {
            assert!(bytes[at] >= b' ' && bytes[at] <= b'~', "a word is printable ASCII");
            at += 1;
        }
        Self(text)
    }

    /// The text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Word {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.0)
    }
}

/// A [`Word`] from a string literal, checked when the program is built.
#[macro_export]
macro_rules! word {
    ($text:literal) => {{
        const WORD: $crate::Word = $crate::Word::__literal($text);
        WORD
    }};
}

/// What a report or a log line may hold: nothing else is ever written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fact {
    /// A count, a size or a duration in a stated unit.
    Count(u64),
    /// A number that can be below zero.
    Signed(i64),
    /// A yes or a no.
    Flag(bool),
    /// A word the source wrote.
    Word(Word),
    /// A GPU, driver or system name that passed [`Product::parse`].
    Product(Product),
    /// A stand-in for an id, the same within one report.
    Stand(Stand),
    /// A product name that was refused, or a stand-in past the limit.
    Withheld,
    /// A value that is not there.
    Absent,
    /// A variant of an enum with the fact of its one `#[public]` field, shown as `Status(404)`.
    Case(Case),
}

/// A variant's word with the fact inside it, one level deep: made by [`Fact::case`], which shows a case inside a case as
/// its word alone, so a row's length is bounded by what its parts allow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    word: Word,
    inner: Box<Fact>,
}

impl Fact {
    /// The case of `word` holding `inner`; an `inner` that is itself a case stands as its word alone.
    #[must_use]
    pub fn case(word: Word, inner: Fact) -> Self {
        let inner = match inner {
            Self::Case(Case { word, .. }) => Self::Word(word),
            other => other,
        };
        Self::Case(Case { word, inner: Box::new(inner) })
    }
}

impl fmt::Display for Case {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}({})", self.word, self.inner)
    }
}

impl fmt::Display for Fact {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count(count) => write!(out, "{count}"),
            Self::Signed(number) => write!(out, "{number}"),
            Self::Flag(flag) => write!(out, "{flag}"),
            Self::Word(word) => word.fmt(out),
            Self::Product(product) => product.fmt(out),
            Self::Stand(stand) => stand.fmt(out),
            Self::Withheld => out.write_str("withheld"),
            Self::Absent => out.write_str("none"),
            Self::Case(case) => fmt::Display::fmt(case, out),
        }
    }
}

/// A value a log line or a report may hold. It is implemented for numbers, flags, [`Product`], [`Stand`], and by
/// `#[derive(Public)]` for an enum (by its variants' names, and the fact of a field marked `#[public]`); never for a string, a path, an address or an id, so
/// writing one is a build error.
pub trait Public {
    /// The fact this value stands for.
    fn fact(&self) -> Fact;
}

impl Public for Fact {
    fn fact(&self) -> Fact {
        self.clone()
    }
}

impl Public for bool {
    fn fact(&self) -> Fact {
        Fact::Flag(*self)
    }
}

macro_rules! count {
    ($($number:ty),*) => {$(
        impl Public for $number {
            fn fact(&self) -> Fact {
                Fact::Count(u64::from(*self))
            }
        }
    )*};
}
count!(u8, u16, u32, u64);

macro_rules! signed {
    ($($number:ty),*) => {$(
        impl Public for $number {
            fn fact(&self) -> Fact {
                Fact::Signed(i64::from(*self))
            }
        }
    )*};
}
signed!(i8, i16, i32, i64);

impl Public for isize {
    fn fact(&self) -> Fact {
        Fact::Signed(i64::try_from(*self).unwrap_or(if *self < 0 { i64::MIN } else { i64::MAX }))
    }
}

/// A duration as whole milliseconds, for a field named `..._ms`; one that does not fit is the largest count.
#[must_use]
pub fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A count as a line says it in at most five digits, the largest when it does not fit, so a line of such fields keeps
/// within [`crate::MAX_LINE`] whatever the counts.
#[must_use]
pub fn count16(count: u64) -> u16 {
    u16::try_from(count).unwrap_or(u16::MAX)
}

/// A count as a line says it in at most ten digits, the largest when it does not fit.
#[must_use]
pub fn count32(count: u64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

impl Public for usize {
    fn fact(&self) -> Fact {
        Fact::Count(u64::try_from(*self).unwrap_or(u64::MAX))
    }
}

impl Public for Product {
    fn fact(&self) -> Fact {
        Fact::Product(self.clone())
    }
}

impl Public for Stand {
    fn fact(&self) -> Fact {
        Fact::Stand(self.clone())
    }
}

impl<T: Public> Public for Option<T> {
    fn fact(&self) -> Fact {
        self.as_ref().map_or(Fact::Absent, Public::fact)
    }
}

impl<T: Public + ?Sized> Public for &T {
    fn fact(&self) -> Fact {
        (**self).fact()
    }
}
