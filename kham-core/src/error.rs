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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::string::ToString;

    #[test]
    fn display_invalid_utf8() {
        assert_eq!(KhamError::InvalidUtf8.to_string(), "invalid UTF-8 sequence");
    }

    #[test]
    fn display_dict_load_error_includes_message() {
        let e = KhamError::DictLoadError(String::from("file not found"));
        assert_eq!(e.to_string(), "dictionary load error: file not found");
    }

    #[test]
    fn display_corrupt_dict() {
        assert_eq!(
            KhamError::CorruptDict.to_string(),
            "dictionary data is corrupt or has wrong version"
        );
    }

    #[test]
    fn display_empty_input() {
        assert_eq!(KhamError::EmptyInput.to_string(), "input must not be empty");
    }

    #[test]
    fn clone_produces_equal_value() {
        let original = KhamError::DictLoadError(String::from("oops"));
        assert_eq!(original.clone(), original);
    }

    #[test]
    fn partial_eq_same_variant() {
        assert_eq!(KhamError::InvalidUtf8, KhamError::InvalidUtf8);
        assert_eq!(KhamError::CorruptDict, KhamError::CorruptDict);
        assert_eq!(KhamError::EmptyInput, KhamError::EmptyInput);
    }

    #[test]
    fn partial_eq_different_variants() {
        assert_ne!(KhamError::InvalidUtf8, KhamError::CorruptDict);
        assert_ne!(KhamError::EmptyInput, KhamError::InvalidUtf8);
    }

    #[test]
    fn partial_eq_dict_load_error_compares_message() {
        assert_eq!(
            KhamError::DictLoadError(String::from("a")),
            KhamError::DictLoadError(String::from("a"))
        );
        assert_ne!(
            KhamError::DictLoadError(String::from("a")),
            KhamError::DictLoadError(String::from("b"))
        );
    }
}
