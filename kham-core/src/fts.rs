//! Full-text search pipeline for Thai text.
//!
//! [`FtsTokenizer`] orchestrates the complete FTS indexing pipeline:
//! normalise → segment → tag stopwords → expand synonyms → attach position.
//!
//! The output [`FtsToken`] slice is consumed by the PostgreSQL `kham-pg`
//! extension and by any other caller that needs FTS-ready lexemes.
//!
//! # Positions
//!
//! `position` is the ordinal index of the token in the non-whitespace token
//! sequence (0-based). Stopwords retain their position so that phrase-distance
//! scoring remains correct when stopwords are later omitted from the index.
//!
//! # Example
//!
//! ```rust
//! use kham_core::fts::{FtsTokenizer, FtsToken};
//!
//! let fts = FtsTokenizer::new();
//! let tokens = fts.segment_for_fts("กินข้าวกับปลา");
//! for t in &tokens {
//!     println!("{} pos={} stop={}", t.text, t.position, t.is_stop);
//! }
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use crate::ne::NeTagger;
use crate::ngram::char_ngrams;
use crate::pos::{PosTag, PosTagger};
use crate::stopwords::StopwordSet;
use crate::synonym::SynonymMap;
use crate::token::{NamedEntityKind, TokenKind};
use crate::Tokenizer;

/// A token produced by the FTS pipeline, ready for lexeme indexing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtsToken {
    /// The token text (owned; may be normalised).
    pub text: String,
    /// Ordinal position in the token sequence (0-based, gaps for whitespace).
    pub position: usize,
    /// Script / category of the original token.
    pub kind: TokenKind,
    /// `true` if this token matches the stopword list.
    pub is_stop: bool,
    /// Synonym expansions (empty if none configured or no match).
    pub synonyms: Vec<String>,
    /// Character trigrams — populated only for [`TokenKind::Unknown`] tokens.
    pub trigrams: Vec<String>,
    /// Primary part-of-speech tag from the lookup table, or `None` if the word
    /// is not in the table (OOV) or is not a Thai token.
    pub pos: Option<PosTag>,
    /// Named entity category, or `None` if the token is not in the NE
    /// gazetteer. When set, `kind` is [`TokenKind::Named`]`(ne)`.
    pub ne: Option<NamedEntityKind>,
}

/// Builder for [`FtsTokenizer`].
#[derive(Default)]
pub struct FtsTokenizerBuilder {
    stopwords: Option<StopwordSet>,
    synonyms: Option<SynonymMap>,
    ngram_size: Option<usize>,
    pos_tagger: Option<PosTagger>,
    ne_tagger: Option<NeTagger>,
}

impl FtsTokenizerBuilder {
    /// Use a custom stopword set instead of the built-in list.
    pub fn stopwords(mut self, s: StopwordSet) -> Self {
        self.stopwords = Some(s);
        self
    }

    /// Attach a synonym map for expansion.
    pub fn synonyms(mut self, m: SynonymMap) -> Self {
        self.synonyms = Some(m);
        self
    }

    /// Override the n-gram size used for [`TokenKind::Unknown`] tokens.
    ///
    /// Default: 3 (trigrams). Set to 0 to disable n-gram generation.
    pub fn ngram_size(mut self, n: usize) -> Self {
        self.ngram_size = Some(n);
        self
    }

    /// Use a custom POS tagger instead of the built-in table.
    pub fn pos_tagger(mut self, t: PosTagger) -> Self {
        self.pos_tagger = Some(t);
        self
    }

    /// Use a custom NE gazetteer instead of the built-in table.
    pub fn ne_tagger(mut self, t: NeTagger) -> Self {
        self.ne_tagger = Some(t);
        self
    }

    /// Consume the builder and return a configured [`FtsTokenizer`].
    pub fn build(self) -> FtsTokenizer {
        FtsTokenizer {
            tokenizer: Tokenizer::new(),
            stopwords: self.stopwords.unwrap_or_else(StopwordSet::builtin),
            synonyms: self.synonyms.unwrap_or_else(SynonymMap::empty),
            ngram_size: self.ngram_size.unwrap_or(3),
            pos_tagger: self.pos_tagger.unwrap_or_else(PosTagger::builtin),
            ne_tagger: self.ne_tagger.unwrap_or_else(NeTagger::builtin),
        }
    }
}

/// Full-text search tokenizer for Thai text.
///
/// Wraps [`Tokenizer`] with stopword filtering, synonym expansion, and n-gram
/// generation for out-of-vocabulary tokens.
///
/// Construct once and reuse:
///
/// ```rust
/// use kham_core::fts::FtsTokenizer;
///
/// let fts = FtsTokenizer::new();
/// let tokens = fts.segment_for_fts("กินข้าวกับปลา");
/// assert!(!tokens.is_empty());
/// ```
pub struct FtsTokenizer {
    tokenizer: Tokenizer,
    stopwords: StopwordSet,
    synonyms: SynonymMap,
    ngram_size: usize,
    pos_tagger: PosTagger,
    ne_tagger: NeTagger,
}

