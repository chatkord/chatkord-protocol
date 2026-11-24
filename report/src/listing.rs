//! What a report's dialog lists of it: the headline, every section in full, and of each log its heading, its count and
//! its newest lines. A report makes one from its parts; a report's text, whose parts are all that arrives from another
//! program, is read once into one, refused where it is not exactly the shape [`Report::text`](crate::Report::text) writes.

use std::collections::VecDeque;
use std::fmt;

use crate::document::{
    COUNT_DIGITS, DROPPED_LABEL, HEADLINE, INDENT, MAX_HEAD, MAX_REPORT_BYTES, MAX_ROW, MAX_ROWS, MAX_SECTIONS,
    MAX_TITLE, NOT_AVAILABLE, PREVIOUS_RUN, RECENT_LOG, REFUSED_LABEL, Side,
};
use crate::ring::MAX_LINE;

/// The most parts a report lists: its sections and its two logs.
const MAX_PARTS: usize = MAX_SECTIONS + 2;

/// A log as the dialog lists it: how many lines it holds, and the newest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogTail {
    total: usize,
    newest: Vec<String>,
}

impl LogTail {
    /// How many lines the log holds.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    /// The newest lines, oldest first, as the file writes them.
    #[must_use]
    pub fn newest(&self) -> &[String] {
        &self.newest
    }
}

/// A section's lines listed in full, or a log's heading and count with the tail of its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedPart {
    rows: Vec<String>,
    log: Option<LogTail>,
}

impl ListedPart {
    pub(crate) fn whole<'a>(lines: impl IntoIterator<Item = &'a str>) -> Self {
        Self { rows: lines.into_iter().map(str::to_owned).collect(), log: None }
    }

    pub(crate) fn with_tail<'a>(head: impl IntoIterator<Item = &'a str>, total: usize, newest: Vec<String>) -> Self {
        Self { log: Some(LogTail { total, newest }), ..Self::whole(head) }
    }

    /// The lines listed in full: a section's title and rows, or a log's heading and count.
    #[must_use]
    pub fn rows(&self) -> &[String] {
        &self.rows
    }

    /// The log's tail, none for a section.
    #[must_use]
    pub fn log(&self) -> Option<&LogTail> {
        self.log.as_ref()
    }
}

/// Why a text is not a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotAReport {
    /// The text is over [`MAX_REPORT_BYTES`].
    TooLong,
    /// The text is not in the shape a report is written in.
    Shape,
}

impl fmt::Display for NotAReport {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::TooLong => "the text is longer than a report can be",
            Self::Shape => "the text is not shaped as a report is",
        })
    }
}

/// A report as the dialog lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    head: Vec<String>,
    parts: Vec<ListedPart>,
}

impl Listing {
    pub(crate) fn of<'a>(head: impl IntoIterator<Item = &'a str>, parts: Vec<ListedPart>) -> Self {
        Self { head: head.into_iter().map(str::to_owned).collect(), parts }
    }

    /// The headline and the line that dates it.
    #[must_use]
    pub fn head(&self) -> &[String] {
        &self.head
    }

    /// The sections and logs, in the order the text writes them; a blank line parts each from the one before.
    #[must_use]
    pub fn parts(&self) -> &[ListedPart] {
        &self.parts
    }

    /// The listing of `text`, which is exactly what [`Report::text`](crate::Report::text) writes, with the `newest`
    /// lines of each log. Nothing else is taken, and the lines of a log beyond the newest are counted, never kept.
    ///
    /// # Errors
    ///
    /// [`NotAReport`] for a text over [`MAX_REPORT_BYTES`] or in any other shape.
    pub fn read(text: &str, newest: usize) -> Result<Self, NotAReport> {
        if text.len() > MAX_REPORT_BYTES {
            return Err(NotAReport::TooLong);
        }
        let body = text.strip_suffix('\n').ok_or(NotAReport::Shape)?;
        let mut blocks = body.split("\n\n");
        let head = headline(blocks.next().ok_or(NotAReport::Shape)?)?;
        let mut parts = Vec::new();
        let mut logs_read = [false; 2];
        for block in blocks {
            if parts.len() >= MAX_PARTS {
                return Err(NotAReport::Shape);
            }
            parts.push(part(block, newest, &mut logs_read)?);
        }
        Ok(Self { head, parts })
    }
}

