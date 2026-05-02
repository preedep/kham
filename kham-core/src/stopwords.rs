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

    /// Load the built-in list plus additional words from `extra`.
    ///
    /// `extra` uses the same format as [`from_text`]: newline-separated words,
    /// `#` comment lines and blank lines ignored, BOM stripped.
    /// The combined set is sorted and deduplicated.
    ///
    /// Use this when you have domain-specific function words to suppress in
    /// addition to the standard Thai stopword list.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::stopwords::StopwordSet;
    ///
    /// let stops = StopwordSet::builtin_with_extra("ดาวน์โหลด\nอัปโหลด\n");
    /// assert!(stops.contains("และ"));       // built-in
    /// assert!(stops.contains("ดาวน์โหลด")); // extra
    /// ```
    ///
    /// [`from_text`]: StopwordSet::from_text
    pub fn builtin_with_extra(extra: &str) -> Self {
        let mut words: Vec<String> = BUILTIN_STOPWORDS
            .lines()
            .chain(extra.lines())
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

    #[test]
    fn builtin_with_extra_includes_builtin_words() {
        let s = StopwordSet::builtin_with_extra("ดาวน์โหลด\n");
        assert!(s.contains("และ"), "built-in word should be present");
        assert!(s.contains("ที่"), "built-in word should be present");
    }

    #[test]
    fn builtin_with_extra_includes_extra_words() {
        let s = StopwordSet::builtin_with_extra("ดาวน์โหลด\nอัปโหลด\n");
        assert!(s.contains("ดาวน์โหลด"), "extra word should be present");
        assert!(s.contains("อัปโหลด"), "extra word should be present");
    }

    #[test]
    fn builtin_with_extra_deduplicates_overlap() {
        let builtin = StopwordSet::builtin();
        // "และ" is already in the built-in list — adding it again should not duplicate.
        let combined = StopwordSet::builtin_with_extra("และ\n");
        assert_eq!(
            combined.len(),
            builtin.len(),
            "duplicate word should not increase set size"
        );
    }

    #[test]
    fn builtin_with_extra_empty_extra_equals_builtin() {
        let a = StopwordSet::builtin();
        let b = StopwordSet::builtin_with_extra("");
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn builtin_with_extra_ignores_comment_and_blank_lines() {
        let base = StopwordSet::builtin();
        let s = StopwordSet::builtin_with_extra("# comment\n\nและ\n");
        assert_eq!(
            s.len(),
            base.len(),
            "comment/blank/duplicate should not add entries"
        );
    }
}
