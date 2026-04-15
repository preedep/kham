//! DAG-based maximal matching segmenter (newmm algorithm).
//!
//! The segmenter builds a Directed Acyclic Word Graph (DAWG) over the input
//! text using TCC boundaries as candidate split points, then finds the path
//! that maximises the number of dictionary matches (fewest unknown tokens).
//!
//! ## Pipeline
//!
//! ```text
//! raw text
//!   │
//!   ▼  (optional) Tokenizer::normalize()   ← call this first for malformed input
//!   │
//!   ▼  pre_tokenize()
//! [Thai span] [Number span] [Latin span] …
//!   │
//!   ▼  (Thai spans only) tcc_boundaries()
//! TCC boundary positions: [0, b1, b2, …, len]
//!   │
//!   ▼  DP over boundary indices
//! path of (start, end) pairs that maximises dict matches
//!   │
//!   ▼
//! Vec<Token<'_>>
//! ```
//!
//! ## Normalization and zero-copy
//!
//! [`Tokenizer::segment`] is zero-copy: every [`Token`] borrows directly from
//! the `&str` you pass in. This means segment() cannot internally normalize
//! the text (normalization may reorder/remove characters, producing a new
//! allocation with different byte offsets).
//!
//! For input that may contain สระลอย in wrong order, stacked tone marks, or
//! decomposed Sara Am, use the two-step pattern:
//!
//! ```rust
//! use kham_core::Tokenizer;
//!
//! let tok = Tokenizer::new();
//! let normalized = tok.normalize("กเินข้าว"); // fix any encoding issues
//! let tokens = tok.segment(&normalized);       // tokens borrow `normalized`
//! ```

use alloc::vec::Vec;
use alloc::vec;

use crate::dict::{Dict, BUILTIN_WORDS};
use crate::error::KhamError;
use crate::normalizer;
use crate::pre_tokenizer::pre_tokenize;
use crate::tcc::tcc_boundaries;
use crate::token::{Token, TokenKind};

/// High-level tokenizer. Holds a compiled dictionary and segmentation options.
///
/// # Example
///
/// ```rust
/// use kham_core::Tokenizer;
///
/// let tok = Tokenizer::new();
/// let tokens = tok.segment("กินข้าวกับปลา");
/// assert!(!tokens.is_empty());
/// ```
pub struct Tokenizer {
    dict: Dict,
    keep_whitespace: bool,
}

impl Tokenizer {
    /// Create a tokenizer with the built-in dictionary.
    pub fn new() -> Self {
        Self {
            dict: Dict::from_word_list(BUILTIN_WORDS),
            keep_whitespace: false,
        }
    }

    /// Normalise Thai text into canonical form.
    ///
    /// This is a convenience wrapper around [`normalizer::normalize`].
    /// Because [`segment`] is zero-copy, normalization must happen **before**
    /// segmentation. The caller owns the returned [`String`] and can then
    /// borrow it for [`segment`]:
    ///
    /// ```rust
    /// use kham_core::Tokenizer;
    ///
    /// let tok = Tokenizer::new();
    /// // Input with สระลอย in wrong order and a doubled tone mark
    /// let raw = "\u{0E01}\u{0E40}\u{0E19}\u{0E48}\u{0E49}"; // กเน + อ่อ้
    /// let normalized = tok.normalize(raw);
    /// let tokens = tok.segment(&normalized); // tokens borrow `normalized`
    /// assert!(!tokens.is_empty());
    /// ```
    ///
    /// [`segment`]: Tokenizer::segment
    pub fn normalize(&self, text: &str) -> alloc::string::String {
        normalizer::normalize(text)
    }

    /// Return a [`TokenizerBuilder`] for custom configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::Tokenizer;
    ///
    /// // Use built-in dict (no extra words needed here)
    /// let tok = Tokenizer::builder().build();
    /// let tokens = tok.segment("สวัสดีชาวโลก");
    /// assert!(!tokens.is_empty());
    /// ```
    pub fn builder() -> TokenizerBuilder {
        TokenizerBuilder::default()
    }

