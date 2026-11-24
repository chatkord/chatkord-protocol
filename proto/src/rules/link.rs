//! A link as both ends read it: the one parse every consumer of a link's text shares, so the server that accepts a
//! span, a client that draws it and the page it may fetch cannot disagree about where it goes.

use std::ops::Range;

/// The schemes a link may have, as spelled in lower case.
const SCHEMES: [(&str, Scheme); 2] = [("https://", Scheme::Https), ("http://", Scheme::Http)];

/// What a link's scheme says it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// `https://`, a page over TLS.
    Https,
    /// `http://`, a page in the clear.
    Http,
}

/// A link's text read into its parts, borrowing it: a scheme of [`Scheme`] in either case, a host that is not empty
/// (a name, an address, or a bracketed IPv6 address), an optional port, and the rest. Made only by [`LinkTarget::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkTarget<'a> {
    scheme: Scheme,
    userinfo: bool,
    host: &'a str,
    port: Option<u16>,
    rest: &'a str,
}

impl<'a> LinkTarget<'a> {
    /// The parts of `text`, or none when it is not a link: another scheme, an empty host, a port that is not a number
    /// of a `u16`, or white space or a control character anywhere.
    pub fn parse(text: &'a str) -> Option<Self> {
        if text.chars().any(|character| character.is_whitespace() || character.is_control()) {
            return None;
        }
        let (scheme, after) = SCHEMES.iter().find_map(|(word, scheme)| {
            text.get(..word.len())
                .filter(|head| head.eq_ignore_ascii_case(word))
                .and_then(|_| text.get(word.len()..))
                .map(|after| (*scheme, after))
        })?;
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let (authority, rest) = after.split_at(end);
        let (userinfo, hostport) = match authority.rsplit_once('@') {
            Some((_, hostport)) => (true, hostport),
            None => (false, authority),
        };
        let (host, port) = match hostport.strip_prefix('[') {
            Some(inside) => {
                let (address, tail) = inside.split_once(']')?;
                let port = if tail.is_empty() { None } else { Some(tail.strip_prefix(':')?.parse().ok()?) };
                (hostport.get(..address.len() + 2).filter(|_| !address.is_empty())?, port)
            }
            None => match hostport.split_once(':') {
                Some((host, port)) => (host, Some(port.parse().ok()?)),
                None => (hostport, None),
            },
        };
        (!host.is_empty()).then_some(Self { scheme, userinfo, host, port, rest })
    }

    /// Its scheme.
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// Whether it is a page over TLS.
    pub fn is_https(&self) -> bool {
        self.scheme == Scheme::Https
    }

    /// Whether the text names credentials before the host (a user and an `@`), which can make a link read as one place and
    /// go to another.
    pub fn has_userinfo(&self) -> bool {
        self.userinfo
    }

    /// The host as written, an IPv6 address in its brackets, never empty.
    pub fn host(&self) -> &'a str {
        self.host
    }

    /// The port, when one is written.
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// What follows the host and port: a path, a query or a fragment, or nothing.
    pub fn rest(&self) -> &'a str {
        self.rest
    }
}

/// The byte ranges of the plain links in `text`, in order and apart, as Discord finds them at send: `http://` or
/// `https://` at a word's start, running to white space, without the punctuation that ends a
/// sentence or a bracket the link did not open. Each is one [`LinkTarget::parse`] takes.
pub fn find_links(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(rest) = text.get(at..) {
        let Some(offset) = rest.find("://") else { break };
        let colon = at + offset;
        match link_around(text, colon) {
            Some(link) => {
                at = link.end;
                found.push(link);
            }
            None => at = colon + 3,
        }
    }
    found
}

