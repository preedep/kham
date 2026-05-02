//! Named entity tagging via a gazetteer (word-list approach).
//!
//! [`NeTagger`] relabels pre-segmented Thai tokens that appear in the
//! gazetteer from [`TokenKind::Thai`] to [`TokenKind::Named`]`(kind)`.
//! The tagger runs as a **post-processing pass** after segmentation — it
//! does not change the segmentation boundaries, only the token kind.
//!
//! **Multi-token matching:** [`NeTagger::tag_tokens`] uses greedy
//! longest-match over consecutive Thai tokens, so compound names split
//! by the segmenter (e.g. `กรุง`+`เทพ` → `กรุงเทพ`) are correctly
//! identified and merged into a single [`TokenKind::Named`] token.
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

static BUILTIN_NE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ne_th.bin"));

/// Gazetteer-based named entity tagger.
///
/// Construct once with [`NeTagger::builtin`] and reuse across calls.
pub struct NeTagger(BTreeMap<String, NamedEntityKind>);

impl NeTagger {
    /// Load the built-in NE gazetteer (hand-curated Thai NEs).
    pub fn builtin() -> Self {
        Self::from_tsv(&crate::decompress_builtin(BUILTIN_NE))
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

    /// Relabel sequences of consecutive [`TokenKind::Thai`] tokens to
    /// [`TokenKind::Named`]`(kind)` using **greedy longest-match**.
    ///
    /// For each Thai token, the longest consecutive span (up to 5 tokens)
    /// whose concatenated text hits the gazetteer is chosen. This handles
    /// compound names that the segmenter splits across multiple tokens —
    /// for example `กรุง`+`เทพ` → `กรุงเทพ` (PLACE).
    ///
    /// Merged tokens borrow their `text` as a zero-copy slice of `source`.
    /// Non-Thai tokens always pass through unchanged.
    ///
    /// `source` must be the normalised string from which `tokens` were
    /// produced (i.e. the same string passed to `Tokenizer::segment`).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::ne::NeTagger;
    /// use kham_core::token::{Token, TokenKind, NamedEntityKind};
    ///
    /// let source = "กรุงเทพ";
    /// let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
    /// // Simulate segmenter splitting กรุงเทพ into กรุง + เทพ
    /// // Each Thai char is 3 bytes: กรุง = 12 bytes, เทพ = 9 bytes
    /// let tokens = vec![
    ///     Token::new("กรุง", 0..12,  0..4, TokenKind::Thai, 1.0),
    ///     Token::new("เทพ",  12..21, 4..7, TokenKind::Thai, 1.0),
    /// ];
    /// let tagged = tagger.tag_tokens(tokens, source);
    /// assert_eq!(tagged.len(), 1);
    /// assert_eq!(tagged[0].text, "กรุงเทพ");
    /// assert_eq!(tagged[0].kind, TokenKind::Named(NamedEntityKind::Place));
    /// ```
    pub fn tag_tokens<'a>(&self, tokens: Vec<Token<'a>>, source: &'a str) -> Vec<Token<'a>> {
        // Maximum number of consecutive Thai tokens to try merging.
        const MAX_SPAN: usize = 5;

        let mut out: Vec<Token<'a>> = Vec::with_capacity(tokens.len());
        let mut i = 0;

        while i < tokens.len() {
            if tokens[i].kind != TokenKind::Thai {
                out.push(tokens[i].clone());
                i += 1;
                continue;
            }

            // Find the end of the consecutive Thai run starting at i.
            let run_end = tokens[i..]
                .iter()
                .position(|t| t.kind != TokenKind::Thai)
                .map_or(tokens.len(), |pos| i + pos);
            let max_end = run_end.min(i + MAX_SPAN);

            // Greedy longest-match: try longest span first, shrink until hit.
            let mut matched = false;
            for end in (i + 1..=max_end).rev() {
                let span_start = tokens[i].span.start;
                let span_end = tokens[end - 1].span.end;
                let candidate = &source[span_start..span_end];
                if let Some(ne_kind) = self.tag(candidate) {
                    let char_start = tokens[i].char_span.start;
                    let char_end = tokens[end - 1].char_span.end;
                    let confidence = tokens[i..end]
                        .iter()
                        .map(|t| t.confidence)
                        .fold(1.0_f32, f32::min);
                    out.push(Token::new(
                        candidate,
                        span_start..span_end,
                        char_start..char_end,
                        TokenKind::Named(ne_kind),
                        confidence,
                    ));
                    i = end;
                    matched = true;
                    break;
                }
            }

            if !matched {
                out.push(tokens[i].clone());
                i += 1;
            }
        }

        out
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
        let source = "กรุงเทพ";
        let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
        let tok = Token::new("กรุงเทพ", 0..21, 0..7, TokenKind::Thai, 1.0);
        let result = tagger.tag_tokens(alloc::vec![tok], source);
        assert_eq!(result[0].kind, TokenKind::Named(NamedEntityKind::Place));
    }

    #[test]
    fn tag_tokens_passes_through_non_thai() {
        use crate::token::Token;
        let source = "hello";
        let tagger = NeTagger::from_tsv("hello\tPERSON\n");
        let tok = Token::new("hello", 0..5, 0..5, TokenKind::Latin, 1.0);
        let result = tagger.tag_tokens(alloc::vec![tok], source);
        assert_eq!(result[0].kind, TokenKind::Latin); // not relabeled
    }

    #[test]
    fn tag_tokens_oov_unchanged() {
        use crate::token::Token;
        let source = "กิน";
        let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
        let tok = Token::new("กิน", 0..9, 0..3, TokenKind::Thai, 1.0);
        let result = tagger.tag_tokens(alloc::vec![tok], source);
        assert_eq!(result[0].kind, TokenKind::Thai);
    }

    // ── multi-token NE tests ──────────────────────────────────────────────────

    #[test]
    fn tag_tokens_multi_merges_two_tokens() {
        use crate::token::Token;
        // กรุงเทพ splits into กรุง + เทพ
        // Each Thai char is 3 bytes: กรุง=12 bytes (4 chars), เทพ=9 bytes (3 chars)
        let source = "กรุงเทพ";
        let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
        let tokens = alloc::vec![
            Token::new("กรุง", 0..12, 0..4, TokenKind::Thai, 1.0),
            Token::new("เทพ", 12..21, 4..7, TokenKind::Thai, 1.0),
        ];
        let result = tagger.tag_tokens(tokens, source);
        assert_eq!(result.len(), 1, "two tokens should merge into one");
        assert_eq!(result[0].text, "กรุงเทพ");
        assert_eq!(result[0].kind, TokenKind::Named(NamedEntityKind::Place));
        assert_eq!(result[0].span, 0..21);
        assert_eq!(result[0].char_span, 0..7);
    }

    #[test]
    fn tag_tokens_multi_greedy_prefers_longer() {
        use crate::token::Token;
        // Both "กรุงเทพ" (2-token) and "กรุง" (1-token) in gazetteer —
        // longest match (กรุงเทพ) must win.
        let source = "กรุงเทพ";
        let tagger = NeTagger::from_tsv("กรุง\tPLACE\nกรุงเทพ\tPLACE\n");
        let tokens = alloc::vec![
            Token::new("กรุง", 0..12, 0..4, TokenKind::Thai, 1.0),
            Token::new("เทพ", 12..21, 4..7, TokenKind::Thai, 1.0),
        ];
        let result = tagger.tag_tokens(tokens, source);
        assert_eq!(result.len(), 1, "longer match should be preferred");
        assert_eq!(result[0].text, "กรุงเทพ");
    }

    #[test]
    fn tag_tokens_multi_does_not_cross_non_thai() {
        use crate::token::Token;
        // "กรุง100เทพ" — Number token between Thai tokens; should NOT merge.
        // กรุง=12 bytes, 100=3 bytes (ASCII), เทพ=9 bytes
        let source = "กรุง100เทพ";
        let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
        let tokens = alloc::vec![
            Token::new("กรุง", 0..12, 0..4, TokenKind::Thai, 1.0),
            Token::new("100", 12..15, 4..7, TokenKind::Number, 1.0),
            Token::new("เทพ", 15..24, 7..10, TokenKind::Thai, 1.0),
        ];
        let result = tagger.tag_tokens(tokens, source);
        assert!(
            result
                .iter()
                .all(|t| t.kind != TokenKind::Named(NamedEntityKind::Place)),
            "no token should become Named when non-Thai sits between them"
        );
        assert_eq!(
            result.len(),
            3,
            "tokens should not merge across Number boundary"
        );
    }

    #[test]
    fn tag_tokens_multi_prefix_context() {
        use crate::token::Token;
        // "ไปกรุงเทพ" → [ไป, กรุง, เทพ]; only กรุงเทพ is in gazetteer.
        // ไป=6 bytes (2 chars), กรุง=12 bytes (4 chars), เทพ=9 bytes (3 chars)
        let source = "ไปกรุงเทพ";
        let tagger = NeTagger::from_tsv("กรุงเทพ\tPLACE\n");
        let tokens = alloc::vec![
            Token::new("ไป", 0..6, 0..2, TokenKind::Thai, 1.0),
            Token::new("กรุง", 6..18, 2..6, TokenKind::Thai, 1.0),
            Token::new("เทพ", 18..27, 6..9, TokenKind::Thai, 1.0),
        ];
        let result = tagger.tag_tokens(tokens, source);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].kind, TokenKind::Thai);
        assert_eq!(result[0].text, "ไป");
        assert_eq!(result[1].kind, TokenKind::Named(NamedEntityKind::Place));
        assert_eq!(result[1].text, "กรุงเทพ");
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
