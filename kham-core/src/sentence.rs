//! Thai sentence segmentation.
//!
//! Splits Thai (and mixed-script) text into sentences by detecting sentence-ending
//! delimiters: Thai terminators (`๚` `๛`), Paiyannoi (`ฯ` — but not in `ฯลฯ`),
//! universal punctuation (`!` `?` `.`), and newlines.
//!
//! ## Delimiters
//!
//! | Char | Unicode | Rule |
//! |------|---------|------|
//! | `๚`  | U+0E5A  | Always ends a sentence |
//! | `๛`  | U+0E5B  | Always ends a sentence |
//! | `ฯ`  | U+0E2F  | Ends a sentence unless it is the first or last character of `ฯลฯ` |
//! | `\n` | U+000A  | Always ends a sentence |
//! | `!`  | U+0021  | Always ends a sentence |
//! | `?`  | U+003F  | Always ends a sentence |
//! | `.`  | U+002E  | Ends a sentence when not a decimal point and followed by whitespace or end-of-string |
//!
//! ## No-split cases
//!
//! - `ฯลฯ` ("etc.") — neither `ฯ` character in the sequence is a split point.
//! - `3.14` — a period between two ASCII digits is a decimal point, not a boundary.
//! - `A.B.C.` — a period not followed by whitespace or end-of-string is not a boundary
//!   (handles abbreviations like `ก.ค.`, `พ.ศ.`, `A.D.`).
//!
//! # Examples
//!
//! ```rust
//! use kham_core::sentence::split_sentences;
//!
//! let sents = split_sentences("วันนี้อากาศดี\nพรุ่งนี้จะฝนตก");
//! assert_eq!(sents.len(), 2);
//! assert_eq!(sents[0].text.trim(), "วันนี้อากาศดี");
//! assert_eq!(sents[1].text.trim(), "พรุ่งนี้จะฝนตก");
//!
//! // ฯลฯ is not a sentence boundary
//! let sents2 = split_sentences("กินข้าวฯลฯทุกวัน");
//! assert_eq!(sents2.len(), 1);
//! ```

use alloc::vec::Vec;
use core::ops::Range;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A sentence extracted from source text.
///
/// `text` is a zero-copy slice of the original input. It includes the
/// terminating delimiter (if any) and surrounding whitespace — call
/// `.text.trim()` to strip those. `span` and `char_span` give the byte and
/// char offsets of the slice in the source string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence<'a> {
    /// Zero-copy slice of the source text (includes terminator).
    pub text: &'a str,
    /// Byte offsets `[start, end)` of this sentence in the source string.
    pub span: Range<usize>,
    /// Unicode scalar-value offsets `[start, end)` of this sentence.
    pub char_span: Range<usize>,
}

// ---------------------------------------------------------------------------
// Segmenter
// ---------------------------------------------------------------------------

/// Splits text into sentences.
///
/// Currently stateless; a builder API will be added when configurable options
/// (e.g., toggling newline splitting) are required.
///
/// ```rust
/// use kham_core::sentence::SentenceSegmenter;
///
/// let seg = SentenceSegmenter::new();
/// let sents = seg.split("กินข้าว\nดื่มน้ำ");
/// assert_eq!(sents.len(), 2);
/// ```
#[derive(Debug, Default, Clone)]
pub struct SentenceSegmenter;

impl SentenceSegmenter {
    /// Create a sentence segmenter with default settings.
    pub fn new() -> Self {
        Self
    }

