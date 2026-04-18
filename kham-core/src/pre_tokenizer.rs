//! Unicode script classifier and pre-tokenizer.
//!
//! Splits raw input into coarse, script-homogeneous [`Token`] spans before
//! the main segmenter runs. The segmenter only needs to apply the expensive
//! DAG algorithm to Thai spans; all other spans pass through unchanged.
//!
//! ## Pipeline position
//!
//! ```text
//! raw text
//!    │
//!    ▼
//! pre_tokenize()   ← this module
//!    │  splits into [Thai | Latin | Number | Whitespace | Emoji | Punctuation | Unknown]
//!    ▼
//! segmenter        ← processes Thai spans with tcc + dict
//!    │
//!    ▼
//! Vec<Token<'_>>
//! ```
//!
//! ## Example
//!
//! ```rust
//! use kham_core::pre_tokenizer::pre_tokenize;
//! use kham_core::TokenKind;
//!
//! let spans = pre_tokenize("ธนาคาร100แห่ง");
//! assert_eq!(spans[0].kind, TokenKind::Thai);   // "ธนาคาร"
//! assert_eq!(spans[1].kind, TokenKind::Number); // "100"
//! assert_eq!(spans[2].kind, TokenKind::Thai);   // "แห่ง"
//! ```

use alloc::vec::Vec;

use crate::token::{Token, TokenKind};

// ---------------------------------------------------------------------------
// Character classification
// ---------------------------------------------------------------------------

/// Classify a single Unicode scalar value into a [`TokenKind`].
///
/// Classification is purely codepoint-based — no context is used. The rules
/// are applied in priority order so that sub-ranges override their parent
/// block (e.g. Thai digits are checked before the broader Thai block).
///
/// ## Classification table
///
/// | Range / set | Kind |
/// |---|---|
/// | U+0E50–U+0E59 (Thai digits ๐–๙) | `Number` |
/// | U+0E00–U+0E7F (Thai block) | `Thai` |
/// | `0`–`9` (ASCII digits) | `Number` |
/// | U+FF10–U+FF19 (fullwidth digits) | `Number` |
/// | `A`–`Z`, `a`–`z` (ASCII letters) | `Latin` |
/// | U+FF21–U+FF5A (fullwidth Latin) | `Latin` |
/// | Space, tab, newline, CR, NBSP, ideographic space | `Whitespace` |
/// | Major emoji blocks (U+1F300–U+1FAFF, U+2600–U+27BF, …) | `Emoji` |
/// | ASCII punctuation (`!`–`/`, `:`–`@`, …) | `Punctuation` |
/// | U+2000–U+206F (Unicode general punctuation) | `Punctuation` |
/// | Everything else | `Unknown` |
#[inline]
pub fn classify_char(c: char) -> TokenKind {
    match c {
        // Thai digits sit inside the Thai block — check them first so they
        // are not misclassified as Thai script.
        '\u{0E50}'..='\u{0E59}' => TokenKind::Number,

        // Remaining Thai Unicode block: consonants, vowels, tone marks, etc.
        '\u{0E00}'..='\u{0E7F}' => TokenKind::Thai,

        // ASCII decimal digits.
        '0'..='9' => TokenKind::Number,

        // Fullwidth digit forms (U+FF10 ０ – U+FF19 ９).
        '\u{FF10}'..='\u{FF19}' => TokenKind::Number,

        // ASCII basic Latin letters (a–z, A–Z).
        'A'..='Z' | 'a'..='z' => TokenKind::Latin,

        // Fullwidth Latin capital (U+FF21 Ａ – U+FF3A Ｚ) and
        // small (U+FF41 ａ – U+FF5A ｚ) letter forms.
        '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}' => TokenKind::Latin,

        // Common whitespace: regular space, horizontal tab, newline, carriage
        // return, non-breaking space (U+00A0), and ideographic space (U+3000).
        ' ' | '\t' | '\n' | '\r' | '\u{00A0}' | '\u{3000}' => TokenKind::Whitespace,

        // Emoji — covers the core emoji blocks in the Supplementary Multilingual
        // Plane and the Miscellaneous Symbols / Dingbats blocks in the BMP.
        // ZWJ (U+200D) and the emoji variation selector (U+FE0F) are also
        // included so that ZWJ emoji sequences stay in one span.
        c if is_emoji(c) => TokenKind::Emoji,

        // ASCII punctuation is split into three non-contiguous ranges:
        //   U+0021–U+002F  ! " # $ % & ' ( ) * + , - . /
        //   U+003A–U+0040  : ; < = > ? @
        //   U+005B–U+0060  [ \ ] ^ _ `
        //   U+007B–U+007E  { | } ~
        '!'..='/' | ':'..='@' | '['..='`' | '{'..='~' => TokenKind::Punctuation,

        // Unicode General Punctuation block (U+2000–U+206F):
        // em-dash, en-dash, ellipsis, quotation marks, etc.
        '\u{2000}'..='\u{206F}' => TokenKind::Punctuation,

        // All other codepoints (Hangul, Arabic, Cyrillic, CJK, etc.).
        _ => TokenKind::Unknown,
    }
}

