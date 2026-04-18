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
//! tokens = kham.segment("กินข้าวกับปลา")
//! print(tokens)  # ['กิน', 'ข้าว', 'กับ', 'ปลา']
//!
//! # Rich: Token objects with span information
//! tokens = kham.segment_tokens("ธนาคาร100แห่ง")
//! for t in tokens:
//!     print(t.text, t.char_start, t.char_end, t.kind)
//! # ธนาคาร 0 6 Thai
//! # 100    6 9 Number
//! # แห่ง   9 13 Thai
//! ```

use kham_core::{TokenKind, Tokenizer};
use pyo3::prelude::*;

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
    }
}

// ---------------------------------------------------------------------------
// PyToken
// ---------------------------------------------------------------------------

/// A single segmentation token with text and span information.
///
/// Attributes:
///     text (str): The token text (a copy of the relevant slice of the input).
///     byte_start (int): Start byte offset in the original input string.
///     byte_end (int): End byte offset in the original input string.
///     char_start (int): Start character offset (Unicode scalar values).
///     char_end (int): End character offset (Unicode scalar values).
///     kind (str): Token kind — one of ``"Thai"``, ``"Latin"``, ``"Number"``,
///         ``"Punctuation"``, ``"Emoji"``, ``"Whitespace"``, ``"Unknown"``.
///
/// Example:
///     >>> tokens = kham.segment_tokens("ธนาคาร100แห่ง")
///     >>> tokens[0].text, tokens[0].char_start, tokens[0].char_end
///     ('ธนาคาร', 0, 6)
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
// Public functions
// ---------------------------------------------------------------------------

/// Segment Thai text into a list of token strings.
///
/// Args:
///     text (str): Input text (may contain mixed scripts).
///
/// Returns:
///     list[str]: Token texts, whitespace excluded.
///
/// Example:
///     >>> import kham
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

/// Segment Thai text into a list of Token objects with full span information.
///
/// Each token carries ``text``, ``byte_start``/``byte_end`` (byte offsets),
/// ``char_start``/``char_end`` (character offsets), and ``kind``.
///
/// Args:
///     text (str): Input text (may contain mixed scripts).
///
/// Returns:
///     list[Token]: Rich token objects, whitespace excluded.
///
/// Example:
///     >>> tokens = kham.segment_tokens("ธนาคาร100แห่ง")
///     >>> [(t.text, t.char_start, t.char_end, t.kind) for t in tokens]
///     [('ธนาคาร', 0, 6, 'Thai'), ('100', 6, 9, 'Number'), ('แห่ง', 9, 13, 'Thai')]
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

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

/// kham — Thai word segmentation engine.
#[pymodule]
fn kham(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(segment, m)?)?;
    m.add_function(wrap_pyfunction!(segment_tokens, m)?)?;
    m.add_class::<Token>()?;
    Ok(())
}
