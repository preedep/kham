//! Thai text normalizer.
//!
//! Applies two transformations in order:
//!
//! 1. **วรรณยุกต์ dedup** — consecutive tone marks on the same consonant are
//!    collapsed to the last one. This handles accidental double-keystrokes
//!    (e.g. อ่ อ้ → อ้) as well as identical repetitions (อ่ อ่ → อ่).
//!
//! 2. **Sara Am composition** — the two-character sequence nikhahit (อํ
//!    U+0E4D) + sara aa (อา U+0E32) is composed into the single sara am
//!    character (อำ U+0E33), as Unicode intends.
//!
//! ## Why สระลอย reorder is not included
//!
//! Reordering a misplaced leading vowel (เ แ โ ใ ไ) requires knowing whether
//! that vowel belongs to the consonant *before* it or the consonant *after* it
//! in the code stream. In correctly encoded Thai text the sequence
//! `consonant + lead_vowel` is common at word boundaries (e.g. ว + โ in
//! "ชาวโลก"), and a simple look-ahead cannot distinguish that from a truly
//! misplaced vowel without full TCC-level analysis. Correct TCC analysis
//! requires the same character predicates used here, creating a dependency
//! cycle. Applications that need สระลอย correction should pre-process the
//! text with a dedicated TCC-aware utility before calling [`normalize`].
//!
//! ## NFC note
//!
//! Full Unicode NFC normalisation is not applied because Thai characters
//! have combining class 0 and do not participate in canonical decomposition.
//! The two rules above cover all practically observed Thai normalisation
//! issues. Mixed-script Latin text is passed through unchanged; callers
//! that require full NFC on Latin portions should pre-process the text
//! with a Unicode normalisation library before calling [`normalize`].

use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Thai character predicates (local — avoids coupling to tcc module)
// ---------------------------------------------------------------------------

