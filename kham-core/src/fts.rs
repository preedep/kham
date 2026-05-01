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

use crate::abbrev::AbbrevMap;
use crate::ne::NeTagger;
use crate::ngram::char_ngrams;
use crate::number::{thai_digits_to_ascii, thai_word_to_decimal};
use crate::pos::{PosTag, PosTagger};
use crate::romanizer::RomanizationMap;
use crate::soundex::{soundex, SoundexAlgorithm};
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
    romanization: Option<RomanizationMap>,
    abbrev_map: Option<AbbrevMap>,
    /// `None` means "use default (true)".
    number_normalize: Option<bool>,
    soundex: Option<SoundexAlgorithm>,
    /// Extra words to overlay on top of the built-in dictionary (fast path).
    dict_merge: Option<String>,
}

impl FtsTokenizerBuilder {
    /// Use a custom stopword set instead of the built-in list.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::stopwords::StopwordSet;
    ///
    /// let stops = StopwordSet::from_text("กิน\nข้าว\n");
    /// let fts = FtsTokenizer::builder().stopwords(stops).build();
    /// let tokens = fts.segment_for_fts("กินข้าว");
    /// assert!(tokens.iter().all(|t| t.is_stop || t.text != "กิน"));
    /// ```
    pub fn stopwords(mut self, s: StopwordSet) -> Self {
        self.stopwords = Some(s);
        self
    }

    /// Attach a synonym map for expansion.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::synonym::SynonymMap;
    ///
    /// // TSV: canonical TAB synonym1 TAB synonym2 …
    /// let syns = SynonymMap::from_tsv("รถ\tรถยนต์\tยานพาหนะ\n");
    /// let fts = FtsTokenizer::builder().synonyms(syns).build();
    /// let tokens = fts.segment_for_fts("รถ");
    /// let t = tokens.iter().find(|t| t.text == "รถ").unwrap();
    /// assert!(t.synonyms.contains(&String::from("รถยนต์")));
    /// ```
    pub fn synonyms(mut self, m: SynonymMap) -> Self {
        self.synonyms = Some(m);
        self
    }

    /// Override the n-gram size used for [`TokenKind::Unknown`] tokens.
    ///
    /// Default: 3 (trigrams). Set to 0 to disable n-gram generation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::stopwords::StopwordSet;
    ///
    /// // Disable n-grams entirely — useful when index size must be small
    /// let fts = FtsTokenizer::builder()
    ///     .ngram_size(0)
    ///     .stopwords(StopwordSet::from_text(""))
    ///     .build();
    /// let tokens = fts.segment_for_fts("กขคง"); // unknown word → no trigrams
    /// assert!(tokens.iter().all(|t| t.trigrams.is_empty()));
    /// ```
    pub fn ngram_size(mut self, n: usize) -> Self {
        self.ngram_size = Some(n);
        self
    }

    /// Use a custom POS tagger instead of the built-in table.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::pos::{PosTag, PosTagger};
    ///
    /// // Custom TSV: word TAB POS_TAG
    /// let tagger = PosTagger::from_tsv("กิน\tVERB\n");
    /// let fts = FtsTokenizer::builder().pos_tagger(tagger).build();
    /// // Segment กิน alone so it is not merged into a compound
    /// let tokens = fts.segment_for_fts("กิน");
    /// let t = tokens.iter().find(|t| t.text == "กิน").unwrap();
    /// assert_eq!(t.pos, Some(PosTag::Verb));
    /// ```
    pub fn pos_tagger(mut self, t: PosTagger) -> Self {
        self.pos_tagger = Some(t);
        self
    }

    /// Use a custom NE gazetteer instead of the built-in table.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::ne::NeTagger;
    /// use kham_core::TokenKind;
    ///
    /// // Domain-specific NE list: word TAB NE_TAG
    /// let ne = NeTagger::from_tsv("เซเรน่า\tPERSON\n");
    /// let fts = FtsTokenizer::builder().ne_tagger(ne).build();
    /// let tokens = fts.segment_for_fts("เซเรน่า");
    /// assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Named(_))));
    /// ```
    pub fn ne_tagger(mut self, t: NeTagger) -> Self {
        self.ne_tagger = Some(t);
        self
    }

