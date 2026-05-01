//! Python bindings for kham-core via PyO3.
//!
//! Build with:
//! ```bash
//! maturin develop -m kham-python/Cargo.toml
//! ```
//!
//! Then from Python:
//! ```python
//! import kham
//!
//! # Simple: list of token strings
//! words = kham.segment("กินข้าวกับปลา")
//! # ['กิน', 'ข้าว', 'กับ', 'ปลา']
//!
//! # Rich tokens with span information
//! for t in kham.segment_tokens("ธนาคาร100แห่ง"):
//!     print(t.text, t.char_start, t.char_end, t.kind)
//!
//! # FTS pipeline: POS, NE, stopwords, synonyms, romanization
//! for t in kham.segment_fts("นายกรัฐมนตรีกินข้าว"):
//!     print(t.text, t.pos, t.ne, t.is_stop, t.roman)
//!
//! # Romanization
//! for t in kham.romanize("กินข้าว"):
//!     print(t.text, t.roman)
//!
//! # Text normalization
//! kham.normalize("ข้้าว")   # → "ข้าว"
//!
//! # Sentence splitting
//! for s in kham.split_sentences("กินข้าวแล้ว! ดื่มน้ำด้วย"):
//!     print(s.text, s.char_start, s.char_end)
//!
//! # Phonetic soundex
//! kham.soundex_word("กาน", "lk82")          # "1600"
//! kham.sounds_like("กาน", "ขาน", "lk82")    # True
//! kham.thai_english_soundex("Robert")        # "671763"
//! kham.sounds_like_cross_lang("Robert", "โรเบิร์ต")  # True
//!
//! # Number conversion
//! kham.number_to_thai_word(1234)             # "หนึ่งพันสองร้อยสามสิบสี่"
//! kham.thai_word_to_number("สองล้าน")        # 2000000  (or None)
//! kham.thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง") # "ธนาคาร100แห่ง"
//! kham.number_to_baht_text(123, 50)          # "หนึ่งร้อยยี่สิบสามบาทห้าสิบสตางค์"
//! amt = kham.parse_baht_text("หนึ่งร้อยบาทถ้วน")
//! if amt: print(amt.baht, amt.satang)        # 100  0
//! ```

use kham_core::{
    fts::FtsTokenizer,
    keyword::KeyExtractor,
    number::{
        parse_thai_baht as core_parse_baht, parse_thai_word as core_parse_thai_word,
        thai_digits_to_ascii as core_thai_digits_to_ascii, to_thai_baht_text as core_to_baht_text,
        u64_to_thai_word as core_number_to_word,
    },
    romanizer::RomanizationMap,
    sentence::split_sentences as core_split,
    soundex::{
        soundex as core_soundex, sounds_like as core_sounds_like,
        sounds_like_cross_lang as core_sounds_like_cross,
        thai_english_soundex as core_thai_english_soundex, SoundexAlgorithm,
    },
    spell::SpellChecker,
    TokenKind, Tokenizer,
};
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn kind_str(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Thai => "Thai",
        TokenKind::Latin => "Latin",
        TokenKind::Number => "Number",
        TokenKind::Punctuation => "Punctuation",
        TokenKind::Emoji => "Emoji",
        TokenKind::Whitespace => "Whitespace",
        TokenKind::Unknown => "Unknown",
        TokenKind::Named(ne) => ne.as_str(),
    }
}