/// Returns `true` if `c` belongs to one of the major Unicode emoji blocks.
///
/// This function is intentionally conservative: it matches codepoints that
/// are nearly always emoji (Emoticons, Miscellaneous Symbols and Pictographs,
/// Transport and Map Symbols, supplemental emoji blocks), plus the two glue
/// codepoints used to build emoji sequences — ZWJ (U+200D) and the emoji
/// variation selector (U+FE0F).
///
/// Full ZWJ-sequence detection (e.g. 👨‍👩‍👧) requires multi-codepoint
/// lookahead and is left to a dedicated Unicode segmenter; this function
/// ensures that the individual codepoints in such sequences are at least
/// classified as `Emoji` so they land in the same pre-token span.
#[inline]
pub fn is_emoji(c: char) -> bool {
    matches!(c,
        // Zero-width joiner — glue used in multi-person / flag emoji sequences.
        '\u{200D}'
        // Variation Selector-16: forces emoji (graphic) presentation.
        | '\u{FE0F}'
        // Miscellaneous Symbols and Dingbats (BMP).
        | '\u{2600}'..='\u{27BF}'
        // Supplemental Symbols and Pictographs — the large SMP emoji block.
        // Covers Emoticons (1F600), Misc Symbols & Pictographs (1F300),
        // Transport (1F680), Activities (1F3C0), Objects (1F4A0), etc.
        | '\u{1F300}'..='\u{1F9FF}'
        // Symbols and Pictographs Extended-A (chess, medical symbols, …).
        | '\u{1FA00}'..='\u{1FAFF}'
    )
}

// ---------------------------------------------------------------------------
// Pre-tokenizer
// ---------------------------------------------------------------------------

/// Split `text` into a sequence of script-homogeneous [`Token`] spans.
///
/// Each span groups consecutive characters that share the same [`TokenKind`]
/// as determined by [`classify_char`]. Spans never overlap and their union
/// is exactly `text` — i.e. joining `token.text` values reconstructs the
/// original string.
///
/// The function is O(n) in the number of Unicode scalar values in `text`.
/// No allocation beyond the output `Vec` is performed.
///
/// # Returns
///
/// An empty `Vec` when `text` is empty.
///
/// # Example
///
/// ```rust
/// use kham_core::pre_tokenizer::pre_tokenize;
/// use kham_core::TokenKind;
///
/// // Mixed Thai / number / Thai
/// let tokens = pre_tokenize("ธนาคาร100แห่ง");
/// assert_eq!(tokens.len(), 3);
/// assert_eq!(tokens[0].text, "ธนาคาร");
/// assert_eq!(tokens[0].kind, TokenKind::Thai);
/// assert_eq!(tokens[1].text, "100");
/// assert_eq!(tokens[1].kind, TokenKind::Number);
/// assert_eq!(tokens[2].text, "แห่ง");
/// assert_eq!(tokens[2].kind, TokenKind::Thai);
/// ```
pub fn pre_tokenize(text: &str) -> Vec<Token<'_>> {
    if text.is_empty() {
        return Vec::new();
    }

    // Capacity hint: most real text averages > 3 bytes per token, so
    // `text.len() / 4` avoids most reallocations without over-allocating.
    let mut tokens: Vec<Token<'_>> = Vec::with_capacity(text.len() / 4 + 1);

    // `span_start`/`char_span_start` track the byte/char offset where the
    // current span began. `span_kind` is `None` only before the first char.
    let mut span_start = 0usize;
    let mut char_span_start = 0usize;
    let mut span_kind: Option<TokenKind> = None;
    let mut char_pos = 0usize;

    for (byte_pos, c) in text.char_indices() {
        let kind = classify_char(c);

        match span_kind {
            // No span open yet — start the first one.
            None => {
                span_start = byte_pos;
                char_span_start = char_pos;
                span_kind = Some(kind);
            }

            // Same kind as the running span — extend it silently.
            Some(k) if k == kind => {}

            // Different kind — flush the completed span and open a new one.
            Some(k) => {
                push_token(
                    &mut tokens,
                    text,
                    span_start,
                    byte_pos,
                    char_span_start,
                    char_pos,
                    k,
                );
                span_start = byte_pos;
                char_span_start = char_pos;
                span_kind = Some(kind);
            }
        }

        char_pos += 1;
    }

    // Flush the final span (always non-empty because text is non-empty).
    if let Some(k) = span_kind {
        push_token(
            &mut tokens,
            text,
            span_start,
            text.len(),
            char_span_start,
            char_pos,
            k,
        );
    }

    tokens
}

