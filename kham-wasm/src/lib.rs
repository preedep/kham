//! WebAssembly bindings for kham-core via wasm-bindgen.
//!
//! Build with:
//! ```bash
//! wasm-pack build kham-wasm --target web
//! ```
//!
//! Then from JavaScript / TypeScript:
//! ```js
//! import init, { segment, segment_tokens, segment_fts,
//!                romanize, split_sentences } from "./pkg/kham_wasm.js";
//! await init();
//!
//! // Simple: array of token strings
//! const words = segment("กินข้าวกับปลา");
//! console.log(words); // ["กิน", "ข้าว", "กับ", "ปลา"]
//!
//! // Rich: Token objects with span information
//! const tokens = segment_tokens("ธนาคาร100แห่ง");
//! for (const t of tokens) {
//!     console.log(t.text, t.char_start, t.char_end, t.kind);
//! }
//!
//! // FTS: rich NLP metadata per token (pos, ne, is_stop, roman, synonyms)
//! const ftsToks = segment_fts("นายกรัฐมนตรีกินข้าว");
//! for (const t of ftsToks) {
//!     console.log(t.text, t.pos, t.ne, t.roman, t.is_stop);
//! }
//!
//! // Romanization only
//! const roman = romanize("กินข้าว");
//! // [{text:"กิน", roman:"kin"}, {text:"ข้าว", roman:"khao"}]
//!
//! // Sentence splitting
//! const sents = split_sentences("กินข้าว ดื่มน้ำ\nนอนหลับ");
//! for (const s of sents) { console.log(s.text, s.char_start, s.char_end); }
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
// WasmToken is the wasm-bindgen Token struct defined below.
type WasmToken = Token;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Token kind helper
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

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/// A single segmentation token with text and span information.
///
/// JavaScript note: `char_start`/`char_end` are Unicode scalar-value offsets.
/// For BMP-only text (no surrogate pairs) these equal JavaScript's
/// `string.slice()` indices. For text containing surrogate-pair emoji,
/// JavaScript's string indices differ from scalar-value counts — use
/// `byte_start`/`byte_end` with a `TextEncoder` for precise slicing.
#[wasm_bindgen]
pub struct Token {
    text: String,
    byte_start: usize,
    byte_end: usize,
    char_start: usize,
    char_end: usize,
    kind: &'static str,
    /// Segmentation confidence in `[0.0, 1.0]`. `0.0` = unknown, `1.0` = high-confidence dict match.
    pub confidence: f32,
}

#[wasm_bindgen]
impl Token {
    /// The token text.
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    /// Start byte offset in the UTF-8 encoded input string.
    #[wasm_bindgen(getter)]
    pub fn byte_start(&self) -> usize {
        self.byte_start
    }

    /// End byte offset in the UTF-8 encoded input string.
    #[wasm_bindgen(getter)]
    pub fn byte_end(&self) -> usize {
        self.byte_end
    }

    /// Start Unicode scalar-value (char) offset in the input string.
    #[wasm_bindgen(getter)]
    pub fn char_start(&self) -> usize {
        self.char_start
    }

    /// End Unicode scalar-value (char) offset in the input string.
    #[wasm_bindgen(getter)]
    pub fn char_end(&self) -> usize {
        self.char_end
    }

    /// Token kind: `"Thai"`, `"Latin"`, `"Number"`, `"Punctuation"`,
    /// `"Emoji"`, `"Whitespace"`, or `"Unknown"`.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.to_owned()
    }
}

// ---------------------------------------------------------------------------
// FtsToken
// ---------------------------------------------------------------------------

