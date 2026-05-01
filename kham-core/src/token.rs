//! Token types returned by the segmenter.

use core::ops::Range;

/// Named entity category assigned by the NE gazetteer.
///
/// Used as the payload of [`TokenKind::Named`]. Stored here (rather than in
/// `ne.rs`) to keep [`TokenKind`] self-contained and avoid circular module
/// dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedEntityKind {
    /// A person — individual, title used as name, or prominent public figure.
    Person,
    /// A place — country, province, city, or geographic region.
    Place,
    /// An organisation — company, government body, institution, or brand.
    Org,
}

impl NamedEntityKind {
    /// Parse the TSV tag string (`"PERSON"`, `"PLACE"`, `"ORG"`).
    ///
    /// Returns `None` for unrecognised strings.
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "PERSON" => Some(Self::Person),
            "PLACE" => Some(Self::Place),
            "ORG" => Some(Self::Org),
            _ => None,
        }
    }

    /// The canonical TSV tag string for this variant.
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::Person => "PERSON",
            Self::Place => "PLACE",
            Self::Org => "ORG",
        }
    }

    /// A human-readable label for use in bindings (`"Person"`, `"Place"`, `"Org"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "Person",
            Self::Place => "Place",
            Self::Org => "Org",
        }
    }
}

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
    /// A named entity identified by the NE gazetteer.
    ///
    /// The segmenter emits [`Thai`](TokenKind::Thai) for all Thai tokens;
    /// [`NeTagger::tag_tokens`](crate::ne::NeTagger::tag_tokens) relabels
    /// gazetteer matches to `Named(kind)` in a post-processing pass.
    Named(NamedEntityKind),
}

/// A single token produced by [`crate::Tokenizer::segment`].
///
/// The `text` field is a **zero-copy** slice of the original input string.
/// Two span types are provided: `span` for byte offsets (suitable for slicing
/// `&str`) and `char_span` for Unicode scalar-value offsets (suitable for
/// Python/JavaScript string indexing and display).
///
/// # Example
///
/// ```rust
/// use kham_core::Tokenizer;
///
/// let tok = Tokenizer::new();
/// let input = "ธนาคาร100แห่ง";
/// let tokens = tok.segment(input);
/// for t in &tokens {
///     // byte span slices the original string exactly
///     assert_eq!(&input[t.span.clone()], t.text);
///     // char span equals the Unicode scalar-value count
///     assert_eq!(t.char_span.end - t.char_span.start, t.text.chars().count());
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// Zero-copy reference into the original input.
    pub text: &'a str,
    /// Byte offsets `start..end` in the original input string.
    /// Both boundaries are valid UTF-8 code-point boundaries.
    pub span: Range<usize>,
    /// Unicode scalar-value (char) offsets `start..end` in the original input.
    /// Use these for language-level string indexing in Python, JavaScript, etc.
    pub char_span: Range<usize>,
    /// Script / category of this token.
    pub kind: TokenKind,
}

impl<'a> Token<'a> {
    /// Construct a new [`Token`].
    ///
    /// # Panics (debug only)
    ///
    /// Panics in debug builds if `span` length does not match `text.len()`, or
    /// if `char_span` length does not match `text.chars().count()`.
    #[inline]
    pub fn new(
        text: &'a str,
        span: Range<usize>,
        char_span: Range<usize>,
        kind: TokenKind,
    ) -> Self {
        debug_assert_eq!(text.len(), span.end - span.start);
        debug_assert_eq!(text.chars().count(), char_span.end - char_span.start);
        Self {
            text,
            span,
            char_span,
            kind,
        }
    }

    /// Byte length of this token's text.
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.span.end - self.span.start
    }

    /// Number of Unicode scalar values (chars) in this token's text.
    #[inline]
    pub fn char_len(&self) -> usize {
        self.char_span.end - self.char_span.start
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make(text: &str, byte_start: usize, char_start: usize, kind: TokenKind) -> Token<'_> {
        let byte_end = byte_start + text.len();
        let char_end = char_start + text.chars().count();
        Token::new(text, byte_start..byte_end, char_start..char_end, kind)
    }

    #[test]
    fn byte_len_matches_text_len() {
        let t = make("กิน", 0, 0, TokenKind::Thai);
        assert_eq!(t.byte_len(), "กิน".len()); // 9 bytes
    }

    #[test]
    fn char_len_matches_char_count() {
        let t = make("กิน", 0, 0, TokenKind::Thai);
        assert_eq!(t.char_len(), 3); // ก + ิ + น
    }

    #[test]
    fn char_len_ascii() {
        let t = make("hello", 0, 0, TokenKind::Latin);
        assert_eq!(t.char_len(), 5);
        assert_eq!(t.byte_len(), 5);
    }

    #[test]
    fn char_span_start_offset() {
        // Token starting at char offset 4 (e.g. after "กิน ")
        let t = make("ข้าว", 10, 4, TokenKind::Thai);
        assert_eq!(t.char_span.start, 4);
        assert_eq!(t.char_span.end, 4 + "ข้าว".chars().count());
    }

    #[test]
    fn emoji_char_len_is_one_per_codepoint() {
        // 😀 is a single codepoint (U+1F600) but 4 bytes.
        let t = make("😀", 0, 0, TokenKind::Emoji);
        assert_eq!(t.char_len(), 1);
        assert_eq!(t.byte_len(), 4);
    }

    // --- NamedEntityKind::from_tag ---

    #[test]
    fn from_tag_person() {
        assert_eq!(
            NamedEntityKind::from_tag("PERSON"),
            Some(NamedEntityKind::Person)
        );
    }

    #[test]
    fn from_tag_place() {
        assert_eq!(
            NamedEntityKind::from_tag("PLACE"),
            Some(NamedEntityKind::Place)
        );
    }

    #[test]
    fn from_tag_org() {
        assert_eq!(NamedEntityKind::from_tag("ORG"), Some(NamedEntityKind::Org));
    }

    #[test]
    fn from_tag_unrecognised_is_none() {
        assert_eq!(NamedEntityKind::from_tag("Person"), None);
        assert_eq!(NamedEntityKind::from_tag(""), None);
        assert_eq!(NamedEntityKind::from_tag("UNKNOWN"), None);
    }

    // --- NamedEntityKind::as_tag ---

    #[test]
    fn as_tag_roundtrips_from_tag() {
        for kind in [
            NamedEntityKind::Person,
            NamedEntityKind::Place,
            NamedEntityKind::Org,
        ] {
            assert_eq!(NamedEntityKind::from_tag(kind.as_tag()), Some(kind));
        }
    }

    // --- NamedEntityKind::as_str ---

    #[test]
    fn as_str_human_readable_labels() {
        assert_eq!(NamedEntityKind::Person.as_str(), "Person");
        assert_eq!(NamedEntityKind::Place.as_str(), "Place");
        assert_eq!(NamedEntityKind::Org.as_str(), "Org");
    }

    #[test]
    fn as_str_differs_from_as_tag() {
        // as_tag is UPPERCASE, as_str is Title-case
        assert_ne!(
            NamedEntityKind::Person.as_str(),
            NamedEntityKind::Person.as_tag()
        );
    }

    // --- TokenKind::Named ---

    #[test]
    fn token_kind_named_equality() {
        assert_eq!(
            TokenKind::Named(NamedEntityKind::Person),
            TokenKind::Named(NamedEntityKind::Person)
        );
        assert_ne!(
            TokenKind::Named(NamedEntityKind::Person),
            TokenKind::Named(NamedEntityKind::Place)
        );
    }
}
