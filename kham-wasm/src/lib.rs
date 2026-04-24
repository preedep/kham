//! WebAssembly bindings for kham-core via wasm-bindgen.
//!
//! Build with:
//! ```bash
//! wasm-pack build kham-wasm --target web
//! ```
//!
//! Then from JavaScript / TypeScript:
//! ```js
//! import init, { segment, segment_tokens } from "./pkg/kham_wasm.js";
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
//! ```

use kham_core::{TokenKind, Tokenizer};
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