fn parse_algo(algo: &str) -> SoundexAlgorithm {
    match algo {
        "udom83" => SoundexAlgorithm::Udom83,
        "metasound" => SoundexAlgorithm::MetaSound,
        _ => SoundexAlgorithm::Lk82,
    }
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/// A single segmentation token returned by :func:`segment_tokens`.
///
/// Attributes:
///     text (str): Token text.
///     byte_start (int): Start byte offset in the UTF-8 encoded input.
///     byte_end (int): End byte offset in the UTF-8 encoded input.
///     char_start (int): Start Unicode scalar-value offset.
///     char_end (int): End Unicode scalar-value offset.
///     kind (str): ``"Thai"`` | ``"Latin"`` | ``"Number"`` | ``"Punctuation"``
///         | ``"Emoji"`` | ``"Whitespace"`` | ``"Unknown"`` | ``"Person"``
///         | ``"Place"`` | ``"Org"``
#[pyclass(frozen)]
pub struct Token {
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub byte_start: usize,
    #[pyo3(get)]
    pub byte_end: usize,
    #[pyo3(get)]
    pub char_start: usize,
    #[pyo3(get)]
    pub char_end: usize,
    #[pyo3(get)]
    pub kind: String,
}

#[pymethods]
impl Token {
    fn __repr__(&self) -> String {
        format!(
            "Token(text={:?}, byte_span={}..{}, char_span={}..{}, kind={:?})",
            self.text, self.byte_start, self.byte_end, self.char_start, self.char_end, self.kind,
        )
    }
}

// ---------------------------------------------------------------------------
// FtsToken
// ---------------------------------------------------------------------------

/// A token produced by the FTS pipeline, returned by :func:`segment_fts`.
///
/// Attributes:
///     text (str): Token text (may be normalised).
///     position (int): Ordinal index in the non-whitespace token sequence (0-based).
///     kind (str): Same values as :class:`Token`.
///     is_stop (bool): ``True`` if the token is in the built-in stopword list.
///     roman (str): RTGS romanization; equals ``text`` for non-Thai / OOV tokens.
///     pos (str | None): POS tag or ``None`` for OOV / non-Thai.
///         One of ``"Noun"`` | ``"Verb"`` | ``"Adj"`` | ``"Adv"`` |
///         ``"Particle"`` | ``"ProperNoun"`` | ``"Pronoun"`` | ``"Numeral"`` |
///         ``"Classifier"`` | ``"Conjunction"`` | ``"Auxiliary"`` |
///         ``"Determiner"`` | ``"Preposition"``.
///     ne (str | None): Named entity category or ``None``.
///         One of ``"Person"`` | ``"Place"`` | ``"Org"``.
///     synonyms (list[str]): Synonym / number-normalisation expansions (may be empty).
///     trigrams (list[str]): Character trigrams for ``"Unknown"`` tokens; empty otherwise.
#[pyclass(frozen)]
pub struct FtsToken {
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub position: usize,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub is_stop: bool,
    #[pyo3(get)]
    pub roman: String,
    #[pyo3(get)]
    pub pos: Option<String>,
    #[pyo3(get)]
    pub ne: Option<String>,
    #[pyo3(get)]
    pub synonyms: Vec<String>,
    #[pyo3(get)]
    pub trigrams: Vec<String>,
}

#[pymethods]
impl FtsToken {
    fn __repr__(&self) -> String {
        format!(
            "FtsToken(text={:?}, pos={:?}, ne={:?}, is_stop={}, roman={:?})",
            self.text, self.pos, self.ne, self.is_stop, self.roman,
        )
    }
}

// ---------------------------------------------------------------------------
// RomanToken
// ---------------------------------------------------------------------------

/// A token paired with its RTGS romanization, returned by :func:`romanize`.
///
/// Attributes:
///     text (str): Original token text.
///     roman (str): RTGS romanization; equals ``text`` for non-Thai / OOV tokens.
#[pyclass(frozen)]
pub struct RomanToken {
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub roman: String,
}

#[pymethods]
impl RomanToken {
    fn __repr__(&self) -> String {
        format!("RomanToken(text={:?}, roman={:?})", self.text, self.roman)
    }
}

// ---------------------------------------------------------------------------
// Sentence
// ---------------------------------------------------------------------------

/// A sentence span returned by :func:`split_sentences`.
///
/// Attributes:
///     text (str): Sentence text (including terminator if any).
///     char_start (int): Start Unicode scalar-value offset.
///     char_end (int): End Unicode scalar-value offset.
#[pyclass(frozen)]
pub struct Sentence {
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub char_start: usize,
    #[pyo3(get)]
    pub char_end: usize,
}

#[pymethods]
impl Sentence {
    fn __repr__(&self) -> String {
        format!(
            "Sentence(text={:?}, char_span={}..{})",
            self.text, self.char_start, self.char_end,
        )
    }
}

// ---------------------------------------------------------------------------
// BahtAmount
// ---------------------------------------------------------------------------

/// A parsed Baht currency amount returned by :func:`parse_baht_text`.
///
/// Attributes:
///     baht (int): Whole baht.
///     satang (int): Satang (0–99; 100 satang = 1 baht).
#[pyclass(frozen)]
pub struct BahtAmount {
    #[pyo3(get)]
    pub baht: u64,
    #[pyo3(get)]
    pub satang: u8,
}

#[pymethods]
impl BahtAmount {
    fn __repr__(&self) -> String {
        format!("BahtAmount(baht={}, satang={})", self.baht, self.satang)
    }
}

// ---------------------------------------------------------------------------
// Segmentation
// ---------------------------------------------------------------------------

/// Segment Thai text and return a list of token strings.
///
/// Args:
///     text (str): Input text (may contain mixed scripts).
///
/// Returns:
///     list[str]: Token texts, whitespace excluded.
///
/// Example:
///     >>> kham.segment("กินข้าวกับปลา")
///     ['กิน', 'ข้าว', 'กับ', 'ปลา']
#[pyfunction]
fn segment(text: &str) -> Vec<String> {
    Tokenizer::new()
        .segment(text)
        .into_iter()
        .map(|t| t.text.to_owned())
        .collect()
}

/// Segment Thai text and return a list of :class:`Token` objects with span info.
///
/// Args:
///     text (str): Input text (may contain mixed scripts).
///
/// Returns:
///     list[Token]: Rich token objects, whitespace excluded.
///
/// Example:
///     >>> [(t.text, t.kind) for t in kham.segment_tokens("ธนาคาร100แห่ง")]
///     [('ธนาคาร', 'Thai'), ('100', 'Number'), ('แห่ง', 'Thai')]
#[pyfunction]
fn segment_tokens(text: &str) -> Vec<Token> {
    Tokenizer::new()
        .segment(text)
        .into_iter()
        .map(|t| Token {
            text: t.text.to_owned(),
            byte_start: t.span.start,
            byte_end: t.span.end,
            char_start: t.char_span.start,
            char_end: t.char_span.end,
            kind: kind_str(t.kind).to_owned(),
        })
        .collect()
}

/// Segment Thai text through the full FTS pipeline.
///
/// Pipeline: normalize → segment → NE tagging → stopword tagging →
/// POS tagging → synonym expansion → RTGS romanization.
///
/// Args:
///     text (str): Input text.
///
/// Returns:
///     list[FtsToken]: Tokens with NLP metadata, whitespace excluded.
///
/// Example:
///     >>> for t in kham.segment_fts("นายกรัฐมนตรีกินข้าว"):
///     ...     print(t.text, t.pos, t.ne, t.is_stop)
#[pyfunction]
fn segment_fts(text: &str) -> Vec<FtsToken> {
    let roman_map = RomanizationMap::builtin();
    FtsTokenizer::new()
        .segment_for_fts(text)
        .into_iter()
        .map(|t| {
            let roman = roman_map.romanize_or_raw(&t.text).to_owned();
            FtsToken {
                roman,
                text: t.text,
                position: t.position,
                kind: kind_str(t.kind).to_owned(),
                is_stop: t.is_stop,
                synonyms: t.synonyms,
                trigrams: t.trigrams,
                pos: t.pos.map(|p| p.as_str().to_owned()),
                ne: t.ne.map(|n| n.as_str().to_owned()),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Romanization
// ---------------------------------------------------------------------------

/// Segment Thai text and return each token paired with its RTGS romanization.
///
/// Args:
///     text (str): Input text.
///
/// Returns:
///     list[RomanToken]: Each token with ``text`` and ``roman`` fields.
///
/// Example:
///     >>> [(t.text, t.roman) for t in kham.romanize("กินข้าว")]
///     [('กิน', 'kin'), ('ข้าว', 'khao')]
#[pyfunction]
fn romanize(text: &str) -> Vec<RomanToken> {
    let map = RomanizationMap::builtin();
    Tokenizer::new()
        .segment(text)
        .into_iter()
        .map(|t| RomanToken {
            roman: map.romanize_or_raw(t.text).to_owned(),
            text: t.text.to_owned(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/// Normalize Thai text into canonical form.
///
/// Applies two transformations:
///
/// 1. **Duplicate tone marks** — consecutive tone marks are collapsed to the last.
/// 2. **Sara Am composition** — nikhahit (อํ) + sara aa (อา) → sara am (อำ).
///
/// Args:
///     text (str): Input text.
///
/// Returns:
///     str: Normalized text (unchanged if no normalization needed).
///
/// Example:
///     >>> kham.normalize("ข้้าว")
///     'ข้าว'
#[pyfunction]
fn normalize(text: &str) -> String {
    kham_core::normalizer::normalize(text)
}

// ---------------------------------------------------------------------------
// Sentence splitting
// ---------------------------------------------------------------------------

/// Split text into sentences.
///
/// Boundaries: Thai sentence markers (ฯ ๚ ๛), newlines, and Western
/// punctuation (``!`` ``?`` ``.`` followed by space).
///
/// Args:
///     text (str): Input text.
///
/// Returns:
///     list[Sentence]: Each sentence with ``text``, ``char_start``, ``char_end``.
///
/// Example:
///     >>> for s in kham.split_sentences("กินข้าวแล้ว! ดื่มน้ำด้วย"):
///     ...     print(repr(s.text))
#[pyfunction]
fn split_sentences(text: &str) -> Vec<Sentence> {
    core_split(text)
        .into_iter()
        .map(|s| Sentence {
            text: s.text.to_owned(),
            char_start: s.char_span.start,
            char_end: s.char_span.end,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Soundex
// ---------------------------------------------------------------------------

/// Encode a Thai word using the selected phonetic algorithm.
///
/// Args:
///     word (str): Thai word to encode.
///     algo (str): ``"lk82"`` (default) | ``"udom83"`` | ``"metasound"``.
///
/// Returns:
///     str: 4-character code for lk82/udom83; variable length for metasound.
///
/// Example:
///     >>> kham.soundex_word("กาน", "lk82")
///     '1600'
#[pyfunction]
#[pyo3(signature = (word, algo = "lk82"))]
fn soundex_word(word: &str, algo: &str) -> String {
    core_soundex(word, parse_algo(algo))
}

/// Return ``True`` if ``a`` and ``b`` produce the same soundex code.
///
/// Args:
///     a (str): First Thai word.
///     b (str): Second Thai word.
///     algo (str): ``"lk82"`` (default) | ``"udom83"`` | ``"metasound"``.
///
/// Returns:
///     bool
///
/// Example:
///     >>> kham.sounds_like("กาน", "ขาน", "lk82")
///     True
#[pyfunction]
#[pyo3(signature = (a, b, algo = "lk82"))]
fn sounds_like(a: &str, b: &str, algo: &str) -> bool {
    core_sounds_like(a, b, parse_algo(algo))
}

/// Encode a Thai or English word using the Thai–English cross-language soundex
/// (Suwanvisat & Prasitjutrakul 1998).
///
/// Args:
///     word (str): Thai or ASCII word.
///
/// Returns:
///     str: Variable-length phonetic code.
///
/// Example:
///     >>> kham.thai_english_soundex("Robert")
///     '671763'
#[pyfunction]
fn thai_english_soundex(word: &str) -> String {
    core_thai_english_soundex(word)
}

/// Return ``True`` if ``a`` and ``b`` sound alike under the Thai–English
/// cross-language soundex (accepts any mix of Thai and ASCII input).
///
/// Args:
///     a (str): First word (Thai or ASCII).
///     b (str): Second word (Thai or ASCII).
///
/// Returns:
///     bool
///
/// Example:
///     >>> kham.sounds_like_cross_lang("Robert", "Rupert")
///     True
#[pyfunction]
fn sounds_like_cross_lang(a: &str, b: &str) -> bool {
    core_sounds_like_cross(a, b)
}

// ---------------------------------------------------------------------------
// Number conversion
// ---------------------------------------------------------------------------

/// Convert Thai digit characters (๐–๙) in text to ASCII digits.
///
/// Other characters are passed through unchanged.
///
/// Args:
///     text (str): Input text.
///
/// Returns:
///     str: Text with Thai digits replaced by ASCII digits.
///
/// Example:
///     >>> kham.thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง")
///     'ธนาคาร100แห่ง'
#[pyfunction]
fn thai_digits_to_ascii(text: &str) -> String {
    core_thai_digits_to_ascii(text)
}

/// Convert a number to its Thai cardinal word representation.
///
/// Args:
///     n (int): Non-negative integer.
///
/// Returns:
///     str: Thai word, e.g. ``"หนึ่งร้อยยี่สิบสาม"`` for 123.
///
/// Example:
///     >>> kham.number_to_thai_word(1000000)
///     'หนึ่งล้าน'
#[pyfunction]
fn number_to_thai_word(n: u64) -> String {
    core_number_to_word(n)
}

/// Parse a Thai cardinal number word and return its integer value.
///
/// Args:
///     text (str): Thai number word.
///
/// Returns:
///     int | None: The integer value, or ``None`` if not a recognised number word.
///
/// Example:
///     >>> kham.thai_word_to_number("สองล้าน")
///     2000000
///     >>> kham.thai_word_to_number("กินข้าว") is None
///     True
#[pyfunction]
fn thai_word_to_number(text: &str) -> Option<u64> {
    core_parse_thai_word(text)
}

/// Render a Baht amount as Thai currency text.
///
/// Args:
///     baht (int): Whole baht amount.
///     satang (int): Satang (0–99; 100 satang = 1 baht).
///
/// Returns:
///     str: Thai currency text.
///
/// Example:
///     >>> kham.number_to_baht_text(100, 0)
///     'หนึ่งร้อยบาทถ้วน'
///     >>> kham.number_to_baht_text(21, 50)
///     'ยี่สิบเอ็ดบาทห้าสิบสตางค์'
#[pyfunction]
fn number_to_baht_text(baht: u64, satang: u8) -> String {
    core_to_baht_text(baht, satang)
}

/// Parse a Thai Baht currency string into a :class:`BahtAmount`.
///
/// Args:
///     text (str): Thai Baht currency string (must contain ``บาท``).
///
/// Returns:
///     BahtAmount | None: Parsed amount, or ``None`` if the string is not
///     a recognised Thai Baht string.
///
/// Example:
///     >>> amt = kham.parse_baht_text("หนึ่งร้อยบาทถ้วน")
///     >>> amt.baht, amt.satang
///     (100, 0)
///     >>> kham.parse_baht_text("กินข้าว") is None
///     True
#[pyfunction]
fn parse_baht_text(text: &str) -> Option<BahtAmount> {
    core_parse_baht(text).map(|b| BahtAmount {
        baht: b.baht,
        satang: b.satang,
    })
}

// ---------------------------------------------------------------------------
// SpellSuggestion
// ---------------------------------------------------------------------------

/// A spelling suggestion returned by :func:`spell_suggestions`.
///
/// Attributes:
///     word (str): Candidate word from the built-in dictionary.
///     edit_distance (int): Levenshtein distance from the input (0–2).
///     soundex_match (bool): ``True`` if the lk82 phonetic codes match.
///     freq_score (int): TNC corpus frequency; ``0`` if not in the table.
#[pyclass(frozen)]
pub struct SpellSuggestion {
    #[pyo3(get)]
    pub word: String,
    #[pyo3(get)]
    pub edit_distance: u8,
    #[pyo3(get)]
    pub soundex_match: bool,
    #[pyo3(get)]
    pub freq_score: u32,
}

#[pymethods]
impl SpellSuggestion {
    fn __repr__(&self) -> String {
        format!(
            "SpellSuggestion(word={:?}, edit_distance={}, soundex_match={}, freq_score={})",
            self.word, self.edit_distance, self.soundex_match, self.freq_score,
        )
    }
}

/// Return up to ``max_n`` spelling suggestions for ``word``.
///
/// Only candidates with Levenshtein edit distance ≤ 2 are returned. Results
/// are sorted by phonetic match (lk82), then edit distance, then TNC corpus
/// frequency. Returns an empty list for empty input or ``max_n = 0``.
///
/// Args:
///     word (str): Potentially misspelled Thai word.
///     max_n (int): Maximum number of suggestions to return.
///
/// Returns:
///     list[SpellSuggestion]
///
/// Example:
///     >>> suggs = kham.spell_suggestions("กานข้าว", 5)
///     >>> [(s.word, s.edit_distance) for s in suggs]
///     [('กินข้าว', 1), ...]
#[pyfunction]
fn spell_suggestions(word: &str, max_n: usize) -> Vec<SpellSuggestion> {
    SpellChecker::builtin()
        .suggestions(word, max_n)
        .into_iter()
        .map(|s| SpellSuggestion {
            word: s.word,
            edit_distance: s.edit_distance,
            soundex_match: s.soundex_match,
            freq_score: s.freq_score,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Keyword
// ---------------------------------------------------------------------------

/// A keyword extracted from a document, returned by :func:`extract_keywords`.
///
/// Attributes:
///     word (str): Keyword text.
///     score (float): TF × IDF_proxy relevance score. Higher = more distinctive.
///     count (int): Raw occurrence count in the document.
#[pyclass(frozen)]
pub struct Keyword {
    #[pyo3(get)]
    pub word: String,
    #[pyo3(get)]
    pub score: f32,
    #[pyo3(get)]
    pub count: usize,
}

#[pymethods]
impl Keyword {
    fn __repr__(&self) -> String {
        format!(
            "Keyword(word={:?}, score={:.4}, count={})",
            self.word, self.score, self.count,
        )
    }
}

/// Extract up to ``max_n`` keywords from ``text``, ranked by TF × IDF_proxy.
///
/// Stopwords and single-character tokens are excluded. Words rare in the TNC
/// corpus receive a higher score than common function words. Returns an empty
/// list for empty text or ``max_n = 0``.
///
/// Args:
///     text (str): Input document text.
///     max_n (int): Maximum number of keywords to return.
///
/// Returns:
///     list[Keyword]
///
/// Example:
///     >>> kws = kham.extract_keywords("การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัล", 5)
///     >>> [k.word for k in kws]
///     ['ซอฟต์แวร์', 'ดิจิทัล', ...]
#[pyfunction]
fn extract_keywords(text: &str, max_n: usize) -> Vec<Keyword> {
    KeyExtractor::builtin()
        .extract(text, max_n)
        .into_iter()
        .map(|k| Keyword {
            word: k.word,
            score: k.score,
            count: k.count,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

/// kham — Thai word segmentation engine (PyO3 bindings).
#[pymodule]
fn kham(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Classes
    m.add_class::<Token>()?;
    m.add_class::<FtsToken>()?;
    m.add_class::<RomanToken>()?;
    m.add_class::<Sentence>()?;
    m.add_class::<BahtAmount>()?;
    m.add_class::<SpellSuggestion>()?;
    m.add_class::<Keyword>()?;

    // Segmentation
    m.add_function(wrap_pyfunction!(segment, m)?)?;
    m.add_function(wrap_pyfunction!(segment_tokens, m)?)?;
    m.add_function(wrap_pyfunction!(segment_fts, m)?)?;

    // Romanization
    m.add_function(wrap_pyfunction!(romanize, m)?)?;

    // Normalization
    m.add_function(wrap_pyfunction!(normalize, m)?)?;

    // Sentence splitting
    m.add_function(wrap_pyfunction!(split_sentences, m)?)?;

    // Soundex
    m.add_function(wrap_pyfunction!(soundex_word, m)?)?;
    m.add_function(wrap_pyfunction!(sounds_like, m)?)?;
    m.add_function(wrap_pyfunction!(thai_english_soundex, m)?)?;
    m.add_function(wrap_pyfunction!(sounds_like_cross_lang, m)?)?;

    // Number conversion
    m.add_function(wrap_pyfunction!(thai_digits_to_ascii, m)?)?;
    m.add_function(wrap_pyfunction!(number_to_thai_word, m)?)?;
    m.add_function(wrap_pyfunction!(thai_word_to_number, m)?)?;
    m.add_function(wrap_pyfunction!(number_to_baht_text, m)?)?;
    m.add_function(wrap_pyfunction!(parse_baht_text, m)?)?;

    // Spell checking
    m.add_function(wrap_pyfunction!(spell_suggestions, m)?)?;

    // Keyword extraction
    m.add_function(wrap_pyfunction!(extract_keywords, m)?)?;

    Ok(())
}
