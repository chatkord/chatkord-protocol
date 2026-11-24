//! The wire types both ends generate from `wire/`, and the rules both read: the envelope, names and grants, the
//! SFU's parameters, files.
#![forbid(unsafe_code)]

/// The types `wire/chatkord/v1` generates, one module for the package; its comments are the schema's.
#[rustfmt::skip]
#[expect(clippy::enum_variant_names, reason = "a oneof's variants are its wire fields' names, renamed only by a major")]
#[path = "../gen/chatkord/v1/chatkord.v1.rs"]
pub mod chatkord_v1;

mod entries;
pub mod envelope;

/// The one mediasoup-types release both ends read a peer's SFU parameters as (`sdp::sfu_json`): a consumer names
/// it only through this re-export, so no two ends read the parameters as different releases.
pub use mediasoup_types;

pub mod files;
#[cfg(any(test, feature = "testing"))]
pub mod filled;
#[cfg(all(any(test, fuzzing, feature = "testing"), not(feature = "codegen")))]
pub mod fuzz;
pub mod notify;
pub mod ops;
pub mod pin;
pub mod rules;
pub mod sdp;
pub mod status;
pub mod video;

mod redact;
mod schema;
