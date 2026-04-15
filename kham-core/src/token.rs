//! Token types returned by the segmenter.

use core::ops::Range;

/// Classification of a [`Token`]'s script / category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Thai script syllable or word.
    Thai,
    /// Latin / ASCII alphabetic text.
    Latin,
    /// Numeric digits (ASCII or Thai ๐–๙).
    Number,
    /// Punctuation or symbol.
    Punctuation,
    /// Emoji character sequence.
    Emoji,
    /// Whitespace (space, tab, newline).
    Whitespace,
    /// Anything that does not fit the above categories.
    Unknown,
}

/// A single token produced by [`crate::Tokenizer::segment`].
///
/// The `text` field is a **zero-copy** slice of the original input string.
///
/// # Example
///
/// ```rust
/// use kham_core::Tokenizer;
///
/// let tok = Tokenizer::new();
/// let input = "ธนาคาร100แห่ง";
/// let tokens = tok.segment(input);
/// // Every token's text must be a valid sub-slice of `input`.
/// for t in &tokens {
///     assert!(input.contains(t.text));
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// Zero-copy reference into the original input.
    pub text: &'a str,
    /// Byte offsets `start..end` in the original input string.
    /// Both boundaries are valid UTF-8 code-point boundaries.
    pub span: Range<usize>,
    /// Script / category of this token.
    pub kind: TokenKind,
}

impl<'a> Token<'a> {
    /// Construct a new [`Token`].
    ///
    /// # Panics (debug only)
    ///
    /// Panics in debug builds if `span` does not align with `text` inside `source`.
    #[inline]
    pub fn new(text: &'a str, span: Range<usize>, kind: TokenKind) -> Self {
        debug_assert_eq!(text.len(), span.end - span.start);
        Self { text, span, kind }
    }

    /// Byte length of this token's text.
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.span.end - self.span.start
    }
}
