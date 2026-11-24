//! A previous run's log file read back for a debug report: a line is taken only when it has exactly the shape the
//! client's file layer writes for a note and every token is within the character set a note's tokens use, and is refused
//! whole otherwise. The guarantee is that shape and set, not that a note wrote the line: a file edited by hand can still
//! add plain words (a name, an id) in a message or a field, but never a path, an address, a URL, a mail address or text
//! with punctuation.

use std::fmt::Write as _;

use crate::ring::{KEPT, LOG_BYTES, MAX_LINE, weight};
use crate::{LogLine, Product};

/// The most bytes of a message read back; the notes' literals are far shorter.
const MAX_MESSAGE: usize = 160;
/// The longest module path, in bytes.
const MAX_TARGET: usize = 64;
/// The longest field name, in bytes.
const MAX_KEY: usize = 32;
/// The most digits of a count or a signed number: `u64::MAX` has 20.
const MAX_DIGITS: usize = 20;
/// What a note's fields start with: the mark only a note has, written by the file layer as a field.
const MARK: &str = " report.note=true";
/// The separators of a timestamp the file layer writes, `2026-11-17T10:42:02.123456Z`, by position.
const STAMP_MARKS: [(usize, u8); 7] =
    [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':'), (19, b'.'), (26, b'Z')];
/// The length of such a timestamp.
const STAMP: usize = 27;

/// The most bytes one line of a log file takes and is still taken as a note: the stamp, level, module path and mark the
/// file layer adds around a note of at most [`MAX_LINE`] bytes, and the newline.
pub const MAX_FILE_LINE: usize = STAMP + 1 + 5 + 1 + MAX_TARGET + 2 + MARK.len() + MAX_LINE + 1;

/// The most bytes of a log file [`Previous::read`] can use: the last [`KEPT`] lines and the one a read cut in two.
pub const READ_BACK: usize = (KEPT + 1) * MAX_FILE_LINE;

/// The lines of a previous run's log that read back as notes, and how many did not. A line is in a note's shape and
/// character set; it was not necessarily written by a note (see the module's guarantee).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Previous {
    lines: Vec<LogLine>,
    refused: usize,
}

impl Previous {
    /// What the last [`KEPT`] lines of `text` read back as, as many as fit the report's log bound: the lines that are
    /// notes, oldest first, and the count of those that are not (a line from another program, a changed one, an empty
    /// one, or a last line without its newline, which is half written).
    #[must_use]
    pub fn read(text: &str) -> Self {
        let all: Vec<&str> = text.split_inclusive('\n').collect();
        let last = all.len().saturating_sub(KEPT);
        let mut previous = Self { lines: Vec::new(), refused: 0 };
        let mut bytes = 0;
        for segment in all.iter().skip(last).rev() {
            match segment.strip_suffix('\n').and_then(note) {
                Some(read) => {
                    bytes += weight(&read);
                    if bytes > LOG_BYTES {
                        break;
                    }
                    previous.lines.push(read);
                }
                None => previous.refused += 1,
            }
        }
        previous.lines.reverse();
        previous
    }

    /// The notes, oldest first.
    #[must_use]
    pub fn lines(&self) -> &[LogLine] {
        &self.lines
    }

    /// How many lines were refused.
    #[must_use]
    pub fn refused(&self) -> usize {
        self.refused
    }
}

/// The time of day of a timestamp in exactly the shape the file layer writes, or `None` for any other text.
fn clock(stamp: &str) -> Option<&str> {
    let shaped = stamp.len() == STAMP
        && stamp.bytes().enumerate().all(|(at, byte)| match STAMP_MARKS.iter().find(|(place, _)| *place == at) {
            Some((_, mark)) => byte == *mark,
            None => byte.is_ascii_digit(),
        });
    let number = |from: usize, to: usize| stamp.get(from..to)?.parse::<u8>().ok();
    let valid = shaped
        && (1..=12).contains(&number(5, 7)?)
        && (1..=31).contains(&number(8, 10)?)
        && number(11, 13)? < 24
        && number(14, 16)? < 60
        && number(17, 19)? < 60;
    if valid { stamp.get(11..19) } else { None }
}

/// The level word the ring writes for the level the file layer pads to five bytes; only notes of INFO and above are
/// kept, so no other level is a note.
fn level(padded: &str) -> Option<&'static str> {
    match padded {
        " INFO" => Some("info "),
        " WARN" => Some("warn "),
        "ERROR" => Some("error"),
        _ => None,
    }
}