/// A token produced by the FTS pipeline with full NLP metadata.
///
/// - `text` — token string (normalized)
/// - `position` — ordinal index in the non-whitespace token sequence (0-based)
/// - `kind` — script/category string; Named entity tokens use `"Person"` /
///   `"Place"` / `"Org"` instead of `"Thai"`
/// - `is_stop` — `true` if the token is in the built-in stopword list
/// - `roman` — RTGS romanization of the token text; equals `text` for
///   non-Thai tokens (Latin, Number, etc.) and unknown Thai words
/// - `pos` — ORCHID-derived POS tag string, or `null` if OOV / non-Thai
///   (`"Noun"` | `"Verb"` | `"Adj"` | `"Adv"` | `"Particle"` | `"ProperNoun"`
///   | `"Pronoun"` | `"Numeral"` | `"Classifier"` | `"Conjunction"`
///   | `"Auxiliary"` | `"Determiner"` | `"Preposition"`)
/// - `ne` — named entity category string, or `null`
///   (`"Person"` | `"Place"` | `"Org"`)
/// - `synonyms` — synonym / number-normalisation expansions (may be empty)
/// - `trigrams` — character trigrams for `Unknown` tokens; empty otherwise
#[wasm_bindgen]
pub struct FtsToken {
    text: String,
    position: usize,
    kind: &'static str,
    is_stop: bool,
    roman: String,
    synonyms: Vec<String>,
    trigrams: Vec<String>,
    pos: Option<&'static str>,
    ne: Option<&'static str>,
    /// Segmentation confidence in `[0.0, 1.0]`. `0.0` = unknown, `1.0` = high-confidence dict match.
    pub confidence: f32,
}

#[wasm_bindgen]
impl FtsToken {
    /// The token text (may be normalised relative to the raw input).
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    /// Ordinal position in the non-whitespace token sequence (0-based).
    #[wasm_bindgen(getter)]
    pub fn position(&self) -> usize {
        self.position
    }

    /// Token kind string.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.to_owned()
    }

    /// `true` if this token matches the built-in Thai stopword list.
    #[wasm_bindgen(getter)]
    pub fn is_stop(&self) -> bool {
        self.is_stop
    }

    /// RTGS romanization of the token. Equals the original text for non-Thai
    /// or out-of-vocabulary tokens.
    #[wasm_bindgen(getter)]
    pub fn roman(&self) -> String {
        self.roman.clone()
    }

    /// Synonym expansions (empty array if none).
    #[wasm_bindgen(getter)]
    pub fn synonyms(&self) -> Vec<JsValue> {
        self.synonyms.iter().map(|s| JsValue::from_str(s)).collect()
    }

    /// Character trigrams for `Unknown` tokens; empty array for all other kinds.
    #[wasm_bindgen(getter)]
    pub fn trigrams(&self) -> Vec<JsValue> {
        self.trigrams.iter().map(|s| JsValue::from_str(s)).collect()
    }

    /// POS tag string, or `null` if OOV or non-Thai.
    #[wasm_bindgen(getter)]
    pub fn pos(&self) -> Option<String> {
        self.pos.map(|s| s.to_owned())
    }

    /// Named entity category string, or `null` if not in the NE gazetteer.
    #[wasm_bindgen(getter)]
    pub fn ne(&self) -> Option<String> {
        self.ne.map(|s| s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// RomanToken
// ---------------------------------------------------------------------------

/// A token paired with its RTGS romanization, returned by [`romanize`].
#[wasm_bindgen]
pub struct RomanToken {
    text: String,
    roman: String,
}

#[wasm_bindgen]
impl RomanToken {
    /// The original token text.
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    /// RTGS romanization. Equals `text` for non-Thai or out-of-vocabulary tokens.
    #[wasm_bindgen(getter)]
    pub fn roman(&self) -> String {
        self.roman.clone()
    }
}

// ---------------------------------------------------------------------------
// Sentence
// ---------------------------------------------------------------------------

/// A sentence span returned by [`split_sentences`].
#[wasm_bindgen]
pub struct Sentence {
    text: String,
    char_start: usize,
    char_end: usize,
}

#[wasm_bindgen]
impl Sentence {
    /// The sentence text (zero-copy slice of the input, including terminator).
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    /// Start Unicode scalar-value (char) offset in the source string.
    #[wasm_bindgen(getter)]
    pub fn char_start(&self) -> usize {
        self.char_start
    }

    /// End Unicode scalar-value (char) offset in the source string.
    #[wasm_bindgen(getter)]
    pub fn char_end(&self) -> usize {
        self.char_end
    }
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Segment Thai text and return an array of token strings.
///
/// Whitespace tokens are excluded from the output.
#[wasm_bindgen]
pub fn segment(text: &str) -> Vec<JsValue> {
    Tokenizer::new()
        .segment(text)
        .into_iter()
        .map(|t| JsValue::from_str(t.text))
        .collect()
}

/// Segment Thai text and return an array of [`Token`] objects with full span
/// information (`text`, `byte_start`/`byte_end`, `char_start`/`char_end`, `kind`).
///
/// Whitespace tokens are excluded from the output.
#[wasm_bindgen]
pub fn segment_tokens(text: &str) -> Vec<Token> {
    Tokenizer::new()
        .segment(text)
        .into_iter()
        .map(|t| Token {
            text: t.text.to_owned(),
            byte_start: t.span.start,
            byte_end: t.span.end,
            char_start: t.char_span.start,
            char_end: t.char_span.end,
            kind: kind_str(t.kind),
            confidence: t.confidence,
        })
        .collect()
}

/// Segment Thai text through the full FTS pipeline and return an array of
/// [`FtsToken`] objects with NLP metadata.
///
/// The built-in pipeline includes: text normalisation, word segmentation,
/// named-entity recognition, stopword tagging, POS tagging, synonym expansion
/// (number normalisation), and RTGS romanization. Whitespace tokens are
/// excluded.
#[wasm_bindgen]
pub fn segment_fts(text: &str) -> Vec<FtsToken> {
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
                kind: kind_str(t.kind),
                is_stop: t.is_stop,
                synonyms: t.synonyms,
                trigrams: t.trigrams,
                pos: t.pos.map(|p| p.as_str()),
                ne: t.ne.map(|n| n.as_str()),
                confidence: t.confidence,
            }
        })
        .collect()
}