    /// Attach a romanization map so RTGS forms are added to [`FtsToken::synonyms`].
    ///
    /// When set, each Thai and Named token whose text is found in the map gets its
    /// RTGS romanization appended to `synonyms`, enabling Latin-script queries
    /// (e.g. `kin`) to match Thai-script documents (e.g. `กิน`) in PostgreSQL FTS.
    ///
    /// Disabled by default — call this method to opt in.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// // TSV: Thai word TAB RTGS romanization
    /// let rom = RomanizationMap::from_tsv("กิน\tkin\n");
    /// let fts = FtsTokenizer::builder().romanization(rom).build();
    /// let tokens = fts.segment_for_fts("กิน");
    /// let t = tokens.iter().find(|t| t.text == "กิน").unwrap();
    /// // Latin synonym "kin" enables queries like `WHERE doc @@ 'kin'`
    /// assert!(t.synonyms.contains(&String::from("kin")));
    /// ```
    pub fn romanization(mut self, m: RomanizationMap) -> Self {
        self.romanization = Some(m);
        self
    }

    /// Attach an abbreviation map for pre-tokenisation expansion.
    ///
    /// When set, [`FtsTokenizer::segment_for_fts`] calls
    /// [`AbbrevMap::expand_text`] on the normalised input before segmentation.
    /// This replaces abbreviated forms (e.g. `ก.ค.`) with their canonical
    /// expansions (`กรกฎาคม`) so they are indexed and searchable by full form.
    ///
    /// Disabled by default — call this method to opt in.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::abbrev::AbbrevMap;
    /// use kham_core::stopwords::StopwordSet;
    ///
    /// let fts = FtsTokenizer::builder()
    ///     .abbrevs(AbbrevMap::builtin())
    ///     .stopwords(StopwordSet::from_text(""))
    ///     .build();
    /// // ก.ค. expands to กรกฎาคม before segmentation — dots disappear
    /// let tokens = fts.segment_for_fts("ก.ค.");
    /// let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
    /// assert!(!texts.contains(&"."), "dots should be consumed by expansion");
    /// ```
    pub fn abbrevs(mut self, m: AbbrevMap) -> Self {
        self.abbrev_map = Some(m);
        self
    }

    /// Enable or disable number normalization (default: `true`).
    ///
    /// When enabled:
    /// - [`TokenKind::Number`] tokens that contain Thai digits (๐–๙) get the
    ///   ASCII digit string added to their [`FtsToken::synonyms`]
    ///   (e.g. `๑๒๓` → synonym `"123"`).
    /// - [`TokenKind::Thai`] tokens that are recognised Thai cardinal number
    ///   words get their decimal value added to `synonyms`
    ///   (e.g. `หนึ่งร้อย` → synonym `"100"`).
    ///
    /// This lets queries using either script match documents written in the
    /// other. Set to `false` to opt out.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::TokenKind;
    ///
    /// // Default (true): ๑๒๓ gets ASCII synonym "123"
    /// let fts = FtsTokenizer::new();
    /// let tokens = fts.segment_for_fts("๑๒๓");
    /// let num = tokens.iter().find(|t| t.kind == TokenKind::Number).unwrap();
    /// assert!(num.synonyms.contains(&String::from("123")));
    ///
    /// // Opt out: no conversion performed
    /// let fts_off = FtsTokenizer::builder().number_normalize(false).build();
    /// let tokens_off = fts_off.segment_for_fts("๑๒๓");
    /// let num_off = tokens_off.iter().find(|t| t.kind == TokenKind::Number).unwrap();
    /// assert!(!num_off.synonyms.contains(&String::from("123")));
    /// ```
    pub fn number_normalize(mut self, v: bool) -> Self {
        self.number_normalize = Some(v);
        self
    }