    /// Segment `text` into tokens.
    ///
    /// Returns a `Vec<Token<'_>>` where every token's `text` is a
    /// zero-copy sub-slice of `text`.
    ///
    /// Non-Thai spans (Latin, Number, Whitespace, Emoji, Punctuation) pass
    /// through unchanged. Thai spans are segmented with the newmm DAG
    /// algorithm constrained to TCC boundaries.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::{Tokenizer, TokenKind};
    ///
    /// let tok = Tokenizer::new();
    /// // Mixed Thai + number + Thai
    /// let tokens = tok.segment("ธนาคาร100แห่ง");
    /// assert_eq!(tokens[1].text, "100");
    /// assert_eq!(tokens[1].kind, TokenKind::Number);
    /// ```
    pub fn segment<'t>(&self, text: &'t str) -> Vec<Token<'t>> {
        if text.is_empty() {
            return Vec::new();
        }

        // Split into script-homogeneous spans. Non-Thai spans pass through;
        // Thai spans go through the newmm DAG segmenter.
        // Call normalize() first if the input may contain สระลอย in wrong
        // order, stacked tone marks, or decomposed Sara Am.
        let pre_tokens = pre_tokenize(text);

        let mut result: Vec<Token<'t>> = Vec::with_capacity(pre_tokens.len() * 2);

        for token in pre_tokens {
            match token.kind {
                TokenKind::Thai => {
                    segment_thai(&self.dict, text, token.span, &mut result);
                }
                TokenKind::Whitespace if !self.keep_whitespace => {
                    // Discard whitespace tokens unless keep_whitespace is set.
                }
                _ => {
                    result.push(token);
                }
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// newmm DAG segmentation — Thai spans only
// ---------------------------------------------------------------------------

/// Segment a single Thai span (identified by its byte range in `text`) using
/// the newmm DAG algorithm and append the resulting tokens to `out`.
///
/// ## Algorithm
///
/// 1. Compute TCC boundaries for the span — these are the only positions
///    where a word boundary may legally fall.
/// 2. Run forward DP over boundary indices. At each index `i`:
///    a. Enumerate all dictionary prefixes of `slice[bounds[i]..]`.
///    b. For each prefix that ends exactly on a TCC boundary `j`, record
///       an edge `i → j` (dictionary match, score +1).
///    c. Always record a fallback edge `i → i+1` (one TCC, unknown token,
///       score +0).
///    d. Ties in dict-word count are broken by preferring fewer total
///       tokens (i.e. longer individual words).
/// 3. Backtrack from the last boundary to reconstruct the winning path.
/// 4. Emit a `Token` for each edge, with `TokenKind::Thai` for dictionary
///    matches and `TokenKind::Unknown` for unknown single-TCC segments.
fn segment_thai<'t>(
    dict: &Dict,
    text: &'t str,
    span: core::ops::Range<usize>,
    out: &mut Vec<Token<'t>>,
) {
    let slice = &text[span.start..span.end];

    // TCC boundaries relative to `slice` (always starts with 0, ends with
    // slice.len(), has at least 2 elements for non-empty input).
    let bounds = tcc_boundaries(slice);
    let nb = bounds.len();

    if nb <= 1 {
        // Empty span — nothing to emit.
        return;
    }

    // DP arrays, indexed by boundary position index.
    //
    // `dp[i]` = (dict_word_count, neg_token_count) — the best score reachable
    // at boundary index `i`. We maximise lexicographically, so:
    //   • dict_word_count is the primary objective (more is better).
    //   • neg_token_count breaks ties: –k means k tokens total, so a less
    //     negative value (fewer tokens, i.e. longer words) is preferred.
    //
    // Sentinel: `dp[i].0 == i32::MIN` means index `i` is not yet reachable.
    const UNREACHABLE: (i32, i32) = (i32::MIN, 0);
    let mut dp: Vec<(i32, i32)> = vec![UNREACHABLE; nb];
    let mut from: Vec<usize> = vec![0; nb];
    let mut edge_is_dict: Vec<bool> = vec![false; nb];

    dp[0] = (0, 0);

    for i in 0..nb - 1 {
        let score = dp[i];
        if score == UNREACHABLE {
            continue;
        }
        let (dw, nt) = score;
        let pos = bounds[i];
        let remaining = &slice[pos..];

        // --- dictionary edges ---
        // `dict.prefixes()` returns matches longest-first; we iterate all of
        // them so the DP can choose globally rather than greedily.
        for prefix in dict.prefixes(remaining) {
            let end_pos = pos + prefix.len();
            // Only accept matches that land exactly on a TCC boundary.
            if let Ok(j) = bounds.binary_search(&end_pos) {
                let candidate = (dw + 1, nt - 1);
                if candidate > dp[j] {
                    dp[j] = candidate;
                    from[j] = i;
                    edge_is_dict[j] = true;
                }
            }
        }

        // --- fallback edge: advance one TCC (unknown token) ---
        let j = i + 1;
        let candidate = (dw, nt - 1);
        if candidate > dp[j] {
            dp[j] = candidate;
            from[j] = i;
            edge_is_dict[j] = false;
        }
    }

    // Backtrack from the last boundary to reconstruct the winning path.
    let mut path: Vec<usize> = Vec::with_capacity(nb);
    let mut cur = nb - 1;
    loop {
        path.push(cur);
        if cur == 0 {
            break;
        }
        cur = from[cur];
    }
    path.reverse();

    // Emit one token per edge in the path.
    for w in path.windows(2) {
        let start_byte = span.start + bounds[w[0]];
        let end_byte = span.start + bounds[w[1]];
        let kind = if edge_is_dict[w[1]] {
            TokenKind::Thai
        } else {
            TokenKind::Unknown
        };
        out.push(Token::new(&text[start_byte..end_byte], start_byte..end_byte, kind));
    }
}

// ---------------------------------------------------------------------------
// Tokenizer trait impls
// ---------------------------------------------------------------------------

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TokenizerBuilder
// ---------------------------------------------------------------------------

/// Builder for [`Tokenizer`].
///
/// # Example
///
/// ```rust
/// use kham_core::Tokenizer;
///
/// let tok = Tokenizer::builder()
///     .keep_whitespace(true)
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct TokenizerBuilder {
    dict_words: Option<alloc::string::String>,
    keep_whitespace: bool,
}

impl TokenizerBuilder {
    /// Load an additional word list from a string (newline-separated words).
    ///
    /// Words are merged with the built-in dictionary.
    pub fn dict_words(mut self, words: &str) -> Self {
        self.dict_words = Some(alloc::string::String::from(words));
        self
    }

    /// Configure whether whitespace tokens are included in the output.
    ///
    /// Default: `false` (whitespace is discarded).
    pub fn keep_whitespace(mut self, keep: bool) -> Self {
        self.keep_whitespace = keep;
        self
    }

    /// Consume the builder and return a configured [`Tokenizer`].
    pub fn build(self) -> Tokenizer {
        let base = BUILTIN_WORDS;
        let dict = if let Some(extra) = &self.dict_words {
            let mut combined = alloc::string::String::from(base);
            combined.push('\n');
            combined.push_str(extra);
            Dict::from_word_list(&combined)
        } else {
            Dict::from_word_list(base)
        };
        Tokenizer { dict, keep_whitespace: self.keep_whitespace }
    }

    /// Try to load a custom word list from a file path.
    ///
    /// Only available when the `std` feature is enabled.
    ///
    /// # Errors
    ///
    /// Returns [`KhamError::DictLoadError`] if the file cannot be read.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use kham_core::Tokenizer;
    ///
    /// let tok = Tokenizer::builder()
    ///     .dict_file("my_words.txt")
    ///     .expect("failed to load dict")
    ///     .build();
    /// ```
    #[cfg(feature = "std")]
    pub fn dict_file(self, path: &str) -> Result<Self, KhamError> {
        extern crate std;
        let content = std::fs::read_to_string(path)
            .map_err(|e| KhamError::DictLoadError(alloc::format!("{path}: {e}")))?;
        Ok(self.dict_words(&content))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tok() -> Tokenizer {
        Tokenizer::new()
    }

    // ── basic smoke tests ────────────────────────────────────────────────────

    #[test]
    fn empty_input() {
        assert!(tok().segment("").is_empty());
    }

    #[test]
    fn pure_latin_passthrough() {
        let tokens = tok().segment("hello");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "hello");
        assert_eq!(tokens[0].kind, TokenKind::Latin);
    }

    #[test]
    fn pure_number_passthrough() {
        let tokens = tok().segment("12345");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "12345");
        assert_eq!(tokens[0].kind, TokenKind::Number);
    }

    #[test]
    fn whitespace_dropped_by_default() {
        let tokens = tok().segment("กิน ข้าว");
        for t in &tokens {
            assert_ne!(t.kind, TokenKind::Whitespace);
        }
    }

    #[test]
    fn whitespace_kept_when_requested() {
        let tokens = Tokenizer::builder()
            .keep_whitespace(true)
            .build()
            .segment("กิน ข้าว");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Whitespace));
    }

    // ── Thai segmentation ────────────────────────────────────────────────────

    #[test]
    fn gin_khao_gap_pla() {
        // "กินข้าวกับปลา" — all words must be in the built-in dict
        let tokens = tok().segment("กินข้าวกับปลา");
        let words: Vec<&str> = tokens.iter().map(|t| t.text).collect();
        // Must segment into at least 2 tokens (dict has กิน, ข้าว, กับ, ปลา)
        assert!(words.len() >= 2, "expected multiple words, got {words:?}");
        // Reconstructing must yield the original string
        assert_eq!(words.join(""), "กินข้าวกับปลา");
    }

    #[test]
    fn mixed_thai_number_thai() {
        // Classic CLAUDE.md example
        let tokens = tok().segment("ธนาคาร100แห่ง");
        let rebuilt: alloc::string::String = tokens.iter().map(|t| t.text).collect();
        assert_eq!(rebuilt, "ธนาคาร100แห่ง");
        // "100" must survive as a Number token
        let num = tokens.iter().find(|t| t.kind == TokenKind::Number);
        assert!(num.is_some());
        assert_eq!(num.unwrap().text, "100");
    }

    #[test]
    fn mixed_thai_latin() {
        let tokens = tok().segment("สวัสดี hello");
        let rebuilt: alloc::string::String = tokens
            .iter()
            .map(|t| t.text)
            .collect();
        // Whitespace dropped by default
        assert_eq!(rebuilt, "สวัสดีhello");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Latin && t.text == "hello"));
    }

    // ── span / byte-offset invariants ────────────────────────────────────────

    #[test]
    fn spans_cover_input_excluding_whitespace() {
        let text = "กินข้าว123hello";
        let tokens = tok().segment(text);
        // Every span must be a valid UTF-8 slice of `text`.
        for t in &tokens {
            assert_eq!(&text[t.span.clone()], t.text);
            assert!(text.is_char_boundary(t.span.start));
            assert!(text.is_char_boundary(t.span.end));
        }
    }

    #[test]
    fn adjacent_spans_are_contiguous() {
        let text = "กินข้าวกับปลา";
        let tokens = Tokenizer::builder()
            .keep_whitespace(true)
            .build()
            .segment(text);
        for w in tokens.windows(2) {
            assert_eq!(
                w[0].span.end,
                w[1].span.start,
                "gap between {:?} and {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn no_empty_tokens() {
        let tokens = tok().segment("กินข้าวกับปลา 100 hello!");
        for t in &tokens {
            assert!(!t.text.is_empty());
        }
    }

    // ── custom dictionary ─────────────────────────────────────────────────────

    #[test]
    fn custom_dict_word_is_matched() {
        let tok = Tokenizer::builder()
            .dict_words("มะม่วงหิมพานต์\n")
            .build();
        let tokens = tok.segment("มะม่วงหิมพานต์");
        // The whole compound should be one Thai token
        let thai: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Thai)
            .map(|t| t.text)
            .collect();
        assert!(thai.contains(&"มะม่วงหิมพานต์"), "got: {thai:?}");
    }

    // ── normalize then segment ────────────────────────────────────────────────

    #[test]
    fn normalize_fixes_lead_vowel_order_before_segment() {
        // กเ (wrong: consonant before lead vowel) should become เก after normalize
        // so segmenter sees correct Thai text.
        let t = tok();
        let raw = "\u{0E01}\u{0E40}"; // ก + เ (wrong order)
        let normalized = t.normalize(raw);
        assert_eq!(normalized, "\u{0E40}\u{0E01}"); // เก
        let tokens = t.segment(&normalized);
        let rebuilt: alloc::string::String = tokens.iter().map(|t| t.text).collect();
        assert_eq!(rebuilt, normalized);
    }

    #[test]
    fn normalize_deduplicates_tone_before_segment() {
        // กินข้าว with a doubled tone mark on ข้ — normalize fixes it, segment proceeds.
        let t = tok();
        // Insert a doubled tone on ข: ข + อ้ + อ้  (ข้้)
        let raw = "กิน\u{0E02}\u{0E49}\u{0E49}าว"; // กิน + ข้้ + าว
        let normalized = t.normalize(raw);
        let tokens = t.segment(&normalized);
        assert!(!tokens.is_empty());
        let rebuilt: alloc::string::String = tokens.iter().map(|t| t.text).collect();
        assert_eq!(rebuilt, normalized);
    }

    #[test]
    fn normalize_clean_input_is_identity() {
        // normalize() on already-clean text should not change it.
        let t = tok();
        let clean = "กินข้าวกับปลา";
        assert_eq!(t.normalize(clean), clean);
    }

    #[test]
    fn segment_without_normalize_on_clean_input() {
        // segment() alone is sufficient when input is already canonical.
        let tokens = tok().segment("กินข้าวกับปลา");
        let rebuilt: alloc::string::String = tokens.iter().map(|t| t.text).collect();
        assert_eq!(rebuilt, "กินข้าวกับปลา");
    }

    // ── edge cases ────────────────────────────────────────────────────────────

    #[test]
    fn single_thai_char() {
        let tokens = tok().segment("ก");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "ก");
    }

    #[test]
    fn sawasdee_khao_lok() {
        let tokens = tok().segment("สวัสดีชาวโลก");
        let rebuilt: alloc::string::String = tokens.iter().map(|t| t.text).collect();
        assert_eq!(rebuilt, "สวัสดีชาวโลก");
    }
}
