//! Thai keyword extraction using TF × inverse-corpus-frequency (TF-IDF proxy).
//!
//! [`KeyExtractor`] segments text with the built-in tokenizer, discards
//! stopwords and single-character tokens, then ranks content words by how
//! often they appear in the document relative to their frequency in the Thai
//! National Corpus (TNC).
//!
//! The scoring formula uses only basic `f32` arithmetic (no transcendentals),
//! keeping the module `no_std` compatible:
//!
//! ```text
//! TF(t)        = occurrences(t, doc) / total_content_tokens(doc)
//! IDF_proxy(t) = (max_tnc_freq + 1) / (tnc_freq(t) + 1)
//! score(t)     = TF(t) × IDF_proxy(t)
//! ```
//!
//! Words absent from TNC receive the maximum IDF weight — they are likely
//! domain-specific and therefore the most distinctive keywords.
//!
//! ```rust
//! use kham_core::keyword::KeyExtractor;
//!
//! let kex = KeyExtractor::builtin();
//! let kws = kex.extract("การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัล", 5);
//! assert!(!kws.is_empty());
//! // Results are always sorted by score descending
//! for pair in kws.windows(2) {
//!     assert!(pair[0].score >= pair[1].score);
//! }
//! ```

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::freq::FreqMap;
use crate::segmenter::Tokenizer;
use crate::stopwords::StopwordSet;
use crate::token::TokenKind;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A keyword extracted from a document with its relevance score.
///
/// Scores are computed as `TF × IDF_proxy`:
/// - **TF**: how often the word appears in this document (normalized by total
///   content tokens)
/// - **IDF_proxy**: `(max_tnc_freq + 1) / (tnc_freq + 1)` — rare corpus
///   words receive a higher weight than common function words
///
/// Keywords are returned sorted by `score` descending.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyword {
    /// The word text.
    pub word: String,
    /// TF × IDF_proxy score. Higher means more document-distinctive.
    pub score: f32,
    /// Raw occurrence count of this word in the document.
    pub count: usize,
}

/// Thai keyword extractor using TF × inverse-corpus-frequency scoring.
///
/// Backed by the built-in 62k-word tokenizer, the TNC frequency table
/// (~106k entries), and the Thai stopword list (~1 029 entries).
///
/// Construction is O(n) in the TNC table size — reuse the returned instance
/// rather than calling [`builtin()`](KeyExtractor::builtin) on every query.
///
/// # Filtering rules
///
/// A token is eligible as a keyword when **all** of the following hold:
/// 1. Kind is `Thai`, `Latin`, `Number`, or `Named` (whitespace, punctuation,
///    emoji, and unknown tokens are always skipped)
/// 2. Character length ≥ 2 (single-char tokens are too coarse to be keywords)
/// 3. Not in the built-in Thai stopword list
///
/// # Examples
///
/// ```rust
/// use kham_core::keyword::KeyExtractor;
///
/// let kex = KeyExtractor::builtin();
///
/// // Rare domain-specific word outranks a common word
/// // "ซอฟต์แวร์" (software) is rare in TNC and should appear as a top keyword
/// let kws = kex.extract("นักพัฒนาซอฟต์แวร์เขียนซอฟต์แวร์ทุกวัน", 5);
/// assert!(kws.iter().any(|k| k.word == "ซอฟต์แวร์"));
/// ```
pub struct KeyExtractor {
    tokenizer: Tokenizer,
    freq: FreqMap,
    stops: StopwordSet,
    max_corpus_freq: u32,
}

impl KeyExtractor {
    /// Create a keyword extractor backed by the built-in tokenizer, TNC
    /// frequency table, and Thai stopword list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::keyword::KeyExtractor;
    ///
    /// let kex = KeyExtractor::builtin();
    /// assert!(!kex.extract("กินข้าวกับปลา", 5).is_empty());
    /// ```
    pub fn builtin() -> Self {
        let freq = FreqMap::builtin();
        let max_corpus_freq = freq.max_freq();
        Self {
            tokenizer: Tokenizer::new(),
            freq,
            stops: StopwordSet::builtin(),
            max_corpus_freq,
        }
    }