impl FtsTokenizer {
    /// Create an [`FtsTokenizer`] with built-in stopwords and no synonyms.
    pub fn new() -> Self {
        FtsTokenizerBuilder::default().build()
    }

    /// Return a [`FtsTokenizerBuilder`] for custom configuration.
    pub fn builder() -> FtsTokenizerBuilder {
        FtsTokenizerBuilder::default()
    }

    /// Segment `text` and annotate each token for FTS indexing.
    ///
    /// Normalises the input text before segmentation so that สระลอย and stacked
    /// tone marks are handled correctly. Whitespace tokens are excluded.
    ///
    /// The returned `Vec<FtsToken>` covers all non-whitespace tokens. Call
    /// [`index_tokens`] instead when you only need the tokens to be indexed
    /// (stopwords excluded).
    ///
    /// [`index_tokens`]: FtsTokenizer::index_tokens
    pub fn segment_for_fts(&self, text: &str) -> Vec<FtsToken> {
        let normalized = self.tokenizer.normalize(text);
        let raw_tokens = self
            .ne_tagger
            .tag_tokens(self.tokenizer.segment(&normalized));

        let mut result = Vec::with_capacity(raw_tokens.len());
        let mut position = 0usize;

        for token in &raw_tokens {
            if token.kind == TokenKind::Whitespace {
                continue;
            }

            let is_stop = self.stopwords.contains(token.text);
            let synonyms = self
                .synonyms
                .expand(token.text)
                .map(|s| s.to_vec())
                .unwrap_or_default();
            let trigrams = if token.kind == TokenKind::Unknown && self.ngram_size > 0 {
                char_ngrams(token.text, self.ngram_size)
                    .map(String::from)
                    .collect()
            } else {
                Vec::new()
            };
            let ne = if let TokenKind::Named(k) = token.kind {
                Some(k)
            } else {
                None
            };
            let pos = if token.kind == TokenKind::Thai {
                self.pos_tagger.tag(token.text)
            } else {
                None
            };

            result.push(FtsToken {
                text: String::from(token.text),
                position,
                kind: token.kind,
                is_stop,
                synonyms,
                trigrams,
                pos,
                ne,
            });

            position += 1;
        }

        result
    }

    /// Return only the tokens to be written into a search index.
    ///
    /// Filters out stopwords and whitespace. Each [`FtsToken`] still carries
    /// its original `position` so phrase-distance scoring remains correct.
    pub fn index_tokens(&self, text: &str) -> Vec<FtsToken> {
        self.segment_for_fts(text)
            .into_iter()
            .filter(|t| !t.is_stop)
            .collect()
    }

    /// Collect all lexeme strings to be stored in a `tsvector`.
    ///
    /// Returns one string per non-stop token, plus synonym expansions and
    /// trigrams for unknown tokens. Duplicates are not removed (the caller or
    /// PostgreSQL handles deduplication).
    pub fn lexemes(&self, text: &str) -> Vec<String> {
        let tokens = self.index_tokens(text);
        let mut out: Vec<String> = Vec::with_capacity(tokens.len() * 2);
        for t in tokens {
            out.push(t.text.clone());
            out.extend(t.synonyms);
            out.extend(t.trigrams);
        }
        out
    }
}