    /// Split `text` into sentences.
    ///
    /// Empty and whitespace-only spans between delimiters are silently dropped.
    /// The returned slices are zero-copy references into `text`.
    pub fn split<'a>(&self, text: &'a str) -> Vec<Sentence<'a>> {
        if text.is_empty() {
            return Vec::new();
        }

        // Collect (byte_offset, char) pairs once for O(1) lookahead/lookbehind.
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let n = chars.len();

        let mut result = Vec::new();
        let mut seg_byte_start = 0usize;
        let mut seg_char_start = 0usize;

        for i in 0..n {
            if !is_boundary(&chars, i) {
                continue;
            }

            let byte_end = if i + 1 < n {
                chars[i + 1].0
            } else {
                text.len()
            };
            let char_end = i + 1;

            let slice = &text[seg_byte_start..byte_end];
            if !slice.trim().is_empty() {
                result.push(Sentence {
                    text: slice,
                    span: seg_byte_start..byte_end,
                    char_span: seg_char_start..char_end,
                });
            }
            seg_byte_start = byte_end;
            seg_char_start = char_end;
        }

        // Remaining text after the last delimiter.
        if seg_byte_start < text.len() {
            let slice = &text[seg_byte_start..];
            if !slice.trim().is_empty() {
                result.push(Sentence {
                    text: slice,
                    span: seg_byte_start..text.len(),
                    char_span: seg_char_start..n,
                });
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Free function
// ---------------------------------------------------------------------------

/// Split `text` into sentences.
///
/// Convenience wrapper over [`SentenceSegmenter::split`].
///
/// # Examples
///
/// ```rust
/// use kham_core::sentence::split_sentences;
///
/// let sents = split_sentences("กินข้าว\nดื่มน้ำ");
/// assert_eq!(sents.len(), 2);
/// assert_eq!(sents[0].text.trim(), "กินข้าว");
/// assert_eq!(sents[1].text.trim(), "ดื่มน้ำ");
/// ```
pub fn split_sentences(text: &str) -> Vec<Sentence<'_>> {
    SentenceSegmenter::new().split(text)
}

// ---------------------------------------------------------------------------
// Boundary detection
// ---------------------------------------------------------------------------

/// Return `true` if `chars[i]` is the last character of a sentence.
fn is_boundary(chars: &[(usize, char)], i: usize) -> bool {
    let c = chars[i].1;
    let prev = if i > 0 { Some(chars[i - 1].1) } else { None };
    let next = if i + 1 < chars.len() {
        Some(chars[i + 1].1)
    } else {
        None
    };

    match c {
        // Thai section / sentence terminators — always end a sentence.
        '\u{0E5A}' | '\u{0E5B}' => true,

        // Paiyannoi ฯ (U+0E2F) — ends a sentence unless it is part of ฯลฯ.
        //   ฯลฯ = U+0E2F  U+0E25  U+0E2F
        // First ฯ: next char is ล AND char after that is ฯ.
        // Last  ฯ: prev char is ล AND char before that is ฯ.
        '\u{0E2F}' => {
            let next2 = chars.get(i + 2).map(|(_, c2)| *c2);
            let is_ฯลฯ_first = next == Some('\u{0E25}') && next2 == Some('\u{0E2F}');
            let is_ฯลฯ_last = prev == Some('\u{0E25}') && i >= 2 && chars[i - 2].1 == '\u{0E2F}';
            !is_ฯลฯ_first && !is_ฯลฯ_last
        }

        // Newline — always ends a sentence (paragraph / line break).
        '\n' => true,

        // Universal sentence-ending punctuation.
        '!' | '?' => true,

        // Period:
        //   - NOT a boundary when it is a decimal point (digit on both sides).
        //   - NOT a boundary when the next character is not whitespace and not
        //     end-of-string (rules out mid-abbreviation dots like ก.ค., A.B.C.).
        '.' => {
            let prev_digit = prev.is_some_and(|p| p.is_ascii_digit());
            let next_digit = next.is_some_and(|n| n.is_ascii_digit());
            let next_space_or_end = next.is_none_or(|n| n.is_whitespace());
            !prev_digit && !next_digit && next_space_or_end
        }

        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn trimmed<'a>(sents: &'a [Sentence<'a>]) -> Vec<&'a str> {
        sents.iter().map(|s| s.text.trim()).collect()
    }

    // ── basic splitting ───────────────────────────────────────────────────────

    #[test]
    fn empty_returns_empty() {
        assert!(split_sentences("").is_empty());
    }

    #[test]
    fn whitespace_only_returns_empty() {
        assert!(split_sentences("   \n\t  ").is_empty());
    }

    #[test]
    fn single_sentence_no_delimiter() {
        let sents = split_sentences("กินข้าวกับปลา");
        assert_eq!(trimmed(&sents), &["กินข้าวกับปลา"]);
    }

    #[test]
    fn split_on_newline() {
        let sents = split_sentences("กินข้าว\nดื่มน้ำ");
        assert_eq!(trimmed(&sents), &["กินข้าว", "ดื่มน้ำ"]);
    }

    #[test]
    fn double_newline_no_empty_sentence() {
        // The empty span between two newlines must be dropped.
        let sents = split_sentences("กินข้าว\n\nดื่มน้ำ");
        assert_eq!(trimmed(&sents), &["กินข้าว", "ดื่มน้ำ"]);
    }

    #[test]
    fn trailing_newline_no_empty_sentence() {
        let sents = split_sentences("กินข้าว\n");
        assert_eq!(sents.len(), 1);
        assert_eq!(sents[0].text.trim(), "กินข้าว");
    }

    #[test]
    fn three_sentences_via_newlines() {
        let sents = split_sentences("ประโยคหนึ่ง\nประโยคสอง\nประโยคสาม");
        assert_eq!(sents.len(), 3);
    }

    // ── Thai terminators ──────────────────────────────────────────────────────

    #[test]
    fn angkhankhu_splits() {
        // ๚ (U+0E5A) is the Thai sentence mark.
        let sents = split_sentences("กินข้าว๚ดื่มน้ำ");
        assert_eq!(sents.len(), 2, "sents: {:?}", trimmed(&sents));
        assert!(sents[0].text.contains("กินข้าว"));
        assert!(sents[1].text.contains("ดื่มน้ำ"));
    }

    #[test]
    fn khomut_splits() {
        // ๛ (U+0E5B) is the Thai chapter/section mark.
        let sents = split_sentences("บทที่หนึ่ง๛บทที่สอง");
        assert_eq!(sents.len(), 2);
    }

    // ── Paiyannoi ฯ rules ─────────────────────────────────────────────────────

    #[test]
    fn paiyannoi_alone_splits() {
        // Standalone ฯ (not part of ฯลฯ) ends the sentence.
        let sents = split_sentences("กินข้าวฯดื่มน้ำ");
        assert_eq!(sents.len(), 2, "ฯ should split: {:?}", trimmed(&sents));
    }

    #[test]
    fn ฯลฯ_does_not_split() {
        // ฯลฯ is an abbreviation ("etc.") — must not be treated as a sentence boundary.
        let sents = split_sentences("กินข้าวฯลฯทุกวัน");
        assert_eq!(
            sents.len(),
            1,
            "ฯลฯ should not split: {:?}",
            trimmed(&sents)
        );
    }

    #[test]
    fn ฯลฯ_in_middle_preserves_two_sentences() {
        // ฯลฯ in the middle of a sentence, split by newline at end.
        let sents = split_sentences("กินข้าวฯลฯทุกวัน\nพรุ่งนี้จะฝน");
        assert_eq!(sents.len(), 2, "sents: {:?}", trimmed(&sents));
        assert!(
            trimmed(&sents)[0].contains("ฯลฯ"),
            "ฯลฯ should remain in first sentence"
        );
    }

    // ── period rules ─────────────────────────────────────────────────────────

    #[test]
    fn period_before_space_splits() {
        let sents = split_sentences("Hello world. Goodbye world.");
        assert_eq!(sents.len(), 2, "sents: {:?}", trimmed(&sents));
        assert_eq!(sents[0].text.trim(), "Hello world.");
        assert_eq!(sents[1].text.trim(), "Goodbye world.");
    }

    #[test]
    fn period_at_end_of_string_does_not_add_empty_sentence() {
        let sents = split_sentences("Hello world.");
        assert_eq!(sents.len(), 1);
        assert_eq!(sents[0].text.trim(), "Hello world.");
    }

    #[test]
    fn decimal_point_does_not_split() {
        // Period between two ASCII digits is a decimal point.
        let sents = split_sentences("ราคา3.14บาท");
        assert_eq!(
            sents.len(),
            1,
            "decimal point should not split: {:?}",
            trimmed(&sents)
        );
    }

    #[test]
    fn abbreviation_dot_not_followed_by_space_does_not_split() {
        // ก.ค. — period not followed by whitespace or end: not a boundary.
        let sents = split_sentences("วันที่5ก.ค.2567");
        assert_eq!(
            sents.len(),
            1,
            "abbreviation dots should not split: {:?}",
            trimmed(&sents)
        );
    }

    // ── exclamation and question marks ────────────────────────────────────────

    #[test]
    fn exclamation_splits() {
        let sents = split_sentences("ดีมาก!แย่มาก");
        assert_eq!(sents.len(), 2, "! should split: {:?}", trimmed(&sents));
    }

    #[test]
    fn question_splits() {
        let sents = split_sentences("ไปไหน?ไปตลาด");
        assert_eq!(sents.len(), 2, "? should split: {:?}", trimmed(&sents));
    }

    // ── span correctness ──────────────────────────────────────────────────────

    #[test]
    fn byte_spans_are_valid_utf8_slices() {
        let text = "กินข้าว\nดื่มน้ำ";
        for s in split_sentences(text) {
            // Must not panic.
            let _ = &text[s.span.clone()];
            assert_eq!(s.text, &text[s.span]);
        }
    }

    #[test]
    fn char_spans_match_text() {
        let text = "กินข้าว\nดื่มน้ำ";
        let all_chars: Vec<char> = text.chars().collect();
        for s in split_sentences(text) {
            let by_char: alloc::string::String = all_chars[s.char_span.clone()].iter().collect();
            assert_eq!(s.text, by_char, "char_span mismatch for '{}'", s.text);
        }
    }

    #[test]
    fn spans_cover_full_input() {
        // The union of sentence spans must equal the full text length
        // (minus any whitespace-only gaps between delimiters).
        let text = "ประโยคหนึ่ง\nประโยคสอง\nประโยคสาม";
        let sents = split_sentences(text);
        let reconstructed: alloc::string::String = sents.iter().map(|s| s.text).collect();
        assert_eq!(reconstructed, text);
    }

    // ── mixed script ──────────────────────────────────────────────────────────

    #[test]
    fn mixed_thai_english_newline() {
        let sents = split_sentences("กินข้าว\nHello world.\nดื่มน้ำ");
        // \n → sentence 1; "Hello world." → period+end/whitespace → sentence 2; "ดื่มน้ำ" → 3
        assert!(
            sents.len() >= 2,
            "expected ≥ 2 sentences, got {:?}",
            trimmed(&sents)
        );
    }

    // ── SentenceSegmenter struct ──────────────────────────────────────────────

    #[test]
    fn segmenter_new_and_default_agree() {
        let text = "กินข้าว\nดื่มน้ำ";
        let a = SentenceSegmenter::new().split(text);
        let b = SentenceSegmenter.split(text);
        assert_eq!(a, b);
    }
}
