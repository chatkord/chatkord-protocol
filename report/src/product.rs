//! A product name from outside, checked where it enters.

use std::fmt;

use crate::Fact;

/// The most bytes a product name takes.
pub(crate) const MAX_PRODUCT: usize = 96;

/// A GPU, driver or system name as a person's machine states it: only letters, digits and ` ._()+-`, at most 96
/// bytes, and not starting with `.`, so neither a path nor a hidden file is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product(Box<str>);

impl Product {
    /// The name, or `None` when it is empty, too long or holds any other character; a report says "withheld" then.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let fits = !text.is_empty()
            && text.len() <= MAX_PRODUCT
            && !text.starts_with('.')
            && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || b" ._()+-".contains(&byte));
        fits.then(|| Self(text.into()))
    }

    /// The name as a fact, or [`Fact::Withheld`] when [`Product::parse`] refuses it.
    #[must_use]
    pub fn fact_of(text: &str) -> Fact {
        Self::parse(text).map_or(Fact::Withheld, Fact::Product)
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Product {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_product_name_is_kept_and_anything_else_is_refused() {
        for name in ["AMD Radeon RX 9060 XT", "Mesa 26.0.1 (radv)", "NVIDIA GeForce RTX 4060 PCIe SSE2", "x264 r3222+1"]
        {
            assert_eq!(Product::parse(name).as_ref().map(Product::as_str), Some(name));
        }
        for refused in [
            "",
            "/tmp/ada/.config",
            ".hidden",
            "a//b",
            "a/b",
            "NVIDIA GeForce RTX 4060/PCIe/SSE2",
            "ada@example.invalid",
            r"C:scratchdir",
            "name\nline",
            "caf\u{e9}",
            "tab\there",
            "a:b",
        ] {
            assert_eq!(Product::parse(refused), None, "{refused:?}");
        }
        assert!(Product::parse(&"a".repeat(MAX_PRODUCT)).is_some());
        assert_eq!(Product::parse(&"a".repeat(MAX_PRODUCT + 1)), None);
    }
}