/// Segment Thai text and return each token paired with its RTGS romanization.
///
/// For non-Thai tokens (Latin, Number, Punctuation) and unknown Thai words,
/// `roman` equals the original `text`. Whitespace tokens are excluded.
#[wasm_bindgen]
pub fn romanize(text: &str) -> Vec<RomanToken> {
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

/// Normalise Thai text and return the canonical form.
///
/// Applies two transformations:
/// 1. **Duplicate tone marks** — consecutive tone marks (อ่ อ้ อ๊ อ๋) are
///    collapsed to the last one (e.g. `ก่้` → `ก้`).
/// 2. **Sara Am composition** — nikhahit + sara aa (`อํ` + `อา`) is
///    composed into sara am (`อำ`).
///
/// Returns the input unchanged if no normalisation is needed.
#[wasm_bindgen]
pub fn normalize(text: &str) -> String {
    kham_core::normalizer::normalize(text)
}

/// Encode a single Thai word using the selected phonetic algorithm.
///
/// `algo` must be one of `"lk82"` (default), `"udom83"`, or `"metasound"`.
/// Unknown values fall back to `"lk82"`.
///
/// - `lk82` / `udom83` — always returns a 4-character ASCII code.
/// - `metasound` — returns 3 characters per syllable (variable length).
///
/// Returns `"0000"` / `"000"` if the word contains no Thai consonants.
#[wasm_bindgen]
pub fn soundex_word(word: &str, algo: &str) -> String {
    let algorithm = match algo {
        "udom83" => SoundexAlgorithm::Udom83,
        "metasound" => SoundexAlgorithm::MetaSound,
        _ => SoundexAlgorithm::Lk82,
    };
    core_soundex(word, algorithm)
}

/// Return `true` if `a` and `b` produce the same soundex code under `algo`.
///
/// `algo` must be `"lk82"` (default), `"udom83"`, or `"metasound"`.
#[wasm_bindgen]
pub fn sounds_like(a: &str, b: &str, algo: &str) -> bool {
    let algorithm = match algo {
        "udom83" => SoundexAlgorithm::Udom83,
        "metasound" => SoundexAlgorithm::MetaSound,
        _ => SoundexAlgorithm::Lk82,
    };
    core_sounds_like(a, b, algorithm)
}

/// Encode a Thai or English word using the Thai–English cross-language soundex
/// (Suwanvisat & Prasitjutrakul 1998).
///
/// Accepts both Thai script and ASCII; the same table is applied, so
/// `thai_english_soundex("Robert")` and `thai_english_soundex("โรเบิร์ต")`
/// share a common prefix.
#[wasm_bindgen]
pub fn thai_english_soundex(word: &str) -> String {
    core_thai_english_soundex(word)
}

/// Return `true` if `a` and `b` sound alike under the Thai–English
/// cross-language soundex (Suwanvisat & Prasitjutrakul 1998).
///
/// Accepts any mix of Thai and ASCII input.
#[wasm_bindgen]
pub fn sounds_like_cross_lang(a: &str, b: &str) -> bool {
    core_sounds_like_cross(a, b)
}

/// Split text into sentences and return an array of [`Sentence`] objects.
///
/// Each sentence carries `text`, `char_start`, and `char_end`
/// (Unicode scalar-value offsets into the original string).
///
/// Sentence boundaries are detected on Thai sentence-final markers
/// (ฯ, ๛, ๚), newlines, and common Western punctuation (`.` `!` `?`),
/// with decimal/abbreviation protection to avoid false splits.
#[wasm_bindgen]
pub fn split_sentences(text: &str) -> Vec<Sentence> {
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
// BahtResult
// ---------------------------------------------------------------------------

/// Result of parsing a Thai Baht currency string via [`parse_baht_text`].
/// Always check [`BahtResult::valid`] before using [`BahtResult::baht`] /
/// [`BahtResult::satang`].
#[wasm_bindgen]
pub struct BahtResult {
    baht: u64,
    satang: u8,
    valid: bool,
}

#[wasm_bindgen]
impl BahtResult {
    /// Whole baht amount.
    #[wasm_bindgen(getter)]
    pub fn baht(&self) -> u64 {
        self.baht
    }

    /// Satang (0–99; 100 satang = 1 baht).
    #[wasm_bindgen(getter)]
    pub fn satang(&self) -> u8 {
        self.satang
    }

    /// `true` if the input was a valid Thai Baht string.
    #[wasm_bindgen(getter)]
    pub fn valid(&self) -> bool {
        self.valid
    }
}

// ---------------------------------------------------------------------------
// Number conversion functions
// ---------------------------------------------------------------------------

/// Convert Thai digit characters (๐–๙) in `text` to ASCII digits.
/// Non-Thai characters are passed through unchanged.
#[wasm_bindgen]
pub fn thai_digits_to_ascii(text: &str) -> String {
    core_thai_digits_to_ascii(text)
}

/// Convert a number to its Thai cardinal word representation.
///
/// e.g. `123` → `"หนึ่งร้อยยี่สิบสาม"`, `0` → `"ศูนย์"`.
#[wasm_bindgen]
pub fn number_to_thai_word(n: u64) -> String {
    core_number_to_word(n)
}

/// Parse a Thai cardinal number word and return its decimal string.
///
/// Returns an empty string when the input is not a recognised number word.
/// e.g. `"หนึ่งร้อยยี่สิบสาม"` → `"123"`, `"กินข้าว"` → `""`.
#[wasm_bindgen]
pub fn thai_word_to_number(text: &str) -> String {
    match core_parse_thai_word(text) {
        Some(n) => n.to_string(),
        None => String::new(),
    }
}

/// Render a Baht amount as Thai currency text.
///
/// `satang` must be 0–99 (100 satang = 1 baht).
/// e.g. `(123, 50)` → `"หนึ่งร้อยยี่สิบสามบาทห้าสิบสตางค์"`,
///      `(100, 0)` → `"หนึ่งร้อยบาทถ้วน"`.
#[wasm_bindgen]
pub fn number_to_baht_text(baht: u64, satang: u8) -> String {
    core_to_baht_text(baht, satang)
}

/// Parse a Thai Baht currency string into a [`BahtResult`].
///
/// Returns a result with `valid = false` when the input is not a recognised
/// Thai Baht string (no `บาท` marker, or unrecognised number words).
#[wasm_bindgen]
pub fn parse_baht_text(text: &str) -> BahtResult {
    match core_parse_baht(text) {
        Some(b) => BahtResult {
            baht: b.baht,
            satang: b.satang,
            valid: true,
        },
        None => BahtResult {
            baht: 0,
            satang: 0,
            valid: false,
        },
    }
}

// ---------------------------------------------------------------------------
// SpellSuggestion
// ---------------------------------------------------------------------------

/// A spelling suggestion returned by [`spell_suggestions`].
#[wasm_bindgen]
pub struct SpellSuggestion {
    word: String,
    edit_distance: u8,
    soundex_match: bool,
    freq_score: u32,
}

#[wasm_bindgen]
impl SpellSuggestion {
    /// The candidate word from the built-in dictionary.
    #[wasm_bindgen(getter)]
    pub fn word(&self) -> String {
        self.word.clone()
    }

    /// Levenshtein edit distance between the input and this candidate (0–2).
    #[wasm_bindgen(getter)]
    pub fn edit_distance(&self) -> u8 {
        self.edit_distance
    }

    /// `true` if the lk82 phonetic codes of the input and candidate match.
    #[wasm_bindgen(getter)]
    pub fn soundex_match(&self) -> bool {
        self.soundex_match
    }

    /// TNC corpus frequency score; `0` if the word is not in the frequency table.
    #[wasm_bindgen(getter)]
    pub fn freq_score(&self) -> u32 {
        self.freq_score
    }
}

/// Return up to `max_n` spelling suggestions for `word`.
///
/// Only candidates with Levenshtein edit distance ≤ 2 are returned.
/// Results are sorted by phonetic match (lk82), then edit distance, then
/// TNC corpus frequency. Returns an empty array for empty input or `max_n = 0`.
#[wasm_bindgen]
pub fn spell_suggestions(word: &str, max_n: usize) -> Vec<SpellSuggestion> {
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

/// A keyword extracted from a document, returned by [`extract_keywords`].
#[wasm_bindgen]
pub struct Keyword {
    word: String,
    score: f32,
    count: usize,
}

#[wasm_bindgen]
impl Keyword {
    /// The keyword text.
    #[wasm_bindgen(getter)]
    pub fn word(&self) -> String {
        self.word.clone()
    }

    /// TF × IDF_proxy relevance score. Higher means more document-distinctive.
    #[wasm_bindgen(getter)]
    pub fn score(&self) -> f32 {
        self.score
    }

    /// Raw occurrence count of this word in the document.
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.count
    }
}

/// Extract up to `max_n` keywords from `text`, ranked by TF × IDF_proxy.
///
/// Stopwords and single-character tokens are excluded. Words rare in the TNC
/// corpus score higher than common function words. Returns an empty array
/// for empty text or `max_n = 0`.
#[wasm_bindgen]
pub fn extract_keywords(text: &str, max_n: usize) -> Vec<Keyword> {
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

/// Segment Thai text and return only tokens whose segmentation confidence
/// meets or exceeds `min_confidence`.
///
/// A convenience filter over [`segment_tokens`]: calls `segment_stream`
/// internally and collects tokens passing `next_above_confidence`.
///
/// Pass `0.0` to include all tokens; `1.0` for only the highest-confidence
/// dictionary matches. Whitespace tokens that have confidence < min_confidence
/// are also excluded.
#[wasm_bindgen]
pub fn segment_above_confidence(text: &str, min_confidence: f32) -> Vec<WasmToken> {
    let tokenizer = Tokenizer::new();
    let mut stream = tokenizer.segment_stream(text);
    let mut result = Vec::new();
    while let Some(t) = stream.next_above_confidence(min_confidence) {
        result.push(WasmToken {
            text: t.text.to_owned(),
            byte_start: t.span.start,
            byte_end: t.span.end,
            char_start: t.char_span.start,
            char_end: t.char_span.end,
            kind: kind_str(t.kind),
            confidence: t.confidence,
        });
    }
    result
}

// ---------------------------------------------------------------------------
// Tests (native only — skip on wasm32 where std tests don't run)
// ---------------------------------------------------------------------------

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    // --- normalize ---

    #[test]
    fn normalize_unchanged_when_already_normal() {
        assert_eq!(normalize("กินข้าว"), "กินข้าว");
    }

    // --- soundex_word ---

    #[test]
    fn soundex_word_lk82_is_four_chars() {
        assert_eq!(soundex_word("กาน", "lk82").len(), 4);
    }

    #[test]
    fn soundex_word_udom83_is_four_chars() {
        assert_eq!(soundex_word("กาน", "udom83").len(), 4);
    }

    #[test]
    fn soundex_word_unknown_algo_falls_back_to_lk82() {
        assert_eq!(soundex_word("กาน", "lk82"), soundex_word("กาน", "unknown"));
    }

    // --- sounds_like ---

    #[test]
    fn sounds_like_homophones() {
        assert!(sounds_like("กาน", "ขาน", "lk82"));
    }

    #[test]
    fn sounds_like_different_initial_groups() {
        assert!(!sounds_like("กิน", "มิน", "lk82"));
    }

    // --- thai_english_soundex / sounds_like_cross_lang ---

    #[test]
    fn thai_english_soundex_returns_non_empty() {
        assert!(!thai_english_soundex("กิน").is_empty());
    }

    #[test]
    fn sounds_like_cross_lang_identical_words() {
        assert!(sounds_like_cross_lang("กิน", "กิน"));
    }

    // --- thai_digits_to_ascii ---

    #[test]
    fn thai_digits_to_ascii_converts_all() {
        assert_eq!(thai_digits_to_ascii("๑๒๓"), "123");
    }

    #[test]
    fn thai_digits_to_ascii_mixed_passes_through_ascii() {
        assert_eq!(thai_digits_to_ascii("1๒3"), "123");
    }

    // --- number_to_thai_word / thai_word_to_number ---

    #[test]
    fn number_to_thai_word_zero() {
        assert_eq!(number_to_thai_word(0), "ศูนย์");
    }

    #[test]
    fn thai_word_to_number_roundtrip() {
        let word = number_to_thai_word(100);
        assert_eq!(thai_word_to_number(&word), "100");
    }

    #[test]
    fn thai_word_to_number_invalid_returns_empty() {
        assert_eq!(thai_word_to_number("กินข้าว"), "");
    }

    // --- number_to_baht_text / parse_baht_text ---

    #[test]
    fn parse_baht_text_roundtrip() {
        let text = number_to_baht_text(100, 0);
        let result = parse_baht_text(&text);
        assert!(result.valid());
        assert_eq!(result.baht(), 100);
        assert_eq!(result.satang(), 0);
    }

    #[test]
    fn parse_baht_text_invalid_input() {
        assert!(!parse_baht_text("กินข้าว").valid());
    }

    // --- segment_tokens ---

    #[test]
    fn segment_tokens_basic_thai() {
        let tokens = segment_tokens("กินข้าว");
        assert!(!tokens.is_empty());
        for t in &tokens {
            assert!(!t.text().is_empty());
            assert_eq!(t.kind(), "Thai");
            assert!(t.byte_end() > t.byte_start());
            assert!(t.char_end() > t.char_start());
        }
    }

    #[test]
    fn segment_tokens_mixed_script_has_number_kind() {
        let tokens = segment_tokens("ธนาคาร100แห่ง");
        let kinds: Vec<String> = tokens.iter().map(|t| t.kind()).collect();
        assert!(kinds.contains(&"Thai".to_string()));
        assert!(kinds.contains(&"Number".to_string()));
    }

    #[test]
    fn segment_tokens_byte_span_matches_text() {
        let src = "กินข้าว";
        let tokens = segment_tokens(src);
        for t in &tokens {
            let sliced = &src.as_bytes()[t.byte_start()..t.byte_end()];
            assert_eq!(std::str::from_utf8(sliced).unwrap(), t.text());
        }
    }

    // --- romanize ---

    #[test]
    fn romanize_produces_kin_for_gin_token() {
        let tokens = romanize("กิน");
        assert!(
            tokens.iter().any(|t| t.roman() == "kin"),
            "expected roman='kin' in {tokens:?}",
            tokens = tokens.iter().map(|t| t.roman()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn romanize_text_field_joins_to_original() {
        let tokens = romanize("กินข้าว");
        let joined: String = tokens.iter().map(|t| t.text()).collect();
        assert_eq!(joined, "กินข้าว");
    }

    // --- split_sentences ---

    #[test]
    fn split_sentences_on_newline_yields_two_sentences() {
        let sents = split_sentences("กินข้าว\nดื่มน้ำ");
        assert!(
            sents.len() >= 2,
            "expected >= 2 sentences, got {}",
            sents.len()
        );
        for s in &sents {
            assert!(!s.text().is_empty());
            assert!(s.char_end() > s.char_start());
        }
    }

    // --- segment_fts ---

    #[test]
    fn segment_fts_position_is_sequential() {
        let tokens = segment_fts("กินข้าวกับปลา");
        for (i, t) in tokens.iter().enumerate() {
            assert_eq!(t.position(), i, "position mismatch at index {i}");
        }
    }

    #[test]
    fn segment_fts_all_tokens_have_kind_and_roman() {
        let tokens = segment_fts("กินข้าว");
        assert!(!tokens.is_empty());
        for t in &tokens {
            assert!(!t.kind().is_empty(), "kind must not be empty");
            assert!(!t.roman().is_empty(), "roman must not be empty");
        }
    }

    #[test]
    fn segment_fts_stopword_flagged() {
        // "กับ" is a Thai conjunction and a built-in stopword
        let tokens = segment_fts("กินข้าวกับปลา");
        assert!(
            tokens.iter().any(|t| t.is_stop()),
            "expected at least one stopword token"
        );
    }
}
