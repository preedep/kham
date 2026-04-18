//! Synonym expansion for Thai full-text search.
//!
//! [`SynonymMap`] maps a canonical form to a list of equivalent terms. During
//! FTS indexing, each matched canonical token is expanded to its synonyms so
//! that queries on any variant find the document.
//!
//! # Data format
//!
//! A tab-separated text file, one rule per line:
//!
//! ```text
//! # canonical<TAB>syn1<TAB>syn2 ...
//! คอม<TAB>คอมพิวเตอร์<TAB>computer
//! รถไฟฟ้า<TAB>BTS<TAB>MRT<TAB>รถไฟใต้ดิน
//! ```
//!
//! Lines beginning with `#` and blank lines are ignored.
//!
//! # Example
//!
//! ```rust
//! use kham_core::synonym::SynonymMap;
//!
//! let map = SynonymMap::from_tsv("คอม\tคอมพิวเตอร์\tcomputer\n");
//! let expansions = map.expand("คอม").unwrap_or_default();
//! assert!(expansions.contains(&"คอมพิวเตอร์".to_string()));
//! ```

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// A canonical-form → synonym-list lookup table.
///
/// Built from tab-separated data via [`SynonymMap::from_tsv`]. Lookup is
/// O(log n) via [`BTreeMap`] (suitable for sets up to ~5 000 entries; use an
/// FST-backed map for larger corpora).
pub struct SynonymMap(BTreeMap<String, Vec<String>>);

impl SynonymMap {
    /// Create an empty [`SynonymMap`] (no expansions).
    pub fn empty() -> Self {
        SynonymMap(BTreeMap::new())
    }

    /// Parse a tab-separated synonym table.
    ///
    /// Format: `canonical\tsyn1\tsyn2\t…` — one rule per line.
    /// Lines beginning with `#` and blank lines are skipped.
    /// Lines with fewer than two tab-separated fields are skipped silently.
    pub fn from_tsv(data: &str) -> Self {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let canonical = match parts.next() {
                Some(c) if !c.is_empty() => String::from(c),
                _ => continue,
            };
            let rest = match parts.next() {
                Some(r) if !r.is_empty() => r,
                _ => continue,
            };
            let synonyms: Vec<String> = rest
                .split('\t')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            if synonyms.is_empty() {
                continue;
            }
            map.entry(canonical).or_default().extend(synonyms);
        }
        SynonymMap(map)
    }

    /// Look up synonym expansions for `word`.
    ///
    /// Returns `None` if `word` has no entry. Returns the full synonym slice
    /// otherwise — the caller should index all returned strings alongside the
    /// canonical form.
    pub fn expand(&self, word: &str) -> Option<&[String]> {
        self.0.get(word).map(Vec::as_slice)
    }

    /// Return `true` if `word` has synonym expansions.
    #[inline]
    pub fn has_synonyms(&self, word: &str) -> bool {
        self.0.contains_key(word)
    }

    /// Number of canonical entries in the map.
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

    #[test]
    fn empty_map_returns_none() {
        let m = SynonymMap::empty();
        assert!(m.expand("คอม").is_none());
        assert!(m.is_empty());
    }

    #[test]
    fn single_synonym_parsed() {
        let m = SynonymMap::from_tsv("คอม\tคอมพิวเตอร์\n");
        let syns = m.expand("คอม").expect("should have synonyms");
        assert_eq!(syns, &[String::from("คอมพิวเตอร์")]);
    }

    #[test]
    fn multiple_synonyms_parsed() {
        let m = SynonymMap::from_tsv("รถไฟฟ้า\tBTS\tMRT\tรถไฟใต้ดิน\n");
        let syns = m.expand("รถไฟฟ้า").expect("should have synonyms");
        assert_eq!(syns.len(), 3);
        assert!(syns.contains(&String::from("BTS")));
        assert!(syns.contains(&String::from("MRT")));
        assert!(syns.contains(&String::from("รถไฟใต้ดิน")));
    }

    #[test]
    fn comment_lines_skipped() {
        let m = SynonymMap::from_tsv("# this is a comment\nคอม\tคอมพิวเตอร์\n");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn blank_lines_skipped() {
        let m = SynonymMap::from_tsv("\n\nคอม\tคอมพิวเตอร์\n\n");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn line_without_tab_skipped() {
        let m = SynonymMap::from_tsv("คอม\n");
        assert!(m.expand("คอม").is_none());
    }

    #[test]
    fn unknown_word_returns_none() {
        let m = SynonymMap::from_tsv("คอม\tคอมพิวเตอร์\n");
        assert!(m.expand("xyz").is_none());
        assert!(!m.has_synonyms("xyz"));
    }

    #[test]
    fn has_synonyms_true_for_known_word() {
        let m = SynonymMap::from_tsv("คอม\tคอมพิวเตอร์\n");
        assert!(m.has_synonyms("คอม"));
    }

    #[test]
    fn duplicate_canonical_merges_synonyms() {
        let m = SynonymMap::from_tsv("คอม\tคอมพิวเตอร์\nคอม\tcomputer\n");
        let syns = m.expand("คอม").expect("should have synonyms");
        assert!(syns.contains(&String::from("คอมพิวเตอร์")));
        assert!(syns.contains(&String::from("computer")));
    }

    #[test]
    fn empty_input_produces_empty_map() {
        assert!(SynonymMap::from_tsv("").is_empty());
    }

    #[test]
    fn whitespace_trimmed_from_synonyms() {
        let m = SynonymMap::from_tsv("คอม\t คอมพิวเตอร์ \n");
        let syns = m.expand("คอม").expect("should have synonyms");
        assert_eq!(syns, &[String::from("คอมพิวเตอร์")]);
    }
}
