//! WebAssembly bindings for kham-core via wasm-bindgen.
//!
//! Build with:
//! ```bash
//! wasm-pack build kham-wasm --target web
//! ```
//!
//! Then from JavaScript / TypeScript:
//! ```js
//! import init, { segment, segment_tokens, segment_fts } from "./pkg/kham_wasm.js";
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
//! // ธนาคาร 0 6 Thai
//! // 100    6 9 Number
//! // แห่ง   9 13 Thai
//!
//! // FTS: rich NLP metadata per token
//! const ftsToks = segment_fts("นายกรัฐมนตรีกินข้าว");
//! for (const t of ftsToks) {
//!     console.log(t.text, t.kind, t.pos, t.ne, t.is_stop);
//! }
//! ```

use kham_core::{fts::FtsTokenizer, TokenKind, Tokenizer};
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
    /// Named entity tokens (reachable via the FTS API) use `"Person"`,
    /// `"Place"`, or `"Org"` instead of `"Thai"`.
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
/// Fields:
/// - `text` — token string (normalized)
/// - `position` — ordinal index in the non-whitespace token sequence (0-based)
/// - `kind` — same values as [`Token::kind`]
/// - `is_stop` — `true` if this token is in the stopword list
/// - `synonyms` — synonym expansions and number / soundex variants (may be empty)
/// - `trigrams` — character trigrams for Unknown tokens; empty for all other kinds
/// - `pos` — ORCHID-derived POS tag string, or `null` if OOV / non-Thai
///   (`"Noun"` | `"Verb"` | `"Adj"` | `"Adv"` | `"Particle"` | `"ProperNoun"`
///   | `"Pronoun"` | `"Numeral"` | `"Classifier"` | `"Conjunction"`
///   | `"Auxiliary"` | `"Determiner"` | `"Preposition"`)
/// - `ne` — named entity category, or `null` if not recognised
///   (`"Person"` | `"Place"` | `"Org"`)
#[wasm_bindgen]
pub struct FtsToken {
    text: String,
    position: usize,
    kind: &'static str,
    is_stop: bool,
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

    /// Token kind string — same values as [`Token::kind`].
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.to_owned()
    }

    /// `true` if this token matches the built-in Thai stopword list.
    #[wasm_bindgen(getter)]
    pub fn is_stop(&self) -> bool {
        self.is_stop
    }

    /// Synonym expansions (empty array if none). Includes number normalizations
    /// and soundex codes when those pipeline stages are active.
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
// Public functions
// ---------------------------------------------------------------------------

/// Segment Thai text and return an array of token strings.
///
/// Mixed-script input (Thai + Latin + numbers) is handled correctly.
/// Whitespace tokens are excluded from the output.
///
/// # Arguments
///
/// * `text` — Input string (valid UTF-8; JavaScript strings are always UTF-8 safe).
///
/// # Returns
///
/// A JavaScript `Array` of `string` token values.
#[wasm_bindgen]
pub fn segment(text: &str) -> Vec<JsValue> {
    Tokenizer::new()
        .segment(text)
        .into_iter()
        .map(|t| JsValue::from_str(t.text))
        .collect()
}

/// Segment Thai text and return an array of [`Token`] objects with full span
/// information.
///
/// Each token carries `text`, `byte_start`/`byte_end` (UTF-8 byte offsets),
/// `char_start`/`char_end` (Unicode scalar-value offsets), and `kind`.
/// Whitespace tokens are excluded from the output.
///
/// # Arguments
///
/// * `text` — Input string (valid UTF-8).
///
/// # Returns
///
/// A JavaScript `Array` of [`Token`] objects.
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
/// named-entity recognition, stopword tagging, POS tagging, and synonym
/// expansion (number normalisation). Whitespace tokens are excluded.
///
/// # Arguments
///
/// * `text` — Input string (valid UTF-8).
///
/// # Returns
///
/// A JavaScript `Array` of [`FtsToken`] objects.
#[wasm_bindgen]
pub fn segment_fts(text: &str) -> Vec<FtsToken> {
    FtsTokenizer::new()
        .segment_for_fts(text)
        .into_iter()
        .map(|t| FtsToken {
            text: t.text,
            position: t.position,
            kind: kind_str(t.kind),
            is_stop: t.is_stop,
            synonyms: t.synonyms,
            trigrams: t.trigrams,
            pos: t.pos.map(|p| p.as_str()),
            ne: t.ne.map(|n| n.as_str()),
        })
        .collect()
}