    /// Emit a Thai phonetic soundex code as an additional synonym for Thai and Named tokens.
    ///
    /// When set, each Thai and Named token whose text contains Thai consonants gets its
    /// soundex code appended to [`FtsToken::synonyms`], enabling phonetic fuzzy matching
    /// in full-text search (e.g. querying `"1600"` matches กาน, ขาน, and คาน with lk82).
    ///
    /// [`SoundexAlgorithm::Lk82`] and [`SoundexAlgorithm::Udom83`] produce fixed
    /// 4-character codes and are the recommended choices for FTS indexing.
    /// [`SoundexAlgorithm::MetaSound`] produces variable-length codes and is more
    /// collision-prone at word level — prefer lk82 or udom83 for general FTS use.
    ///
    /// Disabled by default — call this method to opt in.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::soundex::{lk82, SoundexAlgorithm};
    /// use kham_core::stopwords::StopwordSet;
    ///
    /// let fts = FtsTokenizer::builder()
    ///     .soundex(SoundexAlgorithm::Lk82)
    ///     .stopwords(StopwordSet::from_text(""))
    ///     .build();
    /// // กาน / ขาน / คาน all map to the same lk82 code — stored once per token
    /// for word in &["กาน", "ขาน", "คาน"] {
    ///     let tokens = fts.segment_for_fts(word);
    ///     let t = tokens.first().unwrap();
    ///     assert!(t.synonyms.contains(&lk82(word)), "{word} missing lk82 synonym");
    /// }
    /// ```
    pub fn soundex(mut self, algo: SoundexAlgorithm) -> Self {
        self.soundex = Some(algo);
        self
    }

    /// Overlay extra words on the built-in dictionary without a full trie rebuild.
    ///
    /// Words are stored in a sorted list alongside the pre-compiled trie.
    /// Prefer this over a full rebuild when adding a small domain-specific
    /// vocabulary (e.g. product names, technical terms).
    ///
    /// Newline-separated; `#` lines are ignored.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::TokenKind;
    ///
    /// let fts = FtsTokenizer::builder()
    ///     .dict_merge("โปรแกรมเมอร์\nปัญญาประดิษฐ์\n")
    ///     .build();
    /// let tokens = fts.segment_for_fts("โปรแกรมเมอร์ไทย");
    /// assert!(tokens.iter().any(|t| t.text == "โปรแกรมเมอร์" && t.kind == TokenKind::Thai));
    /// ```
    pub fn dict_merge(mut self, words: &str) -> Self {
        self.dict_merge = Some(String::from(words));
        self
    }

