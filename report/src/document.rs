//! The debug report: sections of public facts and the recent log, as one plain-text file.

use std::fmt::Write as _;

use crate::fact::MAX_WORD;
use crate::listing::{ListedPart, Listing};
use crate::product::MAX_PRODUCT;
use crate::ring::LOG_BYTES;
use crate::{Fact, LogLine, Previous, Public, Recent, time};

/// The most bytes a report's text takes; the caps below make it hold by construction.
pub const MAX_REPORT_BYTES: usize = 8 * 1024 * 1024;
/// The most sections a report holds.
pub const MAX_SECTIONS: usize = 12;
/// The most rows a section holds.
pub const MAX_ROWS: usize = 64;
/// The longest section title, in bytes.
pub const MAX_TITLE: usize = 48;
/// The longest row label, in bytes.
pub const MAX_LABEL: usize = 48;

/// The longest word of a [`Build`], in bytes; a report with a longer one takes no section.
pub const MAX_BUILD_WORD: usize = 64;

/// The first words of a report's first line, which the side's word follows.
pub(crate) const HEADLINE: &str = "Chatkord debug report: ";
/// The heading of this run's log, which a server's report that cannot hold one marks absent under the same words.
pub const RECENT_LOG: &str = "Recent log, newest last";
/// The heading of the previous run's log.
pub(crate) const PREVIOUS_RUN: &str = "Previous run, newest last";
/// What a row, a log's count and a log's line start with.
pub(crate) const INDENT: &str = "  ";
/// The row of a section this report cannot fill.
pub(crate) const NOT_AVAILABLE: &str = "not available in this report";
/// The label of the log's count of notes too long to keep.
pub(crate) const DROPPED_LABEL: &str = "notes too long to keep";
/// The label of the previous run's count of refused lines.
pub(crate) const REFUSED_LABEL: &str = "lines refused";
/// The most digits of a count: `u64::MAX` has 20.
pub(crate) const COUNT_DIGITS: usize = 20;

/// The longest value a row holds: a case, its word, brackets and a product name inside (a case holds no case).
const MAX_VALUE: usize = MAX_WORD + 2 + MAX_PRODUCT;
/// A row at its longest: two spaces, the label, `: `, the longest value and a newline.
pub(crate) const MAX_ROW: usize = 2 + MAX_LABEL + 2 + MAX_VALUE + 1;
/// A log's count row at its longest.
const MAX_COUNT_ROW: usize = 2 + DROPPED_LABEL.len() + 2 + COUNT_DIGITS + 1;
/// The two heading lines at their longest: the headline and side, the date, and three build words.
pub(crate) const MAX_HEAD: usize = HEADLINE.len()
    + "server\n".len()
    + "Created ".len()
    + COUNT_DIGITS
    + "-MM-DD HH:MM UTC".len()
    + " \u{b7} ".len()
    + 3 * MAX_BUILD_WORD
    + " ()\n".len()
    + 1;

const _: () = assert!(
    MAX_HEAD
        + MAX_SECTIONS * (1 + MAX_TITLE + 1 + MAX_ROWS * MAX_ROW)
        + 2 * (1 + MAX_TITLE + 1 + MAX_COUNT_ROW + LOG_BYTES)
        <= MAX_REPORT_BYTES,
    "the caps must keep a report under its bound"
);
const _: () = assert!(REFUSED_LABEL.len() <= DROPPED_LABEL.len(), "the count row bound covers both counts");

/// Which end made a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The client's report.
    Client,
    /// The server's report.
    Server,
}

impl Side {
    /// The side a headline's last word names.
    pub(crate) fn of_word(word: &str) -> Option<Self> {
        [Self::Client, Self::Server].into_iter().find(|side| side.word() == word)
    }

    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
        }
    }
}

/// The file name of a report made at `unix_seconds`.
#[must_use]
pub fn file_name(side: Side, unix_seconds: u64) -> String {
    format!("chatkord-{}-report-{unix_seconds}.txt", side.word())
}

/// Which program made a report, as its build states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Build {
    /// The program's name, such as `Chatkord`.
    pub program: &'static str,
    /// Its version.
    pub version: &'static str,
    /// Where it was built from, such as a tag.
    pub source: &'static str,
}

/// A report's section, a row or a list was added past its limit or its label was too long.
#[derive(Clone, Copy, Debug, PartialEq, Eq, crate::Public)]
pub enum Full {
    /// The report holds [`MAX_SECTIONS`] sections.
    Sections,
    /// The section holds [`MAX_ROWS`] rows, or is absent and holds none.
    Rows,
    /// A title or label is empty or over its bound, or a word of the report's [`Build`] is over [`MAX_BUILD_WORD`].
    Name,
}

/// A titled run of rows, or the title alone when this report cannot fill it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    title: &'static str,
    body: Body,
}

