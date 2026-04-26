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
    fts::FtsTokenizer, romanizer::RomanizationMap, sentence::split_sentences as core_split,
    TokenKind, Tokenizer,
};
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