    /// Extract up to `max_n` keywords from `text`, ranked by TF-IDF score.
    ///
    /// Returns an empty `Vec` when `text` is empty, contains no eligible
    /// content words, or `max_n` is zero.
    ///
    /// Ties in score are broken alphabetically so results are deterministic.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::keyword::KeyExtractor;
    ///
    /// let kex = KeyExtractor::builtin();
    ///
    /// // Edge cases
    /// assert!(kex.extract("", 5).is_empty());
    /// assert!(kex.extract("กินข้าวกับปลา", 0).is_empty());
    ///
    /// // Score order is non-increasing
    /// let kws = kex.extract("การเรียนภาษาโปรแกรมมิ่งเป็นทักษะสำคัญสำหรับนักพัฒนา", 10);
    /// for pair in kws.windows(2) {
    ///     assert!(
    ///         pair[0].score >= pair[1].score,
    ///         "out-of-order: {:?} before {:?}", pair[0], pair[1]
    ///     );
    /// }
    /// ```
    pub fn extract(&self, text: &str, max_n: usize) -> Vec<Keyword> {
        if text.is_empty() || max_n == 0 {
            return Vec::new();
        }

        let tokens = self.tokenizer.segment(text);

        // Count all content tokens for the TF denominator.
        // Count candidate tokens (non-stop, len ≥ 2) for keyword scoring.
        let mut total_content: usize = 0;
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();

        for token in &tokens {
            match token.kind {
                TokenKind::Whitespace
                | TokenKind::Punctuation
                | TokenKind::Emoji
                | TokenKind::Unknown => continue,
                _ => {}
            }

            total_content += 1;

            // Single-char tokens and stopwords are counted in the denominator
            // but excluded from the keyword candidates.
            if token.text.chars().count() < 2 || self.stops.contains(token.text) {
                continue;
            }

            *counts.entry(String::from(token.text)).or_insert(0) += 1;
        }

        if total_content == 0 || counts.is_empty() {
            return Vec::new();
        }

        let total_f = total_content as f32;
        // IDF numerator: max corpus frequency + 1 (avoids div-by-zero for max entry).
        let idf_num = self.max_corpus_freq as f32 + 1.0;

        let mut results: Vec<Keyword> = counts
            .into_iter()
            .map(|(word, count)| {
                let tf = count as f32 / total_f;
                let corpus_freq = self.freq.get(&word);
                let idf = idf_num / (corpus_freq as f32 + 1.0);
                Keyword {
                    word,
                    score: tf * idf,
                    count,
                }
            })
            .collect();

        // Sort: score DESC, word ASC for deterministic ties
        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(a.word.cmp(&b.word))
        });

        results.truncate(max_n);
        results
    }

    /// Extract up to `max_n` multi-word keyphrases (bigrams and trigrams) from
    /// `text`, ranked by TF × average-IDF score.
    ///
    /// Phrases are formed from adjacent content tokens — tokens that pass the
    /// same eligibility rules as [`extract`]: non-whitespace, non-punctuation,
    /// non-emoji, non-unknown, character length ≥ 2, and not a stopword. A
    /// bigram is two such consecutive tokens; a trigram is three.
    ///
    /// The IDF for a phrase is the average IDF of its constituent words.
    ///
    /// Returns an empty `Vec` when `text` has fewer than 2 eligible tokens or
    /// `max_n` is zero.
    ///
    /// # Example
    /// ```rust
    /// use kham_core::keyword::KeyExtractor;
    ///
    /// let kex = KeyExtractor::builtin();
    /// let phrases = kex.extract_phrases("นักพัฒนาซอฟต์แวร์เขียนโค้ดทุกวัน", 5);
    /// // Each keyword word field contains a space-separated phrase
    /// assert!(phrases.iter().all(|k| k.word.contains(' ')));
    /// ```
    pub fn extract_phrases(&self, text: &str, max_n: usize) -> Vec<Keyword> {
        if text.is_empty() || max_n == 0 {
            return Vec::new();
        }

        let tokens = self.tokenizer.segment(text);

        // Collect eligible content token texts
        let content: Vec<&str> = tokens
            .iter()
            .filter(|t| {
                !matches!(
                    t.kind,
                    TokenKind::Whitespace
                        | TokenKind::Punctuation
                        | TokenKind::Emoji
                        | TokenKind::Unknown
                )
            })
            .filter(|t| t.text.chars().count() >= 2 && !self.stops.contains(t.text))
            .map(|t| t.text)
            .collect();

        if content.len() < 2 {
            return Vec::new();
        }

        let total_f = content.len() as f32;
        let idf_num = self.max_corpus_freq as f32 + 1.0;

        let mut counts: BTreeMap<String, usize> = BTreeMap::new();

        // Bigrams
        for w in content.windows(2) {
            let phrase = alloc::format!("{} {}", w[0], w[1]);
            *counts.entry(phrase).or_insert(0) += 1;
        }
        // Trigrams
        for w in content.windows(3) {
            let phrase = alloc::format!("{} {} {}", w[0], w[1], w[2]);
            *counts.entry(phrase).or_insert(0) += 1;
        }

        let mut results: Vec<Keyword> = counts
            .into_iter()
            .map(|(phrase, count)| {
                let tf = count as f32 / total_f;
                let parts: Vec<&str> = phrase.split(' ').collect();
                let avg_idf = parts
                    .iter()
                    .map(|w| idf_num / (self.freq.get(w) as f32 + 1.0))
                    .sum::<f32>()
                    / parts.len() as f32;
                Keyword {
                    word: phrase,
                    score: tf * avg_idf,
                    count,
                }
            })
            .collect();

        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(a.word.cmp(&b.word))
        });
        results.truncate(max_n);
        results
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn kex() -> KeyExtractor {
        KeyExtractor::builtin()
    }

    // ── edge cases ──────────────────────────────────────────────────────────

    #[test]
    fn empty_text_returns_empty() {
        assert!(kex().extract("", 5).is_empty());
    }

    #[test]
    fn zero_max_n_returns_empty() {
        assert!(kex().extract("กินข้าวกับปลา", 0).is_empty());
    }

    #[test]
    fn only_stopwords_returns_empty() {
        // "และ" "หรือ" "ของ" are all stopwords
        assert!(kex().extract("และหรือของ", 5).is_empty());
    }

    #[test]
    fn only_single_chars_returns_empty() {
        // Single Thai characters are below the min-length threshold
        assert!(kex().extract("ก ข ค ง", 5).is_empty());
    }

    // ── result properties ────────────────────────────────────────────────────

    #[test]
    fn respects_max_n() {
        let kws = kex().extract("การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัลสำหรับนักพัฒนา", 3);
        assert!(kws.len() <= 3, "expected ≤ 3 results, got {}", kws.len());
    }

    #[test]
    fn results_sorted_by_score_descending() {
        let kws = kex().extract("การเรียนภาษาโปรแกรมมิ่งเป็นทักษะสำคัญสำหรับนักพัฒนาซอฟต์แวร์", 10);
        for pair in kws.windows(2) {
            assert!(
                pair[0].score >= pair[1].score,
                "sort order violated: {:?} before {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn count_reflects_occurrences() {
        // "ซอฟต์แวร์" appears 3 times in the input
        let kws = kex().extract("นักพัฒนาซอฟต์แวร์เขียนซอฟต์แวร์และทดสอบซอฟต์แวร์ทุกวัน", 10);
        let sw = kws.iter().find(|k| k.word == "ซอฟต์แวร์");
        assert!(sw.is_some(), "expected ซอฟต์แวร์ in keywords; got: {kws:?}");
        assert_eq!(sw.unwrap().count, 3, "expected count=3 for ซอฟต์แวร์");
    }

    #[test]
    fn stopwords_not_in_results() {
        let kws = kex().extract("กินข้าวกับปลาและดื่มน้ำ", 20);
        // "กับ" and "และ" are stopwords and must not appear
        assert!(
            kws.iter().all(|k| k.word != "กับ" && k.word != "และ"),
            "stopword found in results: {kws:?}"
        );
    }

    #[test]
    fn all_scores_positive() {
        let kws = kex().extract("การพัฒนาซอฟต์แวร์ต้องการทักษะและประสบการณ์", 10);
        assert!(
            kws.iter().all(|k| k.score > 0.0),
            "expected all scores > 0; got: {kws:?}"
        );
    }

    // ── IDF weighting ────────────────────────────────────────────────────────

    #[test]
    fn rare_word_outranks_common_word_with_same_count() {
        // Both appear once; rare corpus word should score higher.
        // "ไดโนเสาร์" (dinosaur) is rare in TNC; "คน" (person) is very common.
        let kws = kex().extract("ไดโนเสาร์กินคน", 10);
        let rare = kws.iter().find(|k| k.word == "ไดโนเสาร์");
        let common = kws.iter().find(|k| k.word == "คน");
        if let (Some(r), Some(c)) = (rare, common) {
            assert!(
                r.score > c.score,
                "expected ไดโนเสาร์ ({}) to outscore คน ({})",
                r.score,
                c.score
            );
        }
    }

    #[test]
    fn repeated_word_scores_higher_than_single_occurrence() {
        // "ซอฟต์แวร์" ×3 vs "นักพัฒนา" ×1 — same IDF, TF difference wins
        let kws = kex().extract("นักพัฒนาซอฟต์แวร์เขียนซอฟต์แวร์และทดสอบซอฟต์แวร์", 10);
        let sw = kws.iter().find(|k| k.word == "ซอฟต์แวร์");
        let dev = kws.iter().find(|k| k.word == "นักพัฒนา");
        if let (Some(s), Some(d)) = (sw, dev) {
            assert!(
                s.score > d.score,
                "expected ซอฟต์แวร์ (×3, score {}) > นักพัฒนา (×1, score {})",
                s.score,
                d.score
            );
        }
    }

    // ── mixed script ─────────────────────────────────────────────────────────

    #[test]
    fn latin_tokens_included_as_candidates() {
        let kws = kex().extract("เขียน Python และใช้ Python ทุกวัน", 10);
        // "Python" appears twice and is a Latin token — must be in results
        let py = kws.iter().find(|k| k.word == "Python");
        assert!(py.is_some(), "expected Python in keywords; got: {kws:?}");
        assert_eq!(py.unwrap().count, 2);
    }

    #[test]
    fn punctuation_not_in_results() {
        let kws = kex().extract("กินข้าว, ดื่มน้ำ. นอนหลับ!", 20);
        assert!(
            kws.iter()
                .all(|k| !k.word.chars().all(|c| c.is_ascii_punctuation())),
            "punctuation token found in results: {kws:?}"
        );
    }

    // extract_phrases tests ----------------------------------------------------

    #[test]
    fn extract_phrases_empty_input() {
        assert!(kex().extract_phrases("", 5).is_empty());
    }

    #[test]
    fn extract_phrases_contains_space() {
        let phrases = kex().extract_phrases("นักพัฒนาซอฟต์แวร์เขียนโค้ดทุกวัน", 5);
        assert!(
            phrases.iter().all(|k| k.word.contains(' ')),
            "all phrases should contain a space; got: {phrases:?}"
        );
    }

    #[test]
    fn extract_phrases_score_order() {
        let phrases = kex().extract_phrases("การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัลสำหรับนักพัฒนา", 10);
        for pair in phrases.windows(2) {
            assert!(
                pair[0].score >= pair[1].score,
                "sort order violated: {:?} before {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