/// Tone marks: อ่ อ้ อ๊ อ๋ (U+0E48–U+0E4B).
#[inline]
fn is_tone(c: char) -> bool {
    matches!(c, '\u{0E48}'..='\u{0E4B}')
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Normalise Thai text into canonical form.
///
/// Returns an owned [`String`] with both transformations applied.
/// ASCII and non-Thai characters are passed through unchanged.
///
/// # Examples
///
/// ```rust
/// use kham_core::normalizer::normalize;
///
/// // 1. วรรณยุกต์ dedup: double tone mark → single (keep last)
/// let doubled = "\u{0E01}\u{0E48}\u{0E49}"; // ก + อ่ + อ้
/// let fixed = normalize(doubled);
/// assert_eq!(fixed, "\u{0E01}\u{0E49}"); // ก้ only
/// ```
///
/// ```rust
/// use kham_core::normalizer::normalize;
///
/// // 2. Sara Am composition: nikhahit + sara aa → sara am
/// let decomposed = "\u{0E01}\u{0E4D}\u{0E32}"; // ก + อํ + อา
/// let fixed = normalize(decomposed);
/// assert_eq!(fixed, "\u{0E01}\u{0E33}"); // กำ
/// ```
pub fn normalize(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Collect into a char vec for O(1) indexed look-ahead.
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < n {
        let c = chars[i];

        // ── Rule 1: วรรณยุกต์ dedup ───────────────────────────────────────
        // Consume a run of consecutive tone marks, keep only the last one.
        // Rationale: the last key pressed is the most likely intended mark.
        if is_tone(c) {
            let mut last = c;
            while i + 1 < n && is_tone(chars[i + 1]) {
                i += 1;
                last = chars[i];
            }
            out.push(last);
            i += 1;
            continue;
        }

        // ── Rule 2: Sara Am composition ───────────────────────────────────
        // Nikhahit (อํ U+0E4D) + Sara Aa (อา U+0E32)  →  Sara Am (อำ U+0E33)
        if c == '\u{0E4D}' && i + 1 < n && chars[i + 1] == '\u{0E32}' {
            out.push('\u{0E33}'); // อำ
            i += 2;
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── passthrough ───────────────────────────────────────────────────────────

    #[test]
    fn empty_string() {
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn ascii_passthrough() {
        assert_eq!(normalize("hello 123"), "hello 123");
    }

    #[test]
    fn correctly_encoded_thai_unchanged() {
        // Correctly encoded Thai — no tone duplicates or decomposed Sara Am —
        // must come back byte-for-byte identical.
        assert_eq!(normalize("กินข้าว"), "กินข้าว");
        assert_eq!(normalize("สวัสดี"), "สวัสดี");
        assert_eq!(normalize("สวัสดีชาวโลก"), "สวัสดีชาวโลก");
        assert_eq!(normalize("ธนาคารแห่งนั้น"), "ธนาคารแห่งนั้น");
    }

    #[test]
    fn mixed_script_passthrough() {
        let s = "ธนาคาร100แห่ง";
        assert_eq!(normalize(s), s);
    }

    // ── Rule 1: วรรณยุกต์ dedup ──────────────────────────────────────────────

    #[test]
    fn duplicate_same_tone_removed() {
        // ก + อ่ + อ่ → ก + อ่
        let input = "\u{0E01}\u{0E48}\u{0E48}";
        let expected = "\u{0E01}\u{0E48}";
        assert_eq!(normalize(input), expected);
    }

    #[test]
    fn different_tone_keeps_last() {
        // ก + อ่ (low) + อ้ (falling) → ก + อ้  (last wins)
        let input = "\u{0E01}\u{0E48}\u{0E49}";
        let expected = "\u{0E01}\u{0E49}";
        assert_eq!(normalize(input), expected);
    }

    #[test]
    fn triple_tone_keeps_last() {
        // Three consecutive tone marks → keep the last
        let input = "\u{0E01}\u{0E48}\u{0E49}\u{0E4A}";
        let expected = "\u{0E01}\u{0E4A}";
        assert_eq!(normalize(input), expected);
    }

    #[test]
    fn single_tone_unchanged() {
        // ก้ — no duplicate
        let input = "\u{0E01}\u{0E49}";
        assert_eq!(normalize(input), input);
    }

    #[test]
    fn tone_in_real_word() {
        // ข้้าว (ข้าว with doubled อ้) → ข้าว
        let input = "\u{0E02}\u{0E49}\u{0E49}\u{0E32}\u{0E27}"; // ข + อ้ + อ้ + า + ว
        let expected = "\u{0E02}\u{0E49}\u{0E32}\u{0E27}";        // ข้าว
        assert_eq!(normalize(input), expected);
    }

    // ── Rule 2: Sara Am composition ───────────────────────────────────────────

    #[test]
    fn nikhahit_plus_sara_aa_composed() {
        // อํ (U+0E4D) + อา (U+0E32) → อำ (U+0E33)
        let input = "\u{0E4D}\u{0E32}";
        assert_eq!(normalize(input), "\u{0E33}");
    }

    #[test]
    fn sara_am_in_word_context() {
        // ก + อํ + อา → กำ
        let input = "\u{0E01}\u{0E4D}\u{0E32}";
        let expected = "\u{0E01}\u{0E33}";
        assert_eq!(normalize(input), expected);
    }

    #[test]
    fn nikhahit_without_sara_aa_unchanged() {
        // Lone nikhahit (อํ) not followed by sara aa — no composition.
        let input = "\u{0E01}\u{0E4D}\u{0E01}";
        assert_eq!(normalize(input), input);
    }

    #[test]
    fn already_sara_am_unchanged() {
        // อำ (U+0E33) is already composed — must not be double-processed.
        let input = "\u{0E01}\u{0E33}";
        assert_eq!(normalize(input), input);
    }

    #[test]
    fn nam_decomposed_composes_to_nam() {
        // น้ำ decomposed: น + ้ + อํ + อา → น้ำ (น + ้ + อำ)
        let input = "\u{0E19}\u{0E49}\u{0E4D}\u{0E32}";
        let expected = "\u{0E19}\u{0E49}\u{0E33}";
        assert_eq!(normalize(input), expected);
    }

    // ── output length invariant ────────────────────────────────────────────────

    #[test]
    fn output_length_never_exceeds_input_length() {
        // Normalization can only shrink or preserve, never grow the char count.
        let inputs = [
            "กินข้าว",
            "สวัสดีชาวโลก",
            "\u{0E01}\u{0E48}\u{0E48}", // dedup (shrinks)
            "\u{0E4D}\u{0E32}",         // composition (shrinks)
        ];
        for s in inputs {
            let out = normalize(s);
            assert!(
                out.chars().count() <= s.chars().count(),
                "normalize grew the string: {s:?} → {out:?}"
            );
        }
    }
}