/// The crate of a module path of lowercase words joined by `::`.
fn krate(path: &str) -> Option<&str> {
    let word_ok = |word: &str| {
        !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    if path.len() <= MAX_TARGET && path.split("::").all(word_ok) { path.split("::").next() } else { None }
}

/// A note's literal: words of letters and digits with the few marks the notes' own sentences use, never a path, an
/// address or a computed value. A colon is a sentence's, so a space follows it: no IPv6 or hardware address passes.
fn message_ok(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.len() <= MAX_MESSAGE
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && !text.ends_with(' ')
        && !text.contains("  ")
        && bytes.iter().enumerate().all(|(at, byte)| *byte != b':' || bytes.get(at + 1) == Some(&b' '))
        && bytes.iter().all(|byte| byte.is_ascii_alphanumeric() || b" ,;:'-".contains(byte))
}

fn key_ok(key: &str) -> bool {
    key.len() <= MAX_KEY
        && key.bytes().next().is_some_and(|first| first.is_ascii_lowercase())
        && key.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// A fact as it displays: a count, a signed number, or a name that passes [`Product::parse`], has a letter (so a bare
/// dotted number, an address, is not one) and dots only inside numbers (so a host name or a file name is not one). A
/// flag is a name.
fn value_ok(value: &str) -> bool {
    let digits =
        |text: &str| !text.is_empty() && text.len() <= MAX_DIGITS && text.bytes().all(|byte| byte.is_ascii_digit());
    if digits(value) {
        return value.parse::<u64>().is_ok();
    }
    if value.strip_prefix('-').is_some_and(digits) {
        return value.parse::<i64>().is_ok();
    }
    !value.starts_with(' ')
        && Product::parse(value).is_some()
        && value.bytes().any(|byte| byte.is_ascii_alphabetic())
        && dots_in_numbers(value)
        && !has_address(value)
}

/// Whether `text` holds four digit groups joined by dots, an IPv4 address, whatever stands beside it.
fn has_address(text: &str) -> bool {
    text.split(|at: char| !(at.is_ascii_digit() || at == '.')).any(|run| {
        let mut groups = 0;
        run.split('.').any(|group| {
            groups = if group.is_empty() { 0 } else { groups + 1 };
            groups == 4
        })
    })
}

/// Whether every `.` of `value` stands between two digits, as in a version: a host name, a file name or an address
/// has dots between letters or at an end.
fn dots_in_numbers(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.iter().enumerate().all(|(at, &byte)| {
        byte != b'.'
            || (at.checked_sub(1).and_then(|before| bytes.get(before)).is_some_and(u8::is_ascii_digit)
                && bytes.get(at + 1).is_some_and(u8::is_ascii_digit))
    })
}

/// The fields after the mark as the ring writes them, `  name=value` each, or `None` for any that is not one. A value
/// may hold spaces (a product name), so a token starts a new field only when it is `name=...`.
fn fields(rest: &str) -> Option<String> {
    if rest.is_empty() {
        return Some(String::new());
    }
    let mut found: Vec<(&str, String)> = Vec::new();
    for token in rest.strip_prefix(' ')?.split(' ') {
        match token.split_once('=') {
            Some((key, value)) if key_ok(key) => found.push((key, value.to_owned())),
            _ if token.is_empty() => return None,
            _ => {
                let (_, text) = found.last_mut()?;
                text.push(' ');
                text.push_str(token);
            }
        }
    }
    let mut out = String::new();
    for (key, value) in &found {
        if !value_ok(value) {
            return None;
        }
        let _ = write!(out, "  {key}={value}");
    }
    Some(out)
}

/// The note one line of the file layer's output is, in the shape the ring keeps: `HH:MM:SS level crate message  name=value`.
fn note(line: &str) -> Option<LogLine> {
    let (stamp, rest) = line.split_once(' ')?;
    let time = clock(stamp)?;
    let word = level(rest.get(..5)?)?;
    let rest = rest.get(5..)?.strip_prefix(' ')?;
    let (module, rest) = rest.split_once(": ")?;
    let krate = krate(module)?;
    let (message, after) = rest.split_once(MARK)?;
    if !message_ok(message) {
        return None;
    }
    let shown = format!("{time} {word} {krate} {message}{}", fields(after)?);
    (shown.len() <= MAX_LINE).then(|| LogLine::of(shown))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "2026-11-17T10:42:02.123456Z  INFO shell::window: a share started report.note=true";

    #[test]
    fn a_line_the_file_layer_wrote_for_a_note_reads_back_as_the_ring_shows_it() {
        let text = format!(
            "{START} frames=30 hardware=true delta=-4 gpu=AMD Radeon RX 9060 XT path=none\n\
             2026-11-17T10:42:03.000001Z ERROR shell::main: the app stopped unexpectedly report.note=true file=window folder=src line=12 column=5\n\
             2026-11-17T10:42:04.000000Z  WARN call: the camera is gone report.note=true\n"
        );
        let read = Previous::read(&text);
        let shown: Vec<&str> = read.lines().iter().map(LogLine::as_str).collect();
        assert_eq!(
            shown,
            [
                "10:42:02 info  shell a share started  frames=30  hardware=true  delta=-4  gpu=AMD Radeon RX 9060 XT  path=none",
                "10:42:03 error shell the app stopped unexpectedly  file=window  folder=src  line=12  column=5",
                "10:42:04 warn  call the camera is gone",
            ]
        );
        assert_eq!(read.refused(), 0);
    }

    #[test]
    fn a_line_that_is_not_a_note_in_exactly_that_shape_is_refused_whole() {
        let refused = [
            format!("{START} file=/var/lib/example/a.rs"),
            format!("{START} file=C:scratch"),
            format!("{START} peer=203.0.113.7:8080"),
            format!("{START} peer=203.0.113.7"),
            format!("{START} peer=::1"),
            format!("{START} host=sentinel-host-0a1b.example.invalid"),
            format!("{START} host=host1.example2.invalid"),
            format!("{START} file=window.rs"),
            format!("{START} name=\"Ada\""),
            format!("{START} name=ada@example.invalid"),
            format!("{START} count=99999999999999999999999"),
            format!("{START} who={}", "a".repeat(97)),
            format!("{START} who= trailing"),
            format!("{START}  doubled=1"),
            format!("{START} word=1 =2"),
            format!("{START} peer=x203.0.113.7"),
            format!("{START} peer=router 198.51.100.10"),
            format!("{START} peer=gateway 192.0.2.1 up"),
            "2026-11-17T10:42:02.123456Z  INFO shell::window: opened /var/lib/example/chat.txt report.note=true"
                .to_owned(),
            "2026-11-17T10:42:02.123456Z  INFO shell::window: ada said hello to 203.0.113.7 report.note=true"
                .to_owned(),
            "2026-11-17T10:42:02.123456Z  INFO shell::window: reached 2001:db8:0:0:0:0:0:1 report.note=true".to_owned(),
            "2026-11-17T10:42:02.123456Z  INFO shell::window: card a4:83:e7:12:34:56 report.note=true".to_owned(),
            "2026-11-17T10:42:02.123456Z  INFO shell::window: ends with a colon: report.note=true".to_owned(),
            format!("{START} peer=2001:db8:0:0:0:0:0:1"),
            format!("{START} card=a4:83:e7:12:34:56"),
            "2026-11-17T10:42:02.123456Z  INFO shell::window: hello there".to_owned(),
            "2026-11-17T10:42:02.123456Z DEBUG shell::window: a thing report.note=true".to_owned(),
            "2026-11-17T25:42:02.123456Z  INFO shell::window: a thing report.note=true".to_owned(),
            "2026-11-17 10:42:02  INFO shell::window: a thing report.note=true".to_owned(),
            "2026-11-17T10:42:02.123456Z  INFO Shell/x: a thing report.note=true".to_owned(),
            format!("2026-11-17T10:42:02.123456Z  INFO shell: {} report.note=true", "word ".repeat(40).trim_end()),
            "the log is full; later lines go to stderr only".to_owned(),
            " ".to_owned(),
        ];
        for line in refused {
            let read = Previous::read(&format!("{line}\n"));
            assert!(read.lines().is_empty() && read.refused() == 1, "{line:?}: {read:?}");
        }
    }

    #[test]
    fn a_last_line_without_its_newline_is_half_written_and_refused() {
        let read = Previous::read(&format!("{START} number=1\n{START} number=2"));
        assert_eq!((read.lines().len(), read.refused()), (1, 1));
        assert!(read.lines()[0].as_str().ends_with("number=1"));
    }

    #[test]
    fn a_line_in_a_notes_shape_and_character_set_is_taken_whatever_wrote_it() {
        let read = Previous::read(&format!("{START} who=Ada Lovelace id=member1 version=Mesa 26.0.1\n"));
        assert_eq!(read.refused(), 0, "{read:?}");
    }

    #[test]
    fn only_the_last_kept_lines_are_read() {
        let text: String = (0..KEPT + 3).map(|at| format!("{START} number={at}\n")).collect();
        let read = Previous::read(&text);
        assert_eq!(read.lines().len(), KEPT);
        assert!(read.lines()[0].as_str().ends_with("number=3"));
    }
}
