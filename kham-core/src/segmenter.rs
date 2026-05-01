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
//!   ▼  (optional) Tokenizer::normalize()   ← fixes tone dedup + Sara Am composition
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

use alloc::vec;
use alloc::vec::Vec;

use crate::dict::{builtin_dict, Dict, BUILTIN_WORDS};
use crate::error::KhamError;
use crate::freq::FreqMap;
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
    freq: FreqMap,
    keep_whitespace: bool,
}

impl Tokenizer {
    /// Create a tokenizer with the built-in dictionary and TNC frequency table.
    pub fn new() -> Self {
        Self {
            dict: builtin_dict(),
            freq: FreqMap::builtin(),
            keep_whitespace: false,
        }
    }

    /// Normalise Thai text into canonical form.
    ///
    /// This is a convenience wrapper around [`normalizer::normalize`].
    /// Because [`segment`] is zero-copy, normalization must happen **before**
    /// segmentation. The caller owns the returned [`alloc::string::String`] and can then
    /// borrow it for [`segment`]:
    ///
    /// ```rust
    /// use kham_core::Tokenizer;
    ///
    /// let tok = Tokenizer::new();
    /// // Input with a doubled tone mark and decomposed Sara Am
    /// let raw = "\u{0E01}\u{0E34}\u{0E19}\u{0E19}\u{0E49}\u{0E4D}\u{0E32}"; // กิน + น + ้ + อํ + อา
    /// let normalized = tok.normalize(raw); // น้ำ composed, no dedup needed here
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
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::{Tokenizer, TokenKind};
    ///
    /// let tok = Tokenizer::new();
    /// // Mixed Thai + number + Thai — number token lands at index 1
    /// let tokens = tok.segment("ธนาคาร100แห่ง");
    /// assert_eq!(tokens[1].text, "100");
    /// assert_eq!(tokens[1].kind, TokenKind::Number);
    /// ```
    ///
    /// Joining all token texts reconstructs the original string (whitespace
    /// is dropped by default, so the joined result omits whitespace):
    ///
    /// ```rust
    /// use kham_core::Tokenizer;
    ///
    /// let tok = Tokenizer::new();
    /// let text = "กินข้าวกับปลา";
    /// let tokens = tok.segment(text);
    /// let rebuilt: String = tokens.iter().map(|t| t.text).collect();
    /// assert_eq!(rebuilt, text);
    /// ```
    ///
    /// Every token carries both byte and char offsets into the original string:
    ///
    /// ```rust
    /// use kham_core::Tokenizer;
    ///
    /// let tok = Tokenizer::new();
    /// let text = "ธนาคาร100แห่ง";
    /// let tokens = tok.segment(text);
    /// for t in &tokens {
    ///     // Byte span: valid UTF-8 slice
    ///     assert_eq!(&text[t.span.clone()], t.text);
    ///     // Char span: length matches Unicode scalar count
    ///     assert_eq!(t.char_span.end - t.char_span.start, t.text.chars().count());
    /// }
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
                    segment_thai(&self.dict, &self.freq, text, token.span, &mut result);
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

/// Lexicographic DP score for a TCC boundary position.
///
/// Fields are ordered so that `Ord` naturally expresses the newmm preference:
/// 1. Minimise unknowns (fewer unknowns → `neg_unknowns` less negative → greater).
/// 2. Minimise total token count (prefer longer compounds over split components).
/// 3. Maximise dictionary matches.
/// 4. Maximise cumulative TNC frequency as the final tiebreaker.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DpScore {
    neg_unknowns: i32,
    neg_tokens: i32,
    dict_words: i32,
    freq_score: u64,
}

impl DpScore {
    const ZERO: Self = Self {
        neg_unknowns: 0,
        dict_words: 0,
        freq_score: 0,
        neg_tokens: 0,
    };

    fn dict_edge(self, freq: u32) -> Self {
        Self {
            dict_words: self.dict_words + 1,
            freq_score: self.freq_score + freq as u64,
            neg_tokens: self.neg_tokens - 1,
            ..self
        }
    }

    fn unknown_edge(self) -> Self {
        Self {
            neg_unknowns: self.neg_unknowns - 1,
            neg_tokens: self.neg_tokens - 1,
            ..self
        }
    }
}

/// Output of the forward DP pass.
struct DpTable {
    /// Predecessor boundary index for backtracking.
    from: Vec<usize>,
    /// Whether the incoming edge at index `i` was a dictionary match.
    is_dict: Vec<bool>,
}

