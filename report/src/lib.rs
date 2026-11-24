//! What a log line or a debug report may hold, by type: [`Fact`], a count, a flag, a word written in the source, a
//! product name checked at the edge, or a stand-in for an id that holds within one report. A value becomes one only
//! through [`Public`], which a `String`, a path, an address or an id never implements, so the leak is a build error and
//! no scrub runs after. [`note!`] is the log, [`Ring`] keeps its recent lines for a [`Report`], and [`operator!`] is
//! the one door to a terminal that only the operator sees.
//!
//! ```compile_fail
//! let name = String::from("ada");
//! report::note!(info, "a member signed in", who = name);
//! ```
//!
//! ```compile_fail
//! let name: &str = "ada";
//! report::note!(info, "a member signed in", who = name);
//! ```
//!
//! ```compile_fail
//! let peer = std::net::IpAddr::from([127, 0, 0, 1]);
//! report::note!(warn, "a peer was refused", peer = peer);
//! ```
//!
//! ```compile_fail
//! let place = std::path::PathBuf::from("<dir>/ada");
//! report::note!(warn, "a file was refused", place = place);
//! ```
//!
//! ```compile_fail
//! let message = String::from("a member signed in");
//! report::note!(info, message);
//! ```
//!
//! ```
//! report::note!(info, "a share started", frames = 30_u32, hardware = true);
//! ```

#![forbid(unsafe_code)]

// The derive names this crate by its path, which inside the crate is itself.
extern crate self as report;

mod document;
mod fact;
mod listing;
mod note;
mod previous;
mod product;
mod ring;
#[doc(hidden)]
pub mod site;
mod stand;
mod time;

pub use document::{
    Build, Full, MAX_BUILD_WORD, MAX_LABEL, MAX_REPORT_BYTES, MAX_ROWS, MAX_SECTIONS, MAX_TITLE, RECENT_LOG, Report,
    Section, Side, file_name,
};
pub use fact::{Case, Fact, Public, Word, count16, count32, millis};
pub use listing::{ListedPart, Listing, LogTail, NotAReport};
#[doc(hidden)]
pub use note::{__MARK, __display};
pub use note::{NOTE, Shown, shown};
pub use previous::{MAX_FILE_LINE, Previous, READ_BACK};
pub use product::Product;
pub use report_derive::Public;
pub use ring::{KEPT, LogLine, MAX_LINE, Only, Recent, Ring, is_kept_note};
pub use stand::{MAX_STANDS, Stand, Stands};

#[doc(hidden)]
pub use tracing as __tracing;
