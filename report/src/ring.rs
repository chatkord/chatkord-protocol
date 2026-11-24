//! The last notes of INFO and above, kept in memory for a debug report.

use std::collections::VecDeque;
use std::fmt::{self, Write as _};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, TryLockError};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::filter::{FilterFn, Filtered, filter_fn};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::{__MARK, NOTE, time};

/// How many notes the ring keeps: the newest.
pub const KEPT: usize = 20_000;

/// The most bytes of lines a report's log holds, each line counted with its indent and newline: the newest lines that
/// fit are kept, so two logs and the sections stay under [`MAX_REPORT_BYTES`](crate::MAX_REPORT_BYTES).
pub(crate) const LOG_BYTES: usize = 4_000_000;

/// What one line takes in a report's text: two spaces of indent, the line and a newline.
pub(crate) const fn weight(line: &LogLine) -> usize {
    line.0.len() + 3
}

/// The most bytes one kept line takes; a longer note is not kept, never cut, and is counted by [`Recent::dropped`].
pub const MAX_LINE: usize = 256;

/// One note as a report shows it: `HH:MM:SS level crate message  field=value`. Only the ring makes one, from a note's
/// literal and its facts, and [`Previous`](crate::Previous), from a log file's line that has exactly a note's shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine(String);

impl LogLine {
    pub(crate) fn of(text: String) -> Self {
        Self(text)
    }

    /// The line.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LogLine {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// Reads a note event's fields back as the text its facts displayed.
#[derive(Default)]
struct Rendered {
    message: String,
    fields: String,
}

impl Visit for Rendered {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // The macro passes each field with `%`, so the `Debug` here is the fact's `Display`.
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else if field.name() != __MARK {
            let _ = write!(self.fields, "  {}={value:?}", field.name());
        }
    }
}

/// The notes of a ring and the bytes they take in a report.
#[derive(Default)]
struct Kept {
    lines: VecDeque<LogLine>,
    bytes: usize,
}

struct Inner {
    clock: fn() -> SystemTime,
    kept: Mutex<Kept>,
    dropped: AtomicU64,
}

/// A ring's notes at one moment: what a report's log holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recent {
    lines: Vec<LogLine>,
    dropped: u64,
}

impl Recent {
    /// The kept notes, oldest first.
    #[must_use]
    pub fn lines(&self) -> &[LogLine] {
        &self.lines
    }

    /// How many notes were refused for being longer than [`MAX_LINE`].
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// What the ring's filter is: a note event's metadata passes.
pub type Only = fn(&tracing::Metadata<'_>) -> bool;

/// The lowest level the ring keeps: lower notes would push the warnings out of a report.
const FLOOR: Level = Level::INFO;

/// Whether an event's metadata is a note the ring keeps: a [`note!`](crate::note) of INFO or above, which a run's log
/// file keeps by the same test, so the file and a report never disagree on which crate's line is theirs.
#[must_use]
pub fn is_kept_note(meta: &tracing::Metadata<'_>) -> bool {
    meta.is_event() && meta.name() == NOTE && *meta.level() <= FLOOR && meta.fields().field(__MARK).is_some()
}

/// The newest [`KEPT`] notes that fit the log's byte bound, oldest dropped. A handle is cloned freely; each clone is the same ring. Install
/// [`Ring::layer`] beside a program's other layers.
#[derive(Clone)]
pub struct Ring {
    inner: Arc<Inner>,
}

impl fmt::Debug for Ring {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Ring").finish_non_exhaustive()
    }
}

impl Ring {
    /// An empty ring that times a note by `clock`, the program's one source of the time.
    #[must_use]
    pub fn new(clock: fn() -> SystemTime) -> Self {
        Self { inner: Arc::new(Inner { clock, kept: Mutex::default(), dropped: AtomicU64::new(0) }) }
    }

    /// The layer to register: it sees note events of INFO and above alone, whatever the other layers' filters pass,
    /// and wants nothing lower, so a program no other layer asks for debug notes of evaluates none.
    pub fn layer<S>(&self) -> Filtered<Self, FilterFn<Only>, S>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        self.clone().with_filter(filter_fn(is_kept_note as Only).with_max_level_hint(LevelFilter::INFO))
    }

    /// The kept lines, oldest first.
    #[must_use]
    pub fn lines(&self) -> Vec<LogLine> {
        self.inner.kept.lock().unwrap_or_else(PoisonError::into_inner).lines.iter().cloned().collect()
    }

    /// The kept lines with the count of notes too long to keep: what a [`Report`](crate::Report) holds as its log.
    #[must_use]
    pub fn recent(&self) -> Recent {
        Recent { lines: self.lines(), dropped: self.inner.dropped.load(Ordering::Relaxed) }
    }

    fn keep(&self, line: LogLine) {
        if line.0.len() > MAX_LINE {
            self.inner.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        // A panic hook notes the panic on the thread that panicked, which may hold this lock: that note is dropped.
        let mut kept = match self.inner.kept.try_lock() {
            Ok(kept) => kept,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) if std::thread::panicking() => return,
            Err(TryLockError::WouldBlock) => self.inner.kept.lock().unwrap_or_else(PoisonError::into_inner),
        };
        kept.bytes += weight(&line);
        kept.lines.push_back(line);
        while kept.lines.len() > KEPT || kept.bytes > LOG_BYTES {
            let Some(oldest) = kept.lines.pop_front() else { break };
            kept.bytes -= weight(&oldest);
        }
    }
}

fn level_word(level: Level) -> &'static str {
    match level {
        Level::TRACE => "trace",
        Level::DEBUG => "debug",
        Level::INFO => "info ",
        Level::WARN => "warn ",
        Level::ERROR => "error",
    }
}

impl<S: Subscriber> Layer<S> for Ring {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        if !is_kept_note(meta) {
            return;
        }
        let mut rendered = Rendered::default();
        event.record(&mut rendered);
        let seconds = (self.inner.clock)().duration_since(UNIX_EPOCH).ok().map(|since| since.as_secs());
        let krate = meta.target().split("::").next().unwrap_or_default();
        self.keep(LogLine(format!(
            "{} {} {krate} {}{}",
            time::clock(seconds),
            level_word(*meta.level()),
            rendered.message,
            rendered.fields
        )));
    }
}
