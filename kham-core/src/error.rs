//! Error types for kham-core.

use alloc::string::String;
use core::fmt;

/// All errors that kham-core can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KhamError {
    /// The supplied text was not valid UTF-8.
    InvalidUtf8,
    /// A dictionary file could not be loaded.
    ///
    /// Contains a human-readable description of the problem.
    DictLoadError(String),
    /// The trie data is malformed or has an unexpected version.
    CorruptDict,
    /// An empty input was provided where non-empty input is required.
    EmptyInput,
}

impl fmt::Display for KhamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KhamError::InvalidUtf8 => f.write_str("invalid UTF-8 sequence"),
            KhamError::DictLoadError(msg) => write!(f, "dictionary load error: {msg}"),
            KhamError::CorruptDict => {
                f.write_str("dictionary data is corrupt or has wrong version")
            }
            KhamError::EmptyInput => f.write_str("input must not be empty"),
        }
    }
}

// Bring in std explicitly since this crate is #![no_std].
#[cfg(feature = "std")]
extern crate std;

// `std::error::Error` is only available when the `std` feature is enabled.
#[cfg(feature = "std")]
impl std::error::Error for KhamError {}