/// What a section holds: rows, or nothing because this report cannot fill it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Body {
    Rows(Vec<(&'static str, Fact)>),
    Absent,
}

impl Section {
    /// An empty section.
    #[must_use]
    pub fn new(title: &'static str) -> Self {
        Self { title, body: Body::Rows(Vec::new()) }
    }

    /// A section this report cannot fill, such as a running server's figures in a report made from the command line.
    /// It holds no rows and takes none.
    #[must_use]
    pub fn absent(title: &'static str) -> Self {
        Self { title, body: Body::Absent }
    }

    /// How many rows the section holds.
    #[must_use]
    pub fn row_count(&self) -> usize {
        match &self.body {
            Body::Rows(rows) => rows.len(),
            Body::Absent => 0,
        }
    }

    /// Adds `label: value`, refused when the section is full or absent, or `label` is empty or over [`MAX_LABEL`]
    /// bytes.
    ///
    /// # Errors
    ///
    /// [`Full::Rows`] or [`Full::Name`].
    pub fn push(&mut self, label: &'static str, value: &(impl Public + ?Sized)) -> Result<(), Full> {
        if label.is_empty() || label.len() > MAX_LABEL {
            return Err(Full::Name);
        }
        match &mut self.body {
            Body::Rows(rows) if rows.len() < MAX_ROWS => rows.push((label, value.fact())),
            Body::Rows(_) | Body::Absent => return Err(Full::Rows),
        }
        Ok(())
    }

    /// The section as its lines are written: the title, then each row, or the one row saying it is absent.
    pub(crate) fn write(&self, out: &mut String) {
        let _ = writeln!(out, "{}", self.title);
        match &self.body {
            Body::Rows(rows) => {
                for (label, fact) in rows {
                    let _ = writeln!(out, "{INDENT}{label}: {fact}");
                }
            }
            Body::Absent => {
                let _ = writeln!(out, "{INDENT}{NOT_AVAILABLE}");
            }
        }
    }
}

impl Build {
    fn fits(&self) -> bool {
        [self.program, self.version, self.source].iter().all(|word| word.len() <= MAX_BUILD_WORD)
    }
}

/// A debug report: what the header, the sections and the recent log hold, and nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    side: Side,
    build: Build,
    created: u64,
    sections: Vec<Section>,
    log: Option<Recent>,
    previous: Option<Previous>,
}

impl Report {
    /// A report made at `created`, in Unix seconds, from the program's one clock.
    #[must_use]
    pub fn new(side: Side, build: Build, created: u64) -> Self {
        Self { side, build, created, sections: Vec::new(), log: None, previous: None }
    }

    /// Sets the previous run's log, as [`Previous::read`] gives it; a report without one has no such part.
    pub fn previous_run(&mut self, previous: Previous) {
        self.previous = Some(previous);
    }

    /// Adds a section, refused past [`MAX_SECTIONS`], when its title is empty or over [`MAX_TITLE`] bytes, or when a
    /// word of the report's [`Build`] is over [`MAX_BUILD_WORD`], which the bound of the text counts on.
    ///
    /// # Errors
    ///
    /// [`Full::Sections`] or [`Full::Name`].
    pub fn section(&mut self, section: Section) -> Result<(), Full> {
        if section.title.is_empty() || section.title.len() > MAX_TITLE || !self.build.fits() {
            return Err(Full::Name);
        }
        if self.sections.len() >= MAX_SECTIONS {
            return Err(Full::Sections);
        }
        self.sections.push(section);
        Ok(())
    }

    /// Sets the recent log, as [`Ring::recent`](crate::Ring::recent) gives it: only a ring makes one, and it holds
    /// no more than the report's bound allows.
    pub fn log(&mut self, recent: Recent) {
        self.log = Some(recent);
    }

    /// The file's name.
    #[must_use]
    pub fn file_name(&self) -> String {
        file_name(self.side, self.created)
    }

    /// The report as plain text: the same report always reads the same, and never exceeds [`MAX_REPORT_BYTES`].
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.write_head(&mut out);
        for section in &self.sections {
            out.push('\n');
            section.write(&mut out);
        }
        for (heading, label, count, lines) in self.logs() {
            out.push('\n');
            write_log_head(&mut out, heading, label, count);
            for line in lines {
                let _ = writeln!(out, "{INDENT}{line}");
            }
        }
        out
    }

    /// What the report's dialog lists of it: every section in full, and of each log its heading, its count and the
    /// `newest` lines, made of the parts without writing the text.
    #[must_use]
    pub fn listing(&self, newest: usize) -> Listing {
        let mut head = String::new();
        self.write_head(&mut head);
        let mut parts: Vec<ListedPart> = self
            .sections
            .iter()
            .map(|section| {
                let mut text = String::new();
                section.write(&mut text);
                ListedPart::whole(text.lines())
            })
            .collect();
        for (heading, label, count, lines) in self.logs() {
            let mut text = String::new();
            write_log_head(&mut text, heading, label, count);
            let cut = lines.len().saturating_sub(newest);
            let last = lines[cut..].iter().map(|line| format!("{INDENT}{line}")).collect();
            parts.push(ListedPart::with_tail(text.lines(), lines.len(), last));
        }
        Listing::of(head.lines(), parts)
    }

    fn write_head(&self, out: &mut String) {
        let _ = writeln!(out, "{HEADLINE}{}", self.side.word());
        let _ = writeln!(
            out,
            "Created {} \u{b7} {} {} ({})",
            time::stamp(self.created),
            self.build.program,
            self.build.version,
            self.build.source
        );
    }

    /// The logs the report holds, in the order they are written: heading, count label, count and lines.
    fn logs(&self) -> impl Iterator<Item = (&'static str, &'static str, u64, &[LogLine])> {
        let recent = self.log.as_ref().map(|recent| (RECENT_LOG, DROPPED_LABEL, recent.dropped(), recent.lines()));
        let previous = self.previous.as_ref().map(|previous| {
            (PREVIOUS_RUN, REFUSED_LABEL, u64::try_from(previous.refused()).unwrap_or(u64::MAX), previous.lines())
        });
        recent.into_iter().chain(previous)
    }
}

/// A log's heading and its count row.
pub(crate) fn write_log_head(out: &mut String, heading: &str, label: &str, count: u64) {
    let _ = writeln!(out, "{heading}\n{INDENT}{label}: {count}");
}
