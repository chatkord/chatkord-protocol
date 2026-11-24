//! The typed log: a literal message and public fields, as a `tracing` event the [`Ring`](crate::Ring) can tell.

use std::fmt;

use crate::Public;

/// The name every note event carries; the ring keeps an event with this name and [`__MARK`], which an
/// [`operator!`](crate::operator) event lacks.
pub const NOTE: &str = "note";

/// The field only [`note!`] puts in a note's field set, so a foreign event named `note` is not taken for one.
#[doc(hidden)]
pub const __MARK: &str = "report.note";

/// A [`Public`] value as the text a `tracing` field shows.
#[derive(Debug)]
pub struct Shown(crate::Fact);

impl fmt::Display for Shown {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(out)
    }
}

/// A field's value as its fact: this call is what makes a field that is not [`Public`] a build error.
pub fn shown<T: Public + ?Sized>(value: &T) -> Shown {
    Shown(value.fact())
}

/// The `tracing` level of one of the five words [`note!`] and [`operator!`] take.
#[doc(hidden)]
#[macro_export]
macro_rules! __level {
    (trace) => {
        $crate::__tracing::Level::TRACE
    };
    (debug) => {
        $crate::__tracing::Level::DEBUG
    };
    (info) => {
        $crate::__tracing::Level::INFO
    };
    (warn) => {
        $crate::__tracing::Level::WARN
    };
    (error) => {
        $crate::__tracing::Level::ERROR
    };
}

/// Logs an event of `level` (`trace`, `debug`, `info`, `warn` or `error`) as a string literal and fields whose
/// values are [`Public`]: `note!(warn, "a decoder fell back to software", codec = codec, hardware = false)`. The
/// message takes no format arguments, so no text a program computed reaches a log; a field that is a `String`, a
/// path, an address or an id does not build. The event goes to every subscriber as any `tracing` event does, and the
/// [`Ring`](crate::Ring) keeps it for a debug report. A field's value is computed only when the event is enabled.
#[macro_export]
macro_rules! note {
    ($level:ident, $message:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::__event!($crate::NOTE, [$crate::__MARK], $level, $message $(, $name = $crate::__display($crate::shown(&$value)))*)
    };
}

/// Logs a line only an operator's own terminal may show, whose fields may be anything that displays: an address or
/// an owner code the operator asked to see. It is not a note, so the [`Ring`](crate::Ring) never keeps it and no
/// debug report holds it.
#[macro_export]
macro_rules! operator {
    ($level:ident, $message:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::__event!("operator", [], $level, $message $(, $name = $crate::__display(&$value))*)
    };
}

/// One `tracing` event built from the callsite up, as `tracing::event!` builds it, so that no `tracing` log macro is
/// expanded at a call site and clippy can refuse every direct one.
#[doc(hidden)]
#[macro_export]
macro_rules! __event {
    ($kind:expr, [$($mark:expr),*], $level:ident, $message:literal $(, $name:ident = $value:expr)*) => {{
        static META: $crate::__tracing::Metadata<'static> = $crate::__tracing::Metadata::new(
            $kind,
            ::core::module_path!(),
            $crate::__level!($level),
            ::core::option::Option::Some(::core::file!()),
            ::core::option::Option::Some(::core::line!()),
            ::core::option::Option::Some(::core::module_path!()),
            $crate::__tracing::field::FieldSet::new(
                &["message", $($mark,)* $(::core::stringify!($name)),*],
                $crate::__tracing::callsite::Identifier(&CALLSITE),
            ),
            $crate::__tracing::metadata::Kind::EVENT,
        );
        static CALLSITE: $crate::__tracing::callsite::DefaultCallsite =
            $crate::__tracing::callsite::DefaultCallsite::new(&META);
        if $crate::site::enabled(&CALLSITE, &META) {
            $crate::site::dispatch(
                &META,
                [
                    ::core::option::Option::Some(&::core::format_args!("{}", $message) as &dyn $crate::__tracing::Value),
                    $(::core::option::Option::Some(&{ let _ = $mark; true } as &dyn $crate::__tracing::Value),)*
                    $(::core::option::Option::Some(&$value as &dyn $crate::__tracing::Value),)*
                ],
            );
        }
    }};
}

/// A field's text as a `tracing` value.
#[doc(hidden)]
pub use tracing::field::display as __display;