/// Forward DP over TCC boundary indices for a single Thai slice.
///
/// `bounds` must be the output of [`tcc_boundaries`] for `slice`.
fn forward_dp(dict: &Dict, freqs: &FreqMap, slice: &str, bounds: &[usize]) -> DpTable {
    let nb = bounds.len();
    let mut best: Vec<Option<DpScore>> = vec![None; nb];
    let mut from = vec![0usize; nb];
    let mut is_dict = vec![false; nb];

    best[0] = Some(DpScore::ZERO);

    for i in 0..nb - 1 {
        let score = match best[i] {
            Some(s) => s,
            None => continue,
        };
        let pos = bounds[i];
        let remaining = &slice[pos..];

        // Dictionary edges — all prefixes, not just the longest, so the DP
        // can make a globally optimal choice rather than a greedy one.
        for prefix in dict.prefixes(remaining) {
            let end_pos = pos + prefix.len();
            if let Ok(j) = bounds.binary_search(&end_pos) {
                let freq = freqs.get(prefix);
                let candidate = Some(score.dict_edge(freq));
                if candidate > best[j] {
                    best[j] = candidate;
                    from[j] = i;
                    is_dict[j] = true;
                }
            }
        }

        // Fallback edge: advance one TCC as an unknown token.
        let j = i + 1;
        let candidate = Some(score.unknown_edge());
        if candidate > best[j] {
            best[j] = candidate;
            from[j] = i;
            is_dict[j] = false;
        }
    }

    DpTable { from, is_dict }
}

/// Reconstruct the winning boundary-index path by following `from` pointers
/// from the last index back to 0, then reversing.
fn backtrack_path(from: &[usize]) -> Vec<usize> {
    let nb = from.len();
    let mut path = Vec::with_capacity(nb);
    let mut cur = nb - 1;
    loop {
        path.push(cur);
        if cur == 0 {
            break;
        }
        cur = from[cur];
    }
    path.reverse();
    path
}