/// The link whose `://` is at `colon`, if a scheme we open starts a word just before it.
fn link_around(text: &str, colon: usize) -> Option<Range<usize>> {
    let start = SCHEMES.iter().find_map(|(scheme, _)| {
        let name = scheme.len() - 3;
        let start = colon.checked_sub(name)?;
        let written = text.get(start..colon + 3)?;
        written.eq_ignore_ascii_case(scheme).then_some(start)
    })?;
    let before = text.get(..start).and_then(|head| head.chars().next_back());
    if before.is_some_and(char::is_alphanumeric) {
        return None;
    }
    let tail = text.get(colon + 3..)?;
    let run = tail.find(|c: char| c.is_whitespace() || c == '<').unwrap_or(tail.len());
    let mut link = tail.get(..run)?;
    while let Some(last) = link.chars().next_back() {
        let unopened = match last {
            ')' => link.matches(')').count() > link.matches('(').count(),
            ']' => link.matches(']').count() > link.matches('[').count(),
            '.' | ',' | ':' | ';' | '"' | '\'' | '!' | '?' | '*' | '~' | '|' | '>' => true,
            '_' => before == Some('_'),
            _ => false,
        };
        if !unopened {
            break;
        }
        link = link.get(..link.len() - last.len_utf8())?;
    }
    let host_ends = link.find(['/', '?', '#']).unwrap_or(link.len());
    let host = link.get(..host_ends)?;
    let named = host.chars().any(char::is_alphanumeric)
        && host.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
    let end = colon + 3 + link.len();
    // The parse the server accepts a span by, so a link found here is always one it takes.
    (named && LinkTarget::parse(text.get(start..end)?).is_some()).then_some(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(text: &str) -> Vec<&str> {
        find_links(text).into_iter().map(|range| text.get(range).expect("a range of the text")).collect()
    }

    #[test]
    fn find_links_matches_the_client_s() {
        assert_eq!(links("see https://example.invalid/a?b=c#d, then"), ["https://example.invalid/a?b=c#d"]);
        assert_eq!(links("(http://example.invalid/wiki/A_(b)) and."), ["http://example.invalid/wiki/A_(b)"]);
        assert_eq!(links("**https://example.invalid**"), ["https://example.invalid"]);
        assert_eq!(
            links("a HTTPS://example.invalid b http://x.invalid"),
            ["HTTPS://example.invalid", "http://x.invalid"]
        );
    }

    #[test]
    fn only_our_schemes_at_a_word_start_with_a_host_are_links() {
        assert!(links("ftp://example.invalid file:///etc/passwd").is_empty());
        assert!(links("xhttps://example.invalid").is_empty());
        assert!(links("https:// nothing").is_empty());
        assert!(links("https://...").is_empty());
        assert!(links("javascript://example.invalid").is_empty());
    }

    #[test]
    fn a_link_reads_into_scheme_host_port_and_rest() {
        let link = LinkTarget::parse("HTTPS://Example.invalid:8443/a/b?c=d#e").expect("a link");
        assert_eq!(
            (link.scheme(), link.host(), link.port(), link.rest()),
            (Scheme::Https, "Example.invalid", Some(8443), "/a/b?c=d#e")
        );
        assert!(link.is_https() && !link.has_userinfo());
        let bare = LinkTarget::parse("http://example.invalid").expect("a link");
        assert_eq!((bare.scheme(), bare.host(), bare.port(), bare.rest()), (Scheme::Http, "example.invalid", None, ""));
        let query = LinkTarget::parse("https://example.invalid?x=1").expect("a link");
        assert_eq!((query.host(), query.rest()), ("example.invalid", "?x=1"));
        let v6 = LinkTarget::parse("https://[2001:db8::1]:444/x").expect("a link");
        assert_eq!((v6.host(), v6.port(), v6.rest()), ("[2001:db8::1]", Some(444), "/x"));
        assert_eq!(LinkTarget::parse("https://[::1]/").map(|link| (link.host(), link.port())), Some(("[::1]", None)));
    }

    #[test]
    fn credentials_before_the_host_are_named_and_the_host_is_the_one_after_them() {
        let link = LinkTarget::parse("https://example.invalid@evil.invalid/x").expect("a link");
        assert!(link.has_userinfo());
        assert_eq!(link.host(), "evil.invalid");
    }

    #[test]
    fn what_is_not_a_link_is_none() {
        for bad in [
            "click here",
            "ftp://example.invalid",
            "https://",
            "https:///path",
            "javascript:alert(1)",
            "https://a b",
            "https://example.invalid/\u{7}",
            "https://example.invalid:99999/",
            "https://example.invalid:/",
            "https://example.invalid:x/",
            "https://[::1",
            "https://[]/",
            "https://[::1]x/",
            "https://@/",
            "",
            "https:/example.invalid",
        ] {
            assert_eq!(LinkTarget::parse(bad), None, "{bad:?}");
        }
    }
}
