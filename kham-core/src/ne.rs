//! Named entity tagging via a gazetteer (word-list approach).
//!
//! [`NeTagger`] relabels pre-segmented Thai tokens that appear in the
//! gazetteer from [`TokenKind::Thai`] to [`TokenKind::Named`]`(kind)`.
//! The tagger runs as a **post-processing pass** after segmentation — it
//! does not change the segmentation boundaries, only the token kind.
//!
//! Three entity categories are supported: [`NamedEntityKind::Person`],
//! [`NamedEntityKind::Place`], and [`NamedEntityKind::Org`].
//!
//! # Data format
//!
//! Tab-separated text file, one entry per line:
//!
//! ```text
//! # Thai word<TAB>NE_TAG
//! กรุงเทพ<TAB>PLACE
//! ทักษิณ<TAB>PERSON
//! ปตท<TAB>ORG
//! ```
//!
//! Lines beginning with `#` and blank lines are ignored.
//! Duplicate keys: last entry wins.
//!
//! # Example
//!
//! ```rust
//! use kham_core::ne::NeTagger;
//! use kham_core::token::NamedEntityKind;
//!
//! let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\nทักษิณ\tPERSON\n");
//! assert_eq!(tagger.tag("กรุงเทพ"), Some(NamedEntityKind::Place));
//! assert_eq!(tagger.tag("xyz"), None);
//! ```

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::token::{NamedEntityKind, Token, TokenKind};

static BUILTIN_NE: &str = include_str!("../data/ne_th.tsv");

/// Gazetteer-based named entity tagger.
///
/// Construct once with [`NeTagger::builtin`] and reuse across calls.
pub struct NeTagger(BTreeMap<String, NamedEntityKind>);

impl NeTagger {
    /// Load the built-in NE gazetteer (hand-curated Thai NEs).
    pub fn builtin() -> Self {
        Self::from_tsv(BUILTIN_NE)
    }

    /// Parse a tab-separated NE gazetteer.
    ///
    /// Format: `thai_word\tNE_TAG` — one entry per line.
    /// Lines beginning with `#` and blank lines are skipped.
    /// Unknown tag strings are skipped silently.
    /// For duplicate keys, the last entry wins.
    pub fn from_tsv(data: &str) -> Self {
        let mut map: BTreeMap<String, NamedEntityKind> = BTreeMap::new();
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
            let tag_str = match parts.next() {
                Some(t) if !t.is_empty() => t.trim(),
                _ => continue,
            };
            if let Some(kind) = NamedEntityKind::from_tag(tag_str) {
                map.insert(word, kind);
            }
        }
        NeTagger(map)
    }

    /// Look up the NE category for a pre-segmented word.
    ///
    /// Returns `None` if the word is not in the gazetteer.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::ne::NeTagger;
    /// use kham_core::token::NamedEntityKind;
    ///
    /// let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
    /// assert_eq!(tagger.tag("กรุงเทพ"), Some(NamedEntityKind::Place));
    /// assert_eq!(tagger.tag("xyz"), None);
    /// ```
    pub fn tag(&self, word: &str) -> Option<NamedEntityKind> {
        self.0.get(word).copied()
    }

    /// Relabel [`TokenKind::Thai`] tokens that appear in the gazetteer to
    /// [`TokenKind::Named`]`(kind)`. All other tokens pass through unchanged.
    ///
    /// This is a post-processing pass — it does not alter token boundaries.
    pub fn tag_tokens<'a>(&self, tokens: Vec<Token<'a>>) -> Vec<Token<'a>> {
        tokens
            .into_iter()
            .map(|t| {
                if t.kind == TokenKind::Thai {
                    if let Some(ne_kind) = self.tag(t.text) {
                        return Token::new(t.text, t.span, t.char_span, TokenKind::Named(ne_kind));
                    }
                }
                t
            })
            .collect()
    }

    /// Number of entries in the gazetteer.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if the gazetteer has no entries.
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
    fn builtin_gazetteer_non_empty() {
        let t = NeTagger::builtin();
        assert!(t.len() > 50);
    }

    #[test]
    fn place_lookup() {
        let t = NeTagger::builtin();
        assert_eq!(t.tag("กรุงเทพ"), Some(NamedEntityKind::Place));
        assert_eq!(t.tag("ไทย"), Some(NamedEntityKind::Place));
        assert_eq!(t.tag("ญี่ปุ่น"), Some(NamedEntityKind::Place));
    }

    #[test]
    fn org_lookup() {
        let t = NeTagger::builtin();
        assert_eq!(t.tag("ปตท"), Some(NamedEntityKind::Org));
        assert_eq!(t.tag("ธนาคารแห่งประเทศไทย"), Some(NamedEntityKind::Org));
    }

    #[test]
    fn person_lookup() {
        let t = NeTagger::builtin();
        assert_eq!(t.tag("ทักษิณ"), Some(NamedEntityKind::Person));
    }

    #[test]
    fn oov_returns_none() {
        let t = NeTagger::builtin();
        assert_eq!(t.tag("กิน"), None);
        assert_eq!(t.tag(""), None);
    }

    #[test]
    fn from_tsv_last_duplicate_wins() {
        let t = NeTagger::from_tsv("กรุงเทพ\tPLACE\nกรุงเทพ\tORG\n");
        assert_eq!(t.tag("กรุงเทพ"), Some(NamedEntityKind::Org));
    }

    #[test]
    fn from_tsv_unknown_tag_skipped() {
        let t = NeTagger::from_tsv("กรุงเทพ\tCITY\n");
        assert_eq!(t.tag("กรุงเทพ"), None);
    }

    #[test]
    fn from_tsv_empty() {
        assert!(NeTagger::from_tsv("").is_empty());
    }

    #[test]
    fn tag_tokens_relabels_thai() {
        use crate::token::Token;
        let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
        let tok = Token::new("กรุงเทพ", 0..21, 0..7, TokenKind::Thai);
        let result = tagger.tag_tokens(alloc::vec![tok]);
        assert_eq!(result[0].kind, TokenKind::Named(NamedEntityKind::Place));
    }

    #[test]
    fn tag_tokens_passes_through_non_thai() {
        use crate::token::Token;
        let tagger = NeTagger::from_tsv("hello\tPERSON\n");
        let tok = Token::new("hello", 0..5, 0..5, TokenKind::Latin);
        let result = tagger.tag_tokens(alloc::vec![tok]);
        assert_eq!(result[0].kind, TokenKind::Latin); // not relabeled
    }

    #[test]
    fn tag_tokens_oov_unchanged() {
        use crate::token::Token;
        let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
        let tok = Token::new("กิน", 0..9, 0..3, TokenKind::Thai);
        let result = tagger.tag_tokens(alloc::vec![tok]);
        assert_eq!(result[0].kind, TokenKind::Thai);
    }

    #[test]
    fn named_entity_kind_roundtrip() {
        for kind in [
            NamedEntityKind::Person,
            NamedEntityKind::Place,
            NamedEntityKind::Org,
        ] {
            assert_eq!(NamedEntityKind::from_tag(kind.as_tag()), Some(kind));
        }
    }
}
