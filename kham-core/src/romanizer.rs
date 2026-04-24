//! RTGS romanization of segmented Thai words.
//!
//! [`RomanizationMap`] maps pre-segmented Thai words to their Roman (Latin)
//! phonetic equivalents using the Royal Thai General System of Transcription
//! (RTGS) — the Thai government standard used in road signs, passports, and
//! official documents.
//!
//! This implementation is **table-driven only**. A rule-based phonetic engine
//! is a future `#[cfg(feature = "phonetic")]` extension.
//!
//! # RTGS characteristics
//!
//! - Consonant-by-consonant transliteration (initial vs. final position differ)
//! - No tone marks in output
//! - No vowel-length distinction (อิ and อี both map to `i`)
//! - Diphthongs and vowel clusters have explicit multi-character mappings
//!
//! # Data format
//!
//! Tab-separated text file, one entry per line:
//!
//! ```text
//! # Thai word<TAB>RTGS romanization
//! กิน<TAB>kin
//! ข้าว<TAB>khao
//! ปลา<TAB>pla
//! ```
//!
//! Lines beginning with `#` and blank lines are ignored.
//! Duplicate keys: last entry wins (allows override files).
//!
//! # Example
//!
//! ```rust
//! use kham_core::romanizer::RomanizationMap;
//!
//! let map = RomanizationMap::builtin();
//! assert_eq!(map.romanize("กิน"), Some("kin"));
//! assert_eq!(map.romanize_or_raw("ข้าว"), "khao");
//! assert_eq!(map.romanize_or_raw("xyz"), "xyz");
//!
//! let tokens = vec!["กิน", "ข้าว", "ปลา"];
//! assert_eq!(map.romanize_tokens(&tokens), vec!["kin", "khao", "pla"]);
//! ```

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

static BUILTIN_ROMANIZATION: &str = include_str!("../data/romanization_th.tsv");

/// A Thai-word → RTGS-romanization lookup table.
///
/// Built from tab-separated data via [`RomanizationMap::from_tsv`].
/// Lookup is O(log n) via [`BTreeMap`].
pub struct RomanizationMap(BTreeMap<String, String>);

impl RomanizationMap {
    /// Load the built-in RTGS romanization table.
    pub fn builtin() -> Self {
        Self::from_tsv(BUILTIN_ROMANIZATION)
    }

    /// Parse a tab-separated romanization table.
    ///
    /// Format: `thai_word\trtgs_romanization` — one entry per line.
    /// Lines beginning with `#` and blank lines are skipped.
    /// For duplicate keys, the last entry wins.
    pub fn from_tsv(data: &str) -> Self {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let word = match parts.next() {
                Some(w) if !w.is_empty() => String::from(w),
                _ => continue,
            };
            let roman = match parts.next() {
                Some(r) if !r.is_empty() => String::from(r.trim()),
                _ => continue,
            };
            map.insert(word, roman);
        }
        RomanizationMap(map)
    }

    /// Look up the RTGS romanization for a pre-segmented Thai word.
    ///
    /// Returns `None` if the word is not in the table.
    /// The returned `&str` borrows from the map — zero-copy for hits.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::from_tsv("กิน\tkin\n");
    /// assert_eq!(map.romanize("กิน"), Some("kin"));
    /// assert_eq!(map.romanize("xyz"), None);
    /// ```
    pub fn romanize(&self, word: &str) -> Option<&str> {
        self.0.get(word).map(String::as_str)
    }

    /// Return the RTGS romanization for `word`, or `word` unchanged if not found.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::from_tsv("กิน\tkin\n");
    /// assert_eq!(map.romanize_or_raw("กิน"), "kin");
    /// assert_eq!(map.romanize_or_raw("xyz"), "xyz");
    /// ```
    pub fn romanize_or_raw<'a>(&'a self, word: &'a str) -> &'a str {
        self.0.get(word).map(String::as_str).unwrap_or(word)
    }

    /// Romanize a slice of pre-segmented token strings.
    ///
    /// Returns a `Vec<String>` aligned 1:1 with the input slice. Tokens not
    /// found in the table are returned unchanged (same behaviour as
    /// [`romanize_or_raw`](Self::romanize_or_raw)).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::from_tsv("กิน\tkin\nปลา\tpla\n");
    /// let out = map.romanize_tokens(&["กิน", "ปลา"]);
    /// assert_eq!(out, vec!["kin", "pla"]);
    /// ```
    pub fn romanize_tokens(&self, tokens: &[&str]) -> Vec<String> {
        tokens
            .iter()
            .map(|t| String::from(self.romanize_or_raw(t)))
            .collect()
    }

    /// Number of entries in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if the map has no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn builtin_common_words() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize("กิน"), Some("kin"));
        assert_eq!(map.romanize("ข้าว"), Some("khao"));
        assert_eq!(map.romanize("น้ำ"), Some("nam"));
        assert_eq!(map.romanize("ปลา"), Some("pla"));
    }

    #[test]
    fn unknown_word_returns_none() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize("เปปซี่"), None);
    }

    #[test]
    fn romanize_or_raw_fallback() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize_or_raw("เปปซี่"), "เปปซี่");
    }

    #[test]
    fn romanize_or_raw_hit() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize_or_raw("กิน"), "kin");
    }

    #[test]
    fn from_tsv_last_duplicate_wins() {
        let map = RomanizationMap::from_tsv("กิน\tkin\nกิน\tgin\n");
        assert_eq!(map.romanize("กิน"), Some("gin"));
    }

    #[test]
    fn romanize_tokens_aligned() {
        let map = RomanizationMap::from_tsv("กิน\tkin\nปลา\tpla\n");
        let out = map.romanize_tokens(&["กิน", "ปลา"]);
        assert_eq!(out, vec!["kin", "pla"]);
    }

    #[test]
    fn romanize_tokens_unknown_passthrough() {
        let map = RomanizationMap::from_tsv("กิน\tkin\n");
        let out = map.romanize_tokens(&["กิน", "xyz"]);
        assert_eq!(out, vec!["kin", "xyz"]);
    }

    #[test]
    fn comment_and_blank_lines_skipped() {
        let map = RomanizationMap::from_tsv("# comment\n\nกิน\tkin\n");
        assert_eq!(map.len(), 1);
        assert_eq!(map.romanize("กิน"), Some("kin"));
    }

    #[test]
    fn line_without_tab_skipped() {
        let map = RomanizationMap::from_tsv("กิน\n");
        assert!(map.is_empty());
    }

    #[test]
    fn whitespace_trimmed_from_romanization() {
        let map = RomanizationMap::from_tsv("กิน\t kin \n");
        assert_eq!(map.romanize("กิน"), Some("kin"));
    }

    #[test]
    fn empty_input_produces_empty_map() {
        assert!(RomanizationMap::from_tsv("").is_empty());
    }

    #[test]
    fn romanize_tokens_empty_slice() {
        let map = RomanizationMap::builtin();
        assert!(map.romanize_tokens(&[]).is_empty());
    }
}
