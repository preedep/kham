//! Character-level and token-level n-gram generation for Thai FTS.
//!
//! N-grams serve two roles in Thai full-text search:
//!
//! 1. **Character trigrams** — for out-of-vocabulary (OOV) words that the
//!    segmenter emits as [`TokenKind::Unknown`], generate character n-grams so
//!    they remain searchable via approximate matching.
//!
//! 2. **Token bigrams / trigrams** — for phrase-proximity queries, consecutive
//!    token pairs/triples are indexed alongside individual lexemes.
//!
//! # Example
//!
//! ```rust
//! use kham_core::ngram::{char_ngrams, token_ngrams};
//!
//! // Character trigrams of an unknown word
//! let grams: Vec<&str> = char_ngrams("สวัสดี", 2).collect();
//! assert!(grams.contains(&"สว"));
//!
//! // Token bigrams
//! let tokens = &["กิน", "ข้าว", "กับ", "ปลา"];
//! let bigrams: Vec<_> = token_ngrams(tokens, 2).collect();
//! assert!(bigrams.iter().any(|g| g == "กินข้าว"));
//! ```

use alloc::string::String;
use alloc::vec::Vec;

/// Iterate over character-level n-grams of `text`.
///
/// Each yielded `&str` is a slice of `text` containing exactly `n` Unicode
/// scalar values (chars). An empty iterator is returned if `text` has fewer
/// than `n` chars, or if `n == 0`.
///
/// Thai characters are multi-byte in UTF-8 (3 bytes each), so slicing is done
/// via char boundary tracking rather than byte arithmetic.
pub fn char_ngrams(text: &str, n: usize) -> impl Iterator<Item = &str> {
    CharNgramIter::new(text, n)
}

/// Iterate over token-level n-grams by concatenating consecutive token strings.
///
/// Yields owned [`String`]s because concatenation requires allocation.
/// An empty iterator is returned when `tokens.len() < n` or `n == 0`.
pub fn token_ngrams<'a>(tokens: &'a [&'a str], n: usize) -> impl Iterator<Item = String> + 'a {
    TokenNgramIter { tokens, n, pos: 0 }
}

// ---------------------------------------------------------------------------
// CharNgramIter — yields &str slices of exactly n chars
// ---------------------------------------------------------------------------

struct CharNgramIter<'a> {
    text: &'a str,
    n: usize,
    /// Byte offsets of each char boundary (length = char_count + 1).
    boundaries: Vec<usize>,
    pos: usize,
}

impl<'a> CharNgramIter<'a> {
    fn new(text: &'a str, n: usize) -> Self {
        let boundaries: Vec<usize> = text
            .char_indices()
            .map(|(i, _)| i)
            .chain(core::iter::once(text.len()))
            .collect();
        CharNgramIter {
            text,
            n,
            boundaries,
            pos: 0,
        }
    }
}

impl<'a> Iterator for CharNgramIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n == 0 {
            return None;
        }
        let end_idx = self.pos + self.n;
        if end_idx >= self.boundaries.len() {
            return None;
        }
        let start_byte = self.boundaries[self.pos];
        let end_byte = self.boundaries[end_idx];
        self.pos += 1;
        Some(&self.text[start_byte..end_byte])
    }
}

// ---------------------------------------------------------------------------
// TokenNgramIter — yields concatenated n-token strings
// ---------------------------------------------------------------------------

struct TokenNgramIter<'a> {
    tokens: &'a [&'a str],
    n: usize,
    pos: usize,
}

impl<'a> Iterator for TokenNgramIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n == 0 {
            return None;
        }
        let end = self.pos + self.n;
        if end > self.tokens.len() {
            return None;
        }
        let gram: String = self.tokens[self.pos..end].concat();
        self.pos += 1;
        Some(gram)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── char_ngrams ───────────────────────────────────────────────────────────

    #[test]
    fn char_bigrams_ascii() {
        let grams: Vec<&str> = char_ngrams("abcd", 2).collect();
        assert_eq!(grams, &["ab", "bc", "cd"]);
    }

    #[test]
    fn char_trigrams_thai() {
        let grams: Vec<&str> = char_ngrams("กขค", 3).collect();
        assert_eq!(grams, &["กขค"]);
    }

    #[test]
    fn char_bigrams_thai_multibyte() {
        // Each Thai char is 3 bytes; slices must be valid UTF-8.
        let grams: Vec<&str> = char_ngrams("กขคง", 2).collect();
        assert_eq!(grams, &["กข", "ขค", "คง"]);
        for g in &grams {
            assert_eq!(g.chars().count(), 2);
        }
    }

    #[test]
    fn char_ngrams_n_larger_than_text_is_empty() {
        let grams: Vec<&str> = char_ngrams("กข", 5).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn char_ngrams_n_zero_is_empty() {
        let grams: Vec<&str> = char_ngrams("กขค", 0).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn char_ngrams_empty_text_is_empty() {
        let grams: Vec<&str> = char_ngrams("", 2).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn char_ngrams_n_equals_len_yields_one() {
        let grams: Vec<&str> = char_ngrams("กขค", 3).collect();
        assert_eq!(grams.len(), 1);
        assert_eq!(grams[0], "กขค");
    }

    #[test]
    fn char_ngrams_swasadee_bigrams() {
        let grams: Vec<&str> = char_ngrams("สวัสดี", 2).collect();
        // สวัสดี has 6 chars: ส ว ั ส ด ี → 5 bigrams
        assert_eq!(grams.len(), 5);
        assert!(grams.contains(&"สว"));
    }

    // ── token_ngrams ──────────────────────────────────────────────────────────

    #[test]
    fn token_bigrams_basic() {
        let tokens = &["กิน", "ข้าว", "กับ", "ปลา"];
        let bigrams: Vec<String> = token_ngrams(tokens, 2).collect();
        assert_eq!(bigrams, &["กินข้าว", "ข้าวกับ", "กับปลา"]);
    }

    #[test]
    fn token_trigrams_basic() {
        let tokens = &["กิน", "ข้าว", "กับ", "ปลา"];
        let trigrams: Vec<String> = token_ngrams(tokens, 3).collect();
        assert_eq!(trigrams, &["กินข้าวกับ", "ข้าวกับปลา"]);
    }

    #[test]
    fn token_ngrams_n_larger_than_count_is_empty() {
        let tokens = &["กิน", "ข้าว"];
        let grams: Vec<String> = token_ngrams(tokens, 5).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn token_ngrams_n_zero_is_empty() {
        let tokens = &["กิน", "ข้าว"];
        let grams: Vec<String> = token_ngrams(tokens, 0).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn token_ngrams_empty_tokens_is_empty() {
        let tokens: &[&str] = &[];
        let grams: Vec<String> = token_ngrams(tokens, 2).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn token_unigrams_yield_each_token() {
        let tokens = &["กิน", "ข้าว", "ปลา"];
        let unigrams: Vec<String> = token_ngrams(tokens, 1).collect();
        assert_eq!(
            unigrams,
            &[String::from("กิน"), String::from("ข้าว"), String::from("ปลา")]
        );
    }
}