/// The two heading lines: the headline naming a side, then the line that dates the report.
fn headline(block: &str) -> Result<Vec<String>, NotAReport> {
    let mut lines = block.split('\n');
    let (first, second) = (lines.next(), lines.next());
    match (first.and_then(|first| first.strip_prefix(HEADLINE)), second, lines.next()) {
        (Some(side), Some(second), None)
            if Side::of_word(side).is_some() && is_plain(second) && second.len() < MAX_HEAD =>
        {
            Ok(vec![format!("{HEADLINE}{side}"), second.to_owned()])
        }
        _ => Err(NotAReport::Shape),
    }
}

/// A line that is not indented and holds something: a title or the dating line.
fn is_plain(line: &str) -> bool {
    !line.is_empty() && !line.starts_with(' ')
}

/// The log a heading names, as its slot and its count's label, or none for any other title.
fn log_kind(title: &str) -> Option<(usize, &'static str)> {
    match title {
        RECENT_LOG => Some((0, DROPPED_LABEL)),
        PREVIOUS_RUN => Some((1, REFUSED_LABEL)),
        _ => None,
    }
}

/// Whether `row` is a log's count row under `label`: the indent, the label, `: ` and a number.
fn is_count(row: &str, label: &str) -> bool {
    row.strip_prefix(INDENT)
        .and_then(|row| row.strip_prefix(label))
        .and_then(|row| row.strip_prefix(": "))
        .is_some_and(is_number)
}

/// Whether `count` is a count as it is written: a `u64` in its plain decimal spelling.
fn is_number(count: &str) -> bool {
    count.len() <= COUNT_DIGITS && count.parse::<u64>().is_ok_and(|number| number.to_string() == count)
}

/// Whether `row` is a section's row as one is written: indented, and no longer than a row's bound.
fn is_row(row: &str) -> bool {
    row.starts_with(INDENT) && row.len() < MAX_ROW
}

/// Whether `line` is a log's line as one is written: indented, and no longer than a kept line.
fn is_log_line(line: &str) -> bool {
    line.starts_with(INDENT) && line.len() <= INDENT.len() + MAX_LINE
}

/// One block of lines between blank lines: a section, or a log when its heading and its count row say so.
fn part(block: &str, newest: usize, logs_read: &mut [bool; 2]) -> Result<ListedPart, NotAReport> {
    let mut lines = block.split('\n');
    let title = lines.next().filter(|title| is_plain(title) && title.len() <= MAX_TITLE).ok_or(NotAReport::Shape)?;
    let first = lines.next();
    let log = match log_kind(title) {
        Some((slot, label)) if first.is_some_and(|row| is_count(row, label)) => Some(slot),
        // A log's heading over anything but its count is only a report's mark that it holds none.
        Some(_)
            if first.and_then(|row| row.strip_prefix(INDENT)) != Some(NOT_AVAILABLE)
                || lines.clone().next().is_some() =>
        {
            return Err(NotAReport::Shape);
        }
        _ => None,
    };
    let Some(slot) = log else {
        let rows: Vec<&str> = first.into_iter().chain(lines).take(MAX_ROWS + 1).collect();
        if rows.len() > MAX_ROWS || !rows.iter().all(|row| is_row(row)) {
            return Err(NotAReport::Shape);
        }
        return Ok(ListedPart::whole(std::iter::once(title).chain(rows)));
    };
    if std::mem::replace(&mut logs_read[slot], true) {
        return Err(NotAReport::Shape);
    }
    let (mut total, mut tail): (usize, VecDeque<&str>) = (0, VecDeque::new());
    for line in lines {
        if !is_log_line(line) {
            return Err(NotAReport::Shape);
        }
        total += 1;
        if tail.len() == newest {
            tail.pop_front();
        }
        if newest > 0 {
            tail.push_back(line);
        }
    }
    let head = std::iter::once(title).chain(first);
    Ok(ListedPart::with_tail(head, total, tail.into_iter().map(str::to_owned).collect()))
}