/// Construct a [`Token`] from byte and char ranges of `text` and push it onto `out`.
#[inline]
fn push_token<'t>(
    out: &mut Vec<Token<'t>>,
    text: &'t str,
    start: usize,
    end: usize,
    char_start: usize,
    char_end: usize,
    kind: TokenKind,
) {
    out.push(Token::new(
        &text[start..end],
        start..end,
        char_start..char_end,
        kind,
    ));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Assert that `pre_tokenize(text)` produces tokens with the given
    /// `(text, kind)` pairs, in order.
    fn assert_tokens(text: &str, expected: &[(&str, TokenKind)]) {
        let tokens = pre_tokenize(text);
        assert_eq!(
            tokens.len(),
            expected.len(),
            "token count mismatch for {text:?}\ngot: {tokens:?}"
        );
        for (i, (tok, &(exp_text, exp_kind))) in tokens.iter().zip(expected.iter()).enumerate() {
            assert_eq!(tok.text, exp_text, "token[{i}].text");
            assert_eq!(tok.kind, exp_kind, "token[{i}].kind");
        }
    }

    // ── edge cases ───────────────────────────────────────────────────────────

    #[test]
    fn empty_input_returns_empty_vec() {
        assert!(pre_tokenize("").is_empty());
    }

    #[test]
    fn single_char_each_kind() {
        assert_tokens("ก", &[("ก", TokenKind::Thai)]);
        assert_tokens("A", &[("A", TokenKind::Latin)]);
        assert_tokens("1", &[("1", TokenKind::Number)]);
        assert_tokens(" ", &[(" ", TokenKind::Whitespace)]);
        assert_tokens("!", &[("!", TokenKind::Punctuation)]);
        assert_tokens("😀", &[("😀", TokenKind::Emoji)]);
    }

    // ── Thai ─────────────────────────────────────────────────────────────────

    #[test]
    fn thai_run_stays_one_span() {
        assert_tokens("สวัสดี", &[("สวัสดี", TokenKind::Thai)]);
    }

    #[test]
    fn thai_digits_split_from_thai_script() {
        // Thai digits ๑๒๓ are Number, not Thai.
        assert_tokens("ก๑", &[("ก", TokenKind::Thai), ("๑", TokenKind::Number)]);
    }

    #[test]
    fn thai_digits_grouped_as_number() {
        assert_tokens("๑๒๓", &[("๑๒๓", TokenKind::Number)]);
    }

    // ── Latin ─────────────────────────────────────────────────────────────────

    #[test]
    fn latin_run_stays_one_span() {
        assert_tokens("hello", &[("hello", TokenKind::Latin)]);
    }

    #[test]
    fn latin_case_mixed_stays_one_span() {
        assert_tokens("Hello", &[("Hello", TokenKind::Latin)]);
    }

    #[test]
    fn fullwidth_latin_classified_as_latin() {
        // Ａ = U+FF21, ａ = U+FF41
        assert_tokens("Ａａ", &[("Ａａ", TokenKind::Latin)]);
    }

    // ── Number ───────────────────────────────────────────────────────────────

    #[test]
    fn ascii_digits_grouped() {
        assert_tokens("100", &[("100", TokenKind::Number)]);
    }

    #[test]
    fn fullwidth_digits_classified_as_number() {
        // ０ = U+FF10
        assert_tokens("１２３", &[("１２３", TokenKind::Number)]);
    }

    // ── Whitespace ────────────────────────────────────────────────────────────

    #[test]
    fn space_tab_newline_grouped() {
        assert_tokens(" \t\n", &[(" \t\n", TokenKind::Whitespace)]);
    }

    #[test]
    fn nbsp_classified_as_whitespace() {
        // U+00A0 non-breaking space
        let nbsp = "\u{00A0}";
        assert_tokens(nbsp, &[(nbsp, TokenKind::Whitespace)]);
    }

    #[test]
    fn ideographic_space_classified_as_whitespace() {
        // U+3000 ideographic space
        let is = "\u{3000}";
        assert_tokens(is, &[(is, TokenKind::Whitespace)]);
    }

    // ── Punctuation ───────────────────────────────────────────────────────────

    #[test]
    fn ascii_punctuation_classified() {
        for ch in "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".chars() {
            let s = ch.to_string();
            let tokens = pre_tokenize(&s);
            assert_eq!(tokens.len(), 1, "expected 1 token for {ch:?}");
            assert_eq!(
                tokens[0].kind,
                TokenKind::Punctuation,
                "wrong kind for {ch:?}"
            );
        }
    }

    #[test]
    fn unicode_punctuation_em_dash() {
        // U+2014 EM DASH is in the General Punctuation block.
        assert_tokens("—", &[("—", TokenKind::Punctuation)]);
    }

    #[test]
    fn unicode_punctuation_ellipsis() {
        assert_tokens("…", &[("…", TokenKind::Punctuation)]);
    }

    // ── Emoji ─────────────────────────────────────────────────────────────────

    #[test]
    fn basic_emoji_span() {
        assert_tokens("😀", &[("😀", TokenKind::Emoji)]);
    }

    #[test]
    fn emoji_run_stays_one_span() {
        assert_tokens("😀🎉", &[("😀🎉", TokenKind::Emoji)]);
    }

    #[test]
    fn misc_symbol_emoji() {
        // U+2764 ❤ is in the Miscellaneous Symbols block.
        assert_tokens("❤", &[("❤", TokenKind::Emoji)]);
    }

    // ── Mixed script ──────────────────────────────────────────────────────────

    #[test]
    fn bank_example() {
        // Classic mixed-script Thai example from CLAUDE.md.
        assert_tokens(
            "ธนาคาร100แห่ง",
            &[
                ("ธนาคาร", TokenKind::Thai),
                ("100", TokenKind::Number),
                ("แห่ง", TokenKind::Thai),
            ],
        );
    }

    #[test]
    fn thai_space_latin() {
        assert_tokens(
            "สวัสดี hello",
            &[
                ("สวัสดี", TokenKind::Thai),
                (" ", TokenKind::Whitespace),
                ("hello", TokenKind::Latin),
            ],
        );
    }

    #[test]
    fn latin_number_thai() {
        assert_tokens(
            "hello123สวัสดี",
            &[
                ("hello", TokenKind::Latin),
                ("123", TokenKind::Number),
                ("สวัสดี", TokenKind::Thai),
            ],
        );
    }

    #[test]
    fn all_kinds_in_sequence() {
        assert_tokens(
            "กิน 1 A!😀",
            &[
                ("กิน", TokenKind::Thai),
                (" ", TokenKind::Whitespace),
                ("1", TokenKind::Number),
                (" ", TokenKind::Whitespace),
                ("A", TokenKind::Latin),
                ("!", TokenKind::Punctuation),
                ("😀", TokenKind::Emoji),
            ],
        );
    }

    // ── Structural invariants ─────────────────────────────────────────────────

    #[test]
    fn spans_cover_full_input() {
        // Joining all token texts must reconstruct the original string exactly.
        let inputs = [
            "ธนาคาร100แห่ง",
            "hello world",
            "สวัสดี 😀 123!",
            "กิน\tข้าว\n",
            "",
        ];
        for input in inputs {
            let rebuilt: String = pre_tokenize(input).iter().map(|t| t.text).collect();
            assert_eq!(rebuilt, input, "coverage failed for {input:?}");
        }
    }

    #[test]
    fn span_byte_offsets_are_correct() {
        // Every span's byte range must match the string it refers to.
        let text = "ธนาคาร100แห่ง";
        for tok in pre_tokenize(text) {
            assert_eq!(
                &text[tok.span.clone()],
                tok.text,
                "span mismatch: {:?}",
                tok
            );
            assert!(
                text.is_char_boundary(tok.span.start),
                "span.start is not a char boundary"
            );
            assert!(
                text.is_char_boundary(tok.span.end),
                "span.end is not a char boundary"
            );
        }
    }

    #[test]
    fn no_empty_tokens() {
        // The pre-tokenizer must never emit a zero-length token.
        let text = "กิน hello 123";
        for tok in pre_tokenize(text) {
            assert!(!tok.text.is_empty(), "empty token: {tok:?}");
        }
    }

    #[test]
    fn adjacent_spans_are_contiguous() {
        // The end of span[i] must equal the start of span[i+1].
        let text = "กิน hello 123!😀";
        let tokens = pre_tokenize(text);
        for pair in tokens.windows(2) {
            assert_eq!(
                pair[0].span.end, pair[1].span.start,
                "gap between {:?} and {:?}",
                pair[0], pair[1]
            );
        }
    }

    #[test]
    fn char_spans_are_contiguous() {
        let text = "กิน hello 123!😀";
        let tokens = pre_tokenize(text);
        for pair in tokens.windows(2) {
            assert_eq!(
                pair[0].char_span.end, pair[1].char_span.start,
                "char_span gap between {:?} and {:?}",
                pair[0].text, pair[1].text
            );
        }
    }

    #[test]
    fn char_span_len_matches_char_count() {
        let text = "ธนาคาร100แห่ง";
        for tok in pre_tokenize(text) {
            assert_eq!(
                tok.char_span.end - tok.char_span.start,
                tok.text.chars().count(),
                "char_span mismatch for {:?}",
                tok.text
            );
        }
    }

    #[test]
    fn char_span_mixed_script_offsets() {
        // "ธนาคาร100แห่ง": ธนาคาร=6 chars, 100=3 chars, แห่ง=4 chars
        let tokens = pre_tokenize("ธนาคาร100แห่ง");
        assert_eq!(tokens[0].char_span, 0..6);
        assert_eq!(tokens[1].char_span, 6..9);
        assert_eq!(tokens[2].char_span, 9..13);
    }

    #[test]
    fn char_span_emoji_counts_as_one_char() {
        // 😀 is 4 bytes but 1 Unicode scalar value.
        let tokens = pre_tokenize("😀");
        assert_eq!(tokens[0].char_span, 0..1);
        assert_eq!(tokens[0].span, 0..4);
    }

    // ── classify_char direct tests ────────────────────────────────────────────

    #[test]
    fn classify_char_spot_checks() {
        assert_eq!(classify_char('ก'), TokenKind::Thai);
        assert_eq!(classify_char('๑'), TokenKind::Number); // Thai digit
        assert_eq!(classify_char('a'), TokenKind::Latin);
        assert_eq!(classify_char('Z'), TokenKind::Latin);
        assert_eq!(classify_char('5'), TokenKind::Number);
        assert_eq!(classify_char(' '), TokenKind::Whitespace);
        assert_eq!(classify_char('\n'), TokenKind::Whitespace);
        assert_eq!(classify_char('!'), TokenKind::Punctuation);
        assert_eq!(classify_char('.'), TokenKind::Punctuation);
        assert_eq!(classify_char('😀'), TokenKind::Emoji);
        assert_eq!(classify_char('❤'), TokenKind::Emoji);
    }
}
