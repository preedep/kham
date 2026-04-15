//! Thai text normalizer.
//!
//! Performs the following transformations in order:
//!
//! 1. **สระลอย reorder** — moves floating vowels (สระ อิ อี อึ อื ฯลฯ) to their canonical position.
//! 2. **วรรณยุกต์ dedup** — removes duplicate tone marks stacked on the same base consonant.
//! 3. **NFC normalisation** — applies Unicode NFC so that composed and decomposed forms compare equal.

use alloc::string::String;

/// Normalise Thai text into canonical form.
///
/// Returns an owned `String` with all transformations applied.
///
/// # Example
///
/// ```rust
/// use kham_core::normalizer::normalize;
///
/// // Floating vowel reorder
/// let out = normalize("กิน");
/// assert_eq!(out, "กิน");
/// ```
pub fn normalize(text: &str) -> String {
    // TODO: implement สระลอย reorder, วรรณยุกต์ dedup, NFC
    text.into()
}
