//! Watch-list audio fingerprinting for radio and stream monitoring.
//!
//! The pipeline turns the constant-Q magnitudes of [`cqt_rs`] into a sparse
//! set of spectral peaks ([`PeakPicker`]), combines peaks into pitch- and
//! tempo-invariant triplet hashes ([`TripletHasher`]), stores the hashes of
//! the watched songs in an [`Index`] and accumulates the evidence of a live
//! stream in a sliding window ([`Matcher`]) that reports which watched song
//! is playing, how it was pitched and stretched, where in the song the
//! stream is, and a confidence score.
//!
//! Every stage is streaming with a bounded, documented delay and no
//! per-frame allocation once warmed up, so the whole chain runs on a small
//! fraction of one core.

#![warn(missing_docs)]

mod hashes;
mod index;
mod matcher;
mod peaks;

pub use hashes::{Hash, HashKey, TripletHasher, decode_key, encode_key};
pub use index::{Entry, Index, IndexBuilder};
pub use matcher::{Candidate, Matcher, MatcherConfig};
pub use peaks::{Peak, PeakPicker};