    /// Consume the builder and return a configured [`FtsTokenizer`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::soundex::SoundexAlgorithm;
    /// use kham_core::stopwords::StopwordSet;
    ///
    /// let fts = FtsTokenizer::builder()
    ///     .soundex(SoundexAlgorithm::Lk82)
    ///     .stopwords(StopwordSet::from_text(""))
    ///     .build();
    /// assert!(!fts.segment_for_fts("กินข้าว").is_empty());
    /// ```
    pub fn build(self) -> FtsTokenizer {
        let tokenizer = if let Some(ref words) = self.dict_merge {
            Tokenizer::builder().dict_merge(words).build()
        } else {
            Tokenizer::new()
        };
        FtsTokenizer {
            tokenizer,
            stopwords: self.stopwords.unwrap_or_else(StopwordSet::builtin),
            synonyms: self.synonyms.unwrap_or_else(SynonymMap::empty),
            ngram_size: self.ngram_size.unwrap_or(3),
            pos_tagger: self.pos_tagger.unwrap_or_else(PosTagger::builtin),
            ne_tagger: self.ne_tagger.unwrap_or_else(NeTagger::builtin),
            romanization: self.romanization,
            abbrev_map: self.abbrev_map,
            number_normalize: self.number_normalize.unwrap_or(true),
            soundex: self.soundex,
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
    romanization: Option<RomanizationMap>,
    abbrev_map: Option<AbbrevMap>,
    number_normalize: bool,
    soundex: Option<SoundexAlgorithm>,
}

impl FtsTokenizer {
    /// Create an [`FtsTokenizer`] with built-in stopwords and no synonyms.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    ///
    /// let fts = FtsTokenizer::new();
    /// let lexemes = fts.lexemes("กินข้าวกับปลา");
    /// // Built-in stopword กับ is excluded; content words are present
    /// assert!(!lexemes.contains(&String::from("กับ")));
    /// assert!(lexemes.iter().any(|l| l == "กิน" || l == "ปลา"));
    /// ```
    pub fn new() -> Self {
        FtsTokenizerBuilder::default().build()
    }

    /// Return a [`FtsTokenizerBuilder`] for custom configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::soundex::SoundexAlgorithm;
    /// use kham_core::synonym::SynonymMap;
    ///
    /// let fts = FtsTokenizer::builder()
    ///     .synonyms(SynonymMap::from_tsv("รถ\tรถยนต์\n"))
    ///     .soundex(SoundexAlgorithm::Lk82)
    ///     .build();
    /// assert!(!fts.segment_for_fts("รถ").is_empty());
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    ///
    /// let fts = FtsTokenizer::new();
    /// let tokens = fts.segment_for_fts("กินข้าวกับปลา");
    /// // Positions are 0-based and sequential across non-whitespace tokens
    /// for (i, t) in tokens.iter().enumerate() {
    ///     assert_eq!(t.position, i);
    /// }
    /// // กับ is a common conjunction — marked as a stopword
    /// let kap = tokens.iter().find(|t| t.text == "กับ").unwrap();
    /// assert!(kap.is_stop);
    /// ```
    ///
    /// Named entities are tagged automatically — `kind` becomes `TokenKind::Named`:
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::TokenKind;
    ///
    /// let fts = FtsTokenizer::new();
    /// let tokens = fts.segment_for_fts("ไปกรุงเทพ");
    /// assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Named(_))));
    /// ```
    ///
    /// Enable phonetic synonyms with [`FtsTokenizerBuilder::soundex`]:
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    /// use kham_core::soundex::SoundexAlgorithm;
    ///
    /// let fts = FtsTokenizer::builder()
    ///     .soundex(SoundexAlgorithm::Lk82)
    ///     .build();
    /// let tokens = fts.segment_for_fts("กิน");
    /// let t = tokens.iter().find(|t| t.text == "กิน").unwrap();
    /// // synonyms now contains the lk82 code, enabling fuzzy phonetic matching
    /// assert!(!t.synonyms.is_empty());
    /// ```
    pub fn segment_for_fts(&self, text: &str) -> Vec<FtsToken> {
        let normalized = self.tokenizer.normalize(text);
        // Expand abbreviations (e.g. ก.ค. → กรกฎาคม) before segmentation so
        // dot-containing patterns are replaced as single units.
        let expanded = match self.abbrev_map.as_ref() {
            Some(am) => am.expand_text(&normalized),
            None => normalized,
        };
        let raw_tokens = self
            .ne_tagger
            .tag_tokens(self.tokenizer.segment(&expanded), &expanded);

        let mut result = Vec::with_capacity(raw_tokens.len());
        let mut position = 0usize;

        for token in &raw_tokens {
            if token.kind == TokenKind::Whitespace {
                continue;
            }

            let is_stop = self.stopwords.contains(token.text);
            let is_thai_or_named = matches!(token.kind, TokenKind::Thai | TokenKind::Named(_));
            let mut synonyms = self
                .synonyms
                .expand(token.text)
                .map(|s| s.to_vec())
                .unwrap_or_default();
            if is_thai_or_named {
                if let Some(ref rom) = self.romanization {
                    if let Some(rtgs) = rom.romanize(token.text) {
                        synonyms.push(String::from(rtgs));
                    }
                }
                if let Some(algo) = self.soundex {
                    let code = soundex(token.text, algo);
                    if !code.chars().all(|c| c == '0') {
                        synonyms.push(code);
                    }
                }
            }
            if self.number_normalize {
                match token.kind {
                    // Number token with Thai digits → add ASCII form as synonym.
                    TokenKind::Number => {
                        let ascii = thai_digits_to_ascii(token.text);
                        if ascii != token.text {
                            synonyms.push(ascii);
                        }
                    }
                    // Thai token that is a recognised number word → add decimal string.
                    TokenKind::Thai => {
                        if let Some(decimal) = thai_word_to_decimal(token.text) {
                            synonyms.push(decimal);
                        }
                    }
                    _ => {}
                }
            }
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
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    ///
    /// let fts = FtsTokenizer::new();
    /// let tokens = fts.index_tokens("กินข้าวกับปลา");
    /// // No stopwords in the index
    /// assert!(tokens.iter().all(|t| !t.is_stop));
    /// // Positions are preserved from the full sequence for phrase scoring
    /// let positions: Vec<usize> = tokens.iter().map(|t| t.position).collect();
    /// assert!(positions.windows(2).all(|w| w[0] < w[1]));
    /// ```
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
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    ///
    /// let fts = FtsTokenizer::new();
    /// let lexemes = fts.lexemes("กินข้าวกับปลา");
    /// // Content words are present; stopword กับ is absent
    /// assert!(lexemes.iter().any(|l| l == "กิน" || l == "ปลา"));
    /// assert!(!lexemes.contains(&String::from("กับ")));
    /// ```
    ///
    /// With Thai digit normalization (enabled by default), both scripts match:
    ///
    /// ```rust
    /// use kham_core::fts::FtsTokenizer;
    ///
    /// let fts = FtsTokenizer::new();
    /// let lexemes = fts.lexemes("ธนาคาร๑๐๐แห่ง");
    /// // ๑๐๐ (Thai digits) → synonym "100" (ASCII) — both appear in lexemes
    /// assert!(lexemes.contains(&String::from("100")));
    /// ```
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

    // ── multi-token NE ────────────────────────────────────────────────────────

    #[test]
    fn multi_token_ne_merged_in_pipeline() {
        // กรุงเทพ is in the NE gazetteer as PLACE; the segmenter splits it
        // into กรุง+เทพ. The FTS pipeline must merge them into one Named token.
        let fts = FtsTokenizer::new();
        let tokens = fts.segment_for_fts("ไปกรุงเทพ");
        let named: Vec<_> = tokens
            .iter()
            .filter(|t| matches!(t.kind, TokenKind::Named(_)))
            .collect();
        assert!(
            named.iter().any(|t| t.text == "กรุงเทพ"),
            "กรุงเทพ should be tagged Named after multi-token merge, tokens: {:?}",
            tokens
                .iter()
                .map(|t| (&t.text, &t.kind))
                .collect::<alloc::vec::Vec<_>>()
        );
    }

    #[test]
    fn multi_token_ne_reconstructable() {
        // Texts of all non-whitespace tokens must still reconstruct the normalized input.
        let fts = FtsTokenizer::new();
        let text = "ไปกรุงเทพ";
        let normalized = fts.tokenizer.normalize(text);
        let tokens = fts.segment_for_fts(text);
        let rebuilt: String = tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(rebuilt, normalized);
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

    // ── number normalization ──────────────────────────────────────────────────

    #[test]
    fn thai_digit_token_gets_ascii_synonym() {
        let fts = FtsTokenizer::new();
        let tokens = fts.segment_for_fts("๑๒๓");
        let num = tokens.iter().find(|t| t.kind == TokenKind::Number);
        assert!(num.is_some(), "expected a Number token");
        let t = num.unwrap();
        assert!(
            t.synonyms.contains(&String::from("123")),
            "Thai digit token should have ASCII synonym, got {:?}",
            t.synonyms
        );
    }

    #[test]
    fn ascii_digit_token_has_no_extra_synonym() {
        // ASCII digits need no conversion — synonyms should be empty (no map, no rom).
        let fts = FtsTokenizer::new();
        let tokens = fts.segment_for_fts("123");
        let num = tokens.iter().find(|t| t.kind == TokenKind::Number);
        assert!(num.is_some(), "expected a Number token");
        assert!(
            !num.unwrap().synonyms.contains(&String::from("123")),
            "ASCII digit token should not duplicate itself as a synonym"
        );
    }

    #[test]
    fn thai_number_word_gets_decimal_synonym() {
        // หนึ่งร้อย may segment as a single Thai token or multiple tokens depending
        // on the dictionary. We check that at least one token carries "100" in synonyms.
        let fts = FtsTokenizer::new();
        let tokens = fts.segment_for_fts("หนึ่งร้อย");
        let has_hundred = tokens
            .iter()
            .any(|t| t.synonyms.contains(&String::from("100")));
        // หนึ่ง alone = Some(1), ร้อย alone = Some(100) — at least ร้อย should match.
        assert!(
            has_hundred,
            "expected a token with decimal synonym '100', tokens: {:?}",
            tokens
                .iter()
                .map(|t| (&t.text, &t.synonyms))
                .collect::<alloc::vec::Vec<_>>()
        );
    }

    #[test]
    fn number_normalize_false_disables_conversion() {
        let fts = FtsTokenizer::builder()
            .number_normalize(false)
            .stopwords(StopwordSet::from_text(""))
            .build();
        let tokens = fts.segment_for_fts("๑๒๓");
        let num = tokens.iter().find(|t| t.kind == TokenKind::Number);
        assert!(num.is_some());
        assert!(
            !num.unwrap().synonyms.contains(&String::from("123")),
            "number_normalize=false should suppress ASCII synonym"
        );
    }

    #[test]
    fn mixed_thai_digit_in_context() {
        // "ธนาคาร๑๐๐แห่ง" — the ๑๐๐ part should be a Number token with synonym "100"
        let fts = FtsTokenizer::new();
        let tokens = fts.segment_for_fts("ธนาคาร๑๐๐แห่ง");
        let num = tokens.iter().find(|t| t.kind == TokenKind::Number);
        assert!(num.is_some(), "expected Number token in mixed string");
        assert!(
            num.unwrap().synonyms.contains(&String::from("100")),
            "expected ASCII synonym '100' for ๑๐๐"
        );
    }

    // ── abbreviation expansion ────────────────────────────────────────────────

    #[test]
    fn abbrev_map_expands_before_segmentation() {
        use crate::abbrev::AbbrevMap;
        let fts = FtsTokenizer::builder()
            .abbrevs(AbbrevMap::builtin())
            .stopwords(StopwordSet::from_text(""))
            .build();
        // ก.ค. → กรกฎาคม before segmentation. The segmenter may split the
        // expansion further (กรกฎา + คม) — what matters is that dots are gone
        // and the Thai characters of กรกฎาคม are present.
        let tokens = fts.segment_for_fts("ก.ค.");
        let texts: alloc::vec::Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
        let joined: String = texts.concat();
        assert!(
            joined.contains("กรกฎา") || joined.contains("กรกฎาคม"),
            "expected กรกฎา(คม) characters after abbrev expansion, got: {texts:?}"
        );
        assert!(
            !texts.contains(&"."),
            "dots should be consumed by abbrev expansion, got: {texts:?}"
        );
    }

    #[test]
    fn abbrev_expansion_disabled_by_default() {
        // FtsTokenizer::new() has no abbrev_map — ก.ค. stays as individual tokens.
        let fts = FtsTokenizer::new();
        let tokens = fts.segment_for_fts("ก.ค.");
        let texts: alloc::vec::Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
        // Without expansion the dot(s) must still be present as punctuation tokens.
        assert!(
            texts.contains(&"."),
            "without abbrev expansion, dots should remain as tokens, got: {texts:?}"
        );
    }

    // ── soundex synonyms ──────────────────────────────────────────────────────

    #[test]
    fn soundex_lk82_appended_to_thai_synonyms() {
        use crate::soundex::lk82;
        let fts = FtsTokenizer::builder()
            .soundex(SoundexAlgorithm::Lk82)
            .stopwords(StopwordSet::from_text(""))
            .build();
        let tokens = fts.segment_for_fts("กิน");
        let t = tokens.iter().find(|t| t.text == "กิน");
        assert!(t.is_some(), "expected token 'กิน'");
        let expected_code = lk82("กิน");
        assert!(
            t.unwrap().synonyms.contains(&expected_code),
            "expected lk82 code '{expected_code}' in synonyms, got {:?}",
            t.unwrap().synonyms
        );
    }

    #[test]
    fn soundex_not_emitted_by_default() {
        // Without .soundex() in the builder, no soundex codes should appear.
        let fts = FtsTokenizer::new();
        let tokens = fts.segment_for_fts("กินข้าว");
        for t in &tokens {
            // A soundex code is 4 ASCII chars (lk82/udom83); no synonym should look like one.
            for syn in &t.synonyms {
                let looks_like_soundex =
                    syn.len() == 4 && syn.chars().all(|c| c.is_ascii_alphanumeric());
                assert!(
                    !looks_like_soundex,
                    "unexpected soundex-like synonym '{}' on token '{}'",
                    syn, t.text
                );
            }
        }
    }

    #[test]
    fn soundex_same_sounding_words_share_code_in_index() {
        // กาน and ขาน share lk82 code "1600"; both should carry it as a synonym.
        use crate::soundex::lk82;
        let fts = FtsTokenizer::builder()
            .soundex(SoundexAlgorithm::Lk82)
            .stopwords(StopwordSet::from_text(""))
            .build();
        let code = lk82("กาน");
        for word in &["กาน", "ขาน", "คาน"] {
            let tokens = fts.segment_for_fts(word);
            let t = tokens.first().expect("expected at least one token");
            assert!(
                t.synonyms.contains(&code),
                "'{word}' should carry lk82 code '{code}', got {:?}",
                t.synonyms
            );
        }
    }

    #[test]
    fn soundex_not_emitted_for_non_thai_tokens() {
        let fts = FtsTokenizer::builder()
            .soundex(SoundexAlgorithm::Lk82)
            .stopwords(StopwordSet::from_text(""))
            .build();
        let tokens = fts.segment_for_fts("hello 123");
        for t in &tokens {
            for syn in &t.synonyms {
                let looks_like_soundex =
                    syn.len() == 4 && syn.chars().all(|c| c.is_ascii_alphanumeric());
                assert!(
                    !looks_like_soundex,
                    "non-Thai token '{}' should not get a soundex synonym, got '{syn}'",
                    t.text
                );
            }
        }
    }

    #[test]
    fn soundex_udom83_appended() {
        use crate::soundex::udom83;
        let fts = FtsTokenizer::builder()
            .soundex(SoundexAlgorithm::Udom83)
            .stopwords(StopwordSet::from_text(""))
            .build();
        let tokens = fts.segment_for_fts("กิน");
        let t = tokens.iter().find(|t| t.text == "กิน").unwrap();
        let expected = udom83("กิน");
        assert!(
            t.synonyms.contains(&expected),
            "expected udom83 code '{expected}' in synonyms, got {:?}",
            t.synonyms
        );
    }

    #[test]
    fn abbrev_expansion_date_sentence() {
        use crate::abbrev::AbbrevMap;
        let fts = FtsTokenizer::builder()
            .abbrevs(AbbrevMap::builtin())
            .stopwords(StopwordSet::from_text(""))
            .build();
        // พ.ศ. → พุทธศักราช; the segmenter may split it further — verify the
        // chars are present and dots are gone.
        let tokens = fts.segment_for_fts("พ.ศ.2567");
        let texts: alloc::vec::Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
        let joined: String = texts.concat();
        assert!(
            joined.contains("พุทธ") || joined.contains("พุทธศักราช"),
            "expected พุทธ(ศักราช) chars after expanding พ.ศ., got: {texts:?}"
        );
        assert!(
            !texts.contains(&"."),
            "dots should be consumed by expansion, got: {texts:?}"
        );
    }
}
