//! Thai stopword filter.
//!
//! [`StopwordSet`] identifies common Thai function words (particles, conjunctions,
//! pronouns, discourse markers) that carry little lexical meaning and should be
//! excluded from full-text search indexes.
//!
//! The built-in list (1 029 entries) is sourced from PyThaiNLP (Apache-2.0).
//!
//! # Example
//!
//! ```rust
//! use kham_core::stopwords::StopwordSet;
//!
//! let stops = StopwordSet::builtin();
//! assert!(stops.contains("และ"));
//! assert!(!stops.contains("กินข้าว"));
//! ```

use alloc::string::String;
use alloc::vec::Vec;

static BUILTIN_STOPWORDS: &str = include_str!("../data/stopwords_th.txt");

/// A sorted set of stopwords supporting O(log n) lookup.
///
/// Construct once per process with [`StopwordSet::builtin`] and reuse across
/// segmentation calls.
pub struct StopwordSet {
    words: Vec<String>,
}

impl StopwordSet {
    /// Load the built-in Thai stopword list (1 029 entries, PyThaiNLP Apache-2.0).
    pub fn builtin() -> Self {
        Self::from_text(BUILTIN_STOPWORDS)
    }

    /// Build a [`StopwordSet`] from a newline-separated word list.
    ///
    /// Lines beginning with `#` and blank lines are ignored.
    /// BOM characters (`\u{FEFF}`) are stripped from every line.
    /// The resulting set is sorted and deduplicated.
    pub fn from_text(data: &str) -> Self {
        let mut words: Vec<String> = data
            .lines()
            .map(|l| l.trim_start_matches('\u{FEFF}').trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect();
        words.sort_unstable();
        words.dedup();
        StopwordSet { words }
    }

    /// Return `true` if `word` is in the stopword set.
    #[inline]
    pub fn contains(&self, word: &str) -> bool {
        self.words
            .binary_search_by(|w| w.as_str().cmp(word))
            .is_ok()
    }

    /// Number of stopwords in this set.
    #[inline]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Return `true` if the set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn stops() -> StopwordSet {
        StopwordSet::builtin()
    }

    #[test]
    fn builtin_loads_without_panic() {
        let _ = stops();
    }

    #[test]
    fn builtin_has_expected_count() {
        let s = stops();
        assert!(s.len() >= 1000, "expected ≥1000 stopwords, got {}", s.len());
    }

    #[test]
    fn common_function_words_are_stopwords() {
        let s = stops();
        for word in &["และ", "ที่", "ของ", "ใน", "ไม่", "ได้", "กับ", "จาก"]
        {
            assert!(s.contains(word), "expected '{word}' to be a stopword");
        }
    }

    #[test]
    fn content_words_are_not_stopwords() {
        let s = stops();
        for word in &["กินข้าว", "โรงพยาบาล", "คอมพิวเตอร์", "ประเทศไทย"]
        {
            assert!(!s.contains(word), "'{word}' should not be a stopword");
        }
    }

    #[test]
    fn empty_string_is_not_a_stopword() {
        assert!(!stops().contains(""));
    }

    #[test]
    fn from_text_ignores_comment_lines() {
        let s = StopwordSet::from_text("# comment\nกิน\nข้าว\n");
        assert!(s.contains("กิน"));
        assert!(s.contains("ข้าว"));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn from_text_ignores_blank_lines() {
        let s = StopwordSet::from_text("\nกิน\n\nข้าว\n");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn from_text_strips_bom() {
        let s = StopwordSet::from_text("\u{FEFF}กิน\nข้าว\n");
        assert!(s.contains("กิน"), "BOM should be stripped before lookup");
    }

    #[test]
    fn from_text_deduplicates() {
        let s = StopwordSet::from_text("กิน\nกิน\nกิน\n");
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn empty_input_produces_empty_set() {
        let s = StopwordSet::from_text("");
        assert!(s.is_empty());
    }

    #[test]
    fn contains_is_exact_match() {
        let s = StopwordSet::from_text("กิน\n");
        assert!(s.contains("กิน"));
        assert!(!s.contains("กิน "));
        assert!(!s.contains("กินข้าว"));
    }
}