/// Segment a single Thai span using the newmm DAG algorithm and append tokens
/// to `out`.
///
/// Steps: TCC boundaries → forward DP → backtrack → emit tokens.
fn segment_thai<'t>(
    dict: &Dict,
    freqs: &FreqMap,
    text: &'t str,
    span: core::ops::Range<usize>,
    out: &mut Vec<Token<'t>>,
) {
    let slice = &text[span.start..span.end];
    let bounds = tcc_boundaries(slice);

    if bounds.len() <= 1 {
        return;
    }

    let dp = forward_dp(dict, freqs, slice, &bounds);
    let path = backtrack_path(&dp.from);

    // Char offset of span.start — computed once, then incremented per token.
    let mut char_cursor = text[..span.start].chars().count();

    for w in path.windows(2) {
        let start_byte = span.start + bounds[w[0]];
        let end_byte = span.start + bounds[w[1]];
        let token_text = &text[start_byte..end_byte];
        let char_start = char_cursor;
        char_cursor += token_text.chars().count();
        let kind = if dp.is_dict[w[1]] {
            TokenKind::Thai
        } else {
            TokenKind::Unknown
        };
        out.push(Token::new(
            token_text,
            start_byte..end_byte,
            char_start..char_cursor,
            kind,
        ));
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
    dict_merge: Option<alloc::string::String>,
    keep_whitespace: bool,
}

impl TokenizerBuilder {
    /// Load an additional word list from a string (newline-separated words).
    ///
    /// Words are merged with the built-in dictionary.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::{Tokenizer, TokenKind};
    ///
    /// let tok = Tokenizer::builder()
    ///     .dict_words("ปัญญาประดิษฐ์\n")
    ///     .build();
    /// let tokens = tok.segment("ปัญญาประดิษฐ์คือ");
    /// assert!(tokens.iter().any(|t| t.text == "ปัญญาประดิษฐ์" && t.kind == TokenKind::Thai));
    /// ```
    pub fn dict_words(mut self, words: &str) -> Self {
        self.dict_words = Some(alloc::string::String::from(words));
        self
    }

    /// Configure whether whitespace tokens are included in the output.
    ///
    /// Default: `false` (whitespace is discarded).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::{Tokenizer, TokenKind};
    ///
    /// let tok = Tokenizer::builder().keep_whitespace(true).build();
    /// let tokens = tok.segment("กิน ข้าว");
    /// assert!(tokens.iter().any(|t| t.kind == TokenKind::Whitespace));
    /// // Byte spans are contiguous when whitespace is kept
    /// for w in tokens.windows(2) {
    ///     assert_eq!(w[0].span.end, w[1].span.start);
    /// }
    /// ```
    /// Add extra words via a lightweight overlay — no trie rebuild.
    ///
    /// Words are stored in a sorted list alongside the pre-compiled trie.
    /// This is O(k log k) in the number of custom words and avoids the O(N)
    /// full trie rebuild that [`dict_words`](Self::dict_words) performs.
    ///
    /// Prefer `dict_merge` over `dict_words` when adding a small custom
    /// vocabulary (e.g. domain-specific terms, product names).
    ///
    /// If both `dict_merge` and `dict_words` are called, `dict_words` takes
    /// precedence (it performs a full rebuild that subsumes any overlay).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::{Tokenizer, TokenKind};
    ///
    /// let tok = Tokenizer::builder()
    ///     .dict_merge("ปัญญาประดิษฐ์\nโปรแกรมเมอร์\n")
    ///     .build();
    /// let tokens = tok.segment("ปัญญาประดิษฐ์คือ");
    /// assert!(tokens.iter().any(|t| t.text == "ปัญญาประดิษฐ์" && t.kind == TokenKind::Thai));
    /// ```
    pub fn dict_merge(mut self, words: &str) -> Self {
        self.dict_merge = Some(alloc::string::String::from(words));
        self
    }

    /// Configure whether whitespace tokens are included in the output.
    ///
    /// Default: `false` (whitespace is discarded).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::{Tokenizer, TokenKind};
    ///
    /// let tok = Tokenizer::builder().keep_whitespace(true).build();
    /// let tokens = tok.segment("กิน ข้าว");
    /// assert!(tokens.iter().any(|t| t.kind == TokenKind::Whitespace));
    /// // Byte spans are contiguous when whitespace is kept
    /// for w in tokens.windows(2) {
    ///     assert_eq!(w[0].span.end, w[1].span.start);
    /// }
    /// ```
    pub fn keep_whitespace(mut self, keep: bool) -> Self {
        self.keep_whitespace = keep;
        self
    }

    /// Consume the builder and return a configured [`Tokenizer`].
    pub fn build(self) -> Tokenizer {
        let dict = if let Some(extra) = &self.dict_words {
            // Full rebuild path: merges BUILTIN_WORDS + custom words into a new trie.
            let mut combined = alloc::string::String::from(BUILTIN_WORDS);
            combined.push('\n');
            combined.push_str(extra);
            Dict::from_word_list(&combined)
        } else if let Some(overlay) = &self.dict_merge {
            // Fast overlay path: load pre-compiled binary, attach small sorted list.
            builtin_dict().with_overlay(overlay)
        } else {
            // Default path: load from pre-compiled binary — O(S) copy.
            builtin_dict()
        };
        Tokenizer {
            dict,
            freq: FreqMap::builtin(),
            keep_whitespace: self.keep_whitespace,
        }
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
        let rebuilt: alloc::string::String = tokens.iter().map(|t| t.text).collect();
        // Whitespace dropped by default
        assert_eq!(rebuilt, "สวัสดีhello");
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Latin && t.text == "hello"));
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
                w[0].span.end, w[1].span.start,
                "gap between {:?} and {:?}",
                w[0], w[1]
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
        // Use a nonsense word that is not in the built-in dictionary and cannot
        // be decomposed into subwords — ensures the custom dict is actually used.
        let tok = Tokenizer::builder().dict_words("กขคงจฉ\n").build();
        let tokens = tok.segment("กขคงจฉ");
        let thai: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Thai)
            .map(|t| t.text)
            .collect();
        assert!(thai.contains(&"กขคงจฉ"), "got: {thai:?}");
    }

    // ── normalize then segment ────────────────────────────────────────────────

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

    // ── DpScore ordering ──────────────────────────────────────────────────────
    //
    // The score is a 4-field lexicographic key:
    //   1. neg_unknowns  — fewer unknowns is strictly better
    //   2. neg_tokens    — fewer tokens (prefer longer compounds over split components)
    //   3. dict_words    — more dictionary matches breaks token-count ties
    //   4. freq_score    — higher cumulative TNC frequency as the final tiebreaker

    #[test]
    fn dp_score_fewer_unknowns_is_primary() {
        // A path with no unknowns beats one with unknowns regardless of other fields.
        let no_unknown = DpScore::ZERO;
        let one_unknown = DpScore::ZERO.unknown_edge();
        assert!(no_unknown > one_unknown);
    }

    #[test]
    fn dp_score_fewer_tokens_beats_more_dict_words() {
        // Fewer tokens wins over more dict matches: เดินทาง (1 token, 1 match)
        // beats เดิน+ทาง (2 tokens, 2 matches).
        let compound = DpScore::ZERO.dict_edge(0); // 1 token, 1 dict
        let split = DpScore::ZERO.dict_edge(0).dict_edge(0); // 2 tokens, 2 dict
        assert!(compound > split);
    }

    #[test]
    fn dp_score_higher_freq_breaks_token_tie() {
        // Same unknowns and token count; higher TNC freq wins.
        let low_freq = DpScore::ZERO.dict_edge(10);
        let high_freq = DpScore::ZERO.dict_edge(100);
        assert!(high_freq > low_freq);
    }

    #[test]
    fn dp_score_fewer_tokens_beats_higher_freq() {
        // Fewer tokens wins even when the competing path has higher TNC frequency.
        let high_freq_more_tokens = DpScore {
            neg_unknowns: 0,
            neg_tokens: -2,
            dict_words: 1,
            freq_score: 200,
        };
        let low_freq_fewer_tokens = DpScore {
            neg_unknowns: 0,
            neg_tokens: -1,
            dict_words: 1,
            freq_score: 100,
        };
        assert!(low_freq_fewer_tokens > high_freq_more_tokens);
    }

    #[test]
    fn dp_score_more_dict_words_breaks_token_tie() {
        // Same unknowns and token count; more dict matches wins.
        let fewer_dict = DpScore {
            neg_unknowns: 0,
            neg_tokens: -2,
            dict_words: 1,
            freq_score: 0,
        };
        let more_dict = DpScore {
            neg_unknowns: 0,
            neg_tokens: -2,
            dict_words: 2,
            freq_score: 0,
        };
        assert!(more_dict > fewer_dict);
    }

    #[test]
    fn dict_edge_accumulates_freq_score() {
        let after_one = DpScore::ZERO.dict_edge(50);
        let after_two = after_one.dict_edge(30);
        assert_eq!(after_one.freq_score, 50);
        assert_eq!(after_two.freq_score, 80);
    }

    #[test]
    fn dict_edge_increments_dict_words_and_neg_tokens() {
        let s = DpScore::ZERO.dict_edge(0);
        assert_eq!(s.dict_words, 1);
        assert_eq!(s.neg_tokens, -1);
        assert_eq!(s.neg_unknowns, 0);
    }

    #[test]
    fn unknown_edge_increments_neg_unknowns_only() {
        let s = DpScore::ZERO.unknown_edge();
        assert_eq!(s.neg_unknowns, -1);
        assert_eq!(s.neg_tokens, -1);
        assert_eq!(s.dict_words, 0);
        assert_eq!(s.freq_score, 0);
    }

    #[test]
    fn unknown_edge_does_not_contribute_freq() {
        let s = DpScore::ZERO.unknown_edge().unknown_edge();
        assert_eq!(s.freq_score, 0);
    }

    // ── char_span invariants ──────────────────────────────────────────────────

    #[test]
    fn char_span_len_equals_char_count() {
        let tokens = tok().segment("กินข้าวกับปลา");
        for t in &tokens {
            assert_eq!(
                t.char_span.end - t.char_span.start,
                t.text.chars().count(),
                "char_span length mismatch for {:?}",
                t.text
            );
        }
    }

    #[test]
    fn char_spans_are_contiguous() {
        let tokens = Tokenizer::builder()
            .keep_whitespace(true)
            .build()
            .segment("กินข้าว 100 hello");
        for w in tokens.windows(2) {
            assert_eq!(
                w[0].char_span.end, w[1].char_span.start,
                "char_span gap between {:?} and {:?}",
                w[0].text, w[1].text
            );
        }
    }

    #[test]
    fn char_span_for_mixed_script() {
        // "ธนาคาร100แห่ง": ธนาคาร=6 chars, 100=3 chars, แห่ง=4 chars
        let tokens = tok().segment("ธนาคาร100แห่ง");
        assert_eq!(tokens[0].char_span, 0..6);
        assert_eq!(tokens[1].char_span, 6..9);
        assert_eq!(tokens[2].char_span, 9..13);
    }

    #[test]
    fn char_span_accounts_for_multibyte_chars() {
        // Each Thai codepoint is 3 bytes but 1 char.
        // "กิน" = 3 chars (9 bytes); char_span should be 0..3, span 0..9.
        let tokens = tok().segment("กิน");
        assert_eq!(tokens[0].span, 0..9);
        assert_eq!(tokens[0].char_span, 0..3);
    }

    #[test]
    fn char_span_emoji_is_single_char() {
        // 😀 = 1 char, 4 bytes — verify char_span counts it as 1.
        let tokens = tok().segment("😀");
        assert_eq!(tokens[0].char_len(), 1);
        assert_eq!(tokens[0].byte_len(), 4);
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