impl Default for FtsTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stopwords::StopwordSet;
    use crate::synonym::SynonymMap;

    fn fts() -> FtsTokenizer {
        FtsTokenizer::new()
    }

    // ── segment_for_fts ───────────────────────────────────────────────────────

    #[test]
    fn empty_input_returns_empty() {
        assert!(fts().segment_for_fts("").is_empty());
    }

    #[test]
    fn whitespace_tokens_excluded() {
        let tokens = fts().segment_for_fts("กิน ข้าว");
        assert!(tokens.iter().all(|t| t.kind != TokenKind::Whitespace));
    }

    #[test]
    fn positions_are_sequential() {
        let tokens = fts().segment_for_fts("กินข้าวกับปลา");
        for (i, t) in tokens.iter().enumerate() {
            assert_eq!(t.position, i, "position mismatch at index {i}");
        }
    }

    #[test]
    fn known_stopword_is_tagged() {
        // "กับ" is a common conjunction and should be in the built-in stopword list
        let tokens = fts().segment_for_fts("กินข้าวกับปลา");
        let kap = tokens.iter().find(|t| t.text == "กับ");
        assert!(kap.is_some(), "expected 'กับ' token");
        assert!(kap.unwrap().is_stop, "'กับ' should be tagged as stopword");
    }

    #[test]
    fn content_words_not_tagged_as_stop() {
        let tokens = fts().segment_for_fts("โรงพยาบาล");
        // May be OOV but should not be a stopword
        for t in &tokens {
            assert!(!t.is_stop, "'{}' should not be a stopword", t.text);
        }
    }

    #[test]
    fn text_is_reconstructable() {
        // All tokens joined == normalised input (whitespace dropped)
        let fts = fts();
        let text = "กินข้าวกับปลา";
        let normalized = fts.tokenizer.normalize(text);
        let tokens = fts.segment_for_fts(text);
        let rebuilt: String = tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(rebuilt, normalized);
    }

    // ── synonym expansion ─────────────────────────────────────────────────────

    #[test]
    fn synonym_expansion_attached() {
        let synonyms = SynonymMap::from_tsv("คอม\tคอมพิวเตอร์\tcomputer\n");
        let fts = FtsTokenizer::builder()
            .synonyms(synonyms)
            .stopwords(StopwordSet::from_text(""))
            .build();
        // Segment a text containing "คอม" — need it in dict or it lands as Unknown
        // Use builder with custom word so the segmenter recognises it
        let tokens = fts.segment_for_fts("คอม");
        let t = tokens.iter().find(|t| t.text == "คอม");
        if let Some(tok) = t {
            assert!(
                tok.synonyms.contains(&String::from("คอมพิวเตอร์")),
                "expected synonym expansion, got {:?}",
                tok.synonyms
            );
        }
    }

    #[test]
    fn no_synonyms_when_map_empty() {
        let tokens = fts().segment_for_fts("กินข้าว");
        for t in &tokens {
            assert!(t.synonyms.is_empty());
        }
    }

    // ── unknown token trigrams ────────────────────────────────────────────────

    #[test]
    fn unknown_token_gets_trigrams() {
        // "กิ" = consonant + sara-i, a single 2-char TCC that is not a word.
        // With ngram_size=2 the token should yield one bigram ("กิ").
        // The newmm DP emits Unknown tokens one TCC at a time, so multi-char TCCs
        // (like "กิ") are the shortest unit that can produce n-grams.
        let fts = FtsTokenizer::builder()
            .ngram_size(2)
            .stopwords(StopwordSet::from_text(""))
            .build();
        let tokens = fts.segment_for_fts("กิ");
        let unknown: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown && t.text.chars().count() >= 2)
            .collect();
        assert!(
            !unknown.is_empty(),
            "expected at least one multi-char Unknown token for 'กิ'"
        );
        for u in &unknown {
            assert!(
                !u.trigrams.is_empty(),
                "unknown token '{}' ({} chars) should have bigrams",
                u.text,
                u.text.chars().count()
            );
        }
    }

    #[test]
    fn known_thai_token_has_no_trigrams() {
        let tokens = fts().segment_for_fts("กิน");
        for t in &tokens {
            if t.kind == TokenKind::Thai {
                assert!(
                    t.trigrams.is_empty(),
                    "known Thai token '{}' should not have trigrams",
                    t.text
                );
            }
        }
    }

    #[test]
    fn ngram_size_zero_disables_trigrams() {
        let fts = FtsTokenizer::builder()
            .ngram_size(0)
            .stopwords(StopwordSet::from_text(""))
            .build();
        let tokens = fts.segment_for_fts("กขคง");
        for t in &tokens {
            assert!(t.trigrams.is_empty());
        }
    }

    // ── index_tokens ──────────────────────────────────────────────────────────

    #[test]
    fn index_tokens_excludes_stopwords() {
        let tokens = fts().index_tokens("กินข้าวกับปลา");
        assert!(tokens.iter().all(|t| !t.is_stop));
    }

    #[test]
    fn index_tokens_preserves_positions() {
        // Positions in index_tokens must be a subset of segment_for_fts positions
        let all = fts().segment_for_fts("กินข้าวกับปลา");
        let indexed = fts().index_tokens("กินข้าวกับปลา");
        for t in &indexed {
            assert!(
                all.iter().any(|a| a.position == t.position),
                "indexed token at position {} not found in full token list",
                t.position
            );
        }
    }

    // ── lexemes ───────────────────────────────────────────────────────────────

    #[test]
    fn lexemes_returns_non_stop_texts() {
        let lexemes = fts().lexemes("กินข้าวกับปลา");
        // "กับ" is a stopword — should not appear
        assert!(!lexemes.contains(&String::from("กับ")));
        // Content words should appear
        assert!(
            lexemes
                .iter()
                .any(|l| l == "กิน" || l == "ข้าว" || l == "ปลา"),
            "expected content words in lexemes: {lexemes:?}"
        );
    }

    #[test]
    fn lexemes_empty_input_is_empty() {
        assert!(fts().lexemes("").is_empty());
    }

    // ── builder ───────────────────────────────────────────────────────────────

    #[test]
    fn builder_custom_stopwords() {
        let stops = StopwordSet::from_text("กิน\n");
        let fts = FtsTokenizer::builder().stopwords(stops).build();
        let tokens = fts.segment_for_fts("กินข้าว");
        let gin = tokens.iter().find(|t| t.text == "กิน");
        if let Some(t) = gin {
            assert!(t.is_stop, "'กิน' should be stop with custom list");
        }
    }

    #[test]
    fn builder_default_equals_new() {
        // Both paths should produce the same result for a simple input
        let a = FtsTokenizer::new().lexemes("กินข้าว");
        let b = FtsTokenizer::builder().build().lexemes("กินข้าว");
        assert_eq!(a, b);
    }
}
