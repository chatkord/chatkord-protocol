//! The crate's integration tests as one binary: a module for each subject, the helpers beside them.
// Clippy reads this lint's level at the crate alone, so the one foreign event a test sends is excused here.
#![expect(clippy::disallowed_macros, reason = "one test sends a foreign event the ring must refuse")]

mod document;
mod typed_log;
