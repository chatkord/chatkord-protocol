//! What a note's expansion calls at run time: the same enabled check and dispatch `tracing::event!` makes.

use tracing::callsite::DefaultCallsite;
use tracing::field::{Field, Value};
use tracing::level_filters::{LevelFilter, STATIC_MAX_LEVEL};
use tracing::{Event, Metadata, dispatcher};

/// Whether some subscriber wants the event of `meta`, asked the way `tracing::event!` asks: the compile-time ceiling,
/// the current maximum, then the callsite's cached interest.
#[doc(hidden)]
pub fn enabled(callsite: &'static DefaultCallsite, meta: &'static Metadata<'static>) -> bool {
    let level = *meta.level();
    if level > STATIC_MAX_LEVEL || level > LevelFilter::current() {
        return false;
    }
    let interest = callsite.interest();
    !interest.is_never() && (interest.is_always() || dispatcher::get_default(|current| current.enabled(meta)))
}

/// Sends the event of `meta` with one value per field, in the field set's order, as `tracing`'s own macros pair them.
#[doc(hidden)]
pub fn dispatch<const N: usize>(meta: &'static Metadata<'static>, values: [Option<&dyn Value>; N]) {
    let fields = meta.fields();
    // The expansion gives exactly one value per field; a field set without any would have no message to send.
    let Some(first) = fields.iter().next() else { return };
    let owned: [Field; N] = std::array::from_fn(|at| fields.iter().nth(at).unwrap_or_else(|| first.clone()));
    let pairs: [(&Field, Option<&dyn Value>); N] = std::array::from_fn(|at| (&owned[at], values[at]));
    Event::dispatch(meta, &fields.value_set(&pairs));
}
