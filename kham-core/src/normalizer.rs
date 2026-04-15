//! Thai text normalizer.
//!
//! Applies three transformations in order:
//!
//! 1. **สระลอย reorder** — a leading vowel (เ แ โ ไ ใ) that appears *after*
//!    its consonant in the code stream is moved before it, matching the
//!    Unicode storage convention. Some keyboards and input methods produce
//!    consonant-first sequences; this step corrects them.
//!
//! 2. **วรรณยุกต์ dedup** — consecutive tone marks on the same consonant are
//!    collapsed to the last one. This handles accidental double-keystrokes
//!    (e.g. อ่ อ้ → อ้) as well as identical repetitions (อ่ อ่ → อ่).
//!
//! 3. **Sara Am composition** — the two-character sequence nikhahit (อํ
//!    U+0E4D) + sara aa (อา U+0E32) is composed into the single sara am
//!    character (อำ U+0E33), as Unicode intends.
//!
//! ## NFC note
//!
//! Full Unicode NFC normalisation is not applied because Thai characters
//! have combining class 0 and do not participate in canonical decomposition.
//! The three rules above cover all practically observed Thai normalisation
//! issues. Mixed-script Latin text is passed through unchanged; callers
//! that require full NFC on Latin portions should pre-process the text
//! with a Unicode normalisation library before calling [`normalize`].

use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Thai character predicates (local — avoids coupling to tcc module)
// ---------------------------------------------------------------------------

/// Thai consonants ก–ฮ (U+0E01–U+0E2E), including vowel-consonants ฤ ฦ.
#[inline]
fn is_consonant(c: char) -> bool {
    matches!(c, '\u{0E01}'..='\u{0E2E}')
}

/// Leading vowels written visually *before* the consonant: เ แ โ ใ ไ
/// (U+0E40–U+0E44). In correct Unicode encoding these precede the consonant
/// in the code stream as well.
#[inline]
fn is_lead_vowel(c: char) -> bool {
    matches!(c, '\u{0E40}'..='\u{0E44}')
}

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
/// Returns an owned [`String`] with all three transformations applied.
/// ASCII and non-Thai characters are passed through unchanged.
///
/// # Examples
///
/// ```rust
/// use kham_core::normalizer::normalize;
///
/// // 1. สระลอย reorder: ก + เ (wrong order) → เก (correct)
/// let wrong_order = "\u{0E01}\u{0E40}"; // ก then เ
/// let fixed = normalize(wrong_order);
/// assert_eq!(fixed, "\u{0E40}\u{0E01}"); // เ then ก
/// ```
///
/// ```rust
/// use kham_core::normalizer::normalize;
///
/// // 2. วรรณยุกต์ dedup: double tone mark → single (keep last)
/// let doubled = "\u{0E01}\u{0E48}\u{0E49}"; // ก + อ่ + อ้
/// let fixed = normalize(doubled);
/// assert_eq!(fixed, "\u{0E01}\u{0E49}"); // ก้ only
/// ```
///
/// ```rust
/// use kham_core::normalizer::normalize;
///
/// // 3. Sara Am composition: nikhahit + sara aa → sara am
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

        // ── Rule 1: สระลอย reorder ────────────────────────────────────────
        // Pattern: CONSONANT + LEAD_VOWEL (wrong order)
        // Fix:     emit LEAD_VOWEL first, then CONSONANT (Unicode order)
        if is_consonant(c) && i + 1 < n && is_lead_vowel(chars[i + 1]) {
            out.push(chars[i + 1]); // leading vowel comes first in Unicode
            out.push(c);
            i += 2;
            continue;
        }

        // ── Rule 2: วรรณยุกต์ dedup ───────────────────────────────────────
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

        // ── Rule 3: Sara Am composition ───────────────────────────────────
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
    use alloc::string::String;

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
    fn already_correct_thai() {
        // Correctly encoded Thai should come back unchanged.
        assert_eq!(normalize("กินข้าว"), "กินข้าว");
        assert_eq!(normalize("สวัสดี"), "สวัสดี");
    }

    #[test]
    fn mixed_script_passthrough() {
        let s = "ธนาคาร100แห่ง";
        assert_eq!(normalize(s), s);
    }

    // ── Rule 1: สระลอย reorder ────────────────────────────────────────────────

    #[test]
    fn lead_vowel_after_consonant_reordered() {
        // ก (U+0E01) + เ (U+0E40) → เก
        let input = "\u{0E01}\u{0E40}";
        let expected = "\u{0E40}\u{0E01}";
        assert_eq!(normalize(input), expected);
    }

    #[test]
    fn all_lead_vowels_reordered() {
        // Each leading vowel after ก should be moved before it.
        let consonant = '\u{0E01}'; // ก
        let lead_vowels = ['\u{0E40}', '\u{0E41}', '\u{0E42}', '\u{0E43}', '\u{0E44}'];
        for lv in lead_vowels {
            let input: String = [consonant, lv].iter().collect();
            let expected: String = [lv, consonant].iter().collect();
            assert_eq!(normalize(&input), expected, "failed for lead vowel U+{:04X}", lv as u32);
        }
    }

    #[test]
    fn lead_vowel_already_before_consonant_unchanged() {
        // เก — vowel already in correct position, must not double-move.
        let correct = "\u{0E40}\u{0E01}";
        assert_eq!(normalize(correct), correct);
    }

    #[test]
    fn multiple_reorder_in_sequence() {
        // กเ + นเ → เก + เน  (two swaps)
        let input = "\u{0E01}\u{0E40}\u{0E19}\u{0E40}";
        let expected = "\u{0E40}\u{0E01}\u{0E40}\u{0E19}";
        assert_eq!(normalize(input), expected);
    }

    // ── Rule 2: วรรณยุกต์ dedup ──────────────────────────────────────────────

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

    // ── Rule 3: Sara Am composition ───────────────────────────────────────────

    #[test]
    fn nikhahit_plus_sara_aa_composed() {
        // อํ (U+0E4D) + อา (U+0E32) → อำ (U+0E33)
        let input = "\u{0E4D}\u{0E32}";
        assert_eq!(normalize(input), "\u{0E33}");
    }

    #[test]
    fn sara_am_in_word_context() {
        // ก + นํ + อา = ก + นำ  (กำ-like context)
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

    // ── interaction between rules ─────────────────────────────────────────────

    #[test]
    fn reorder_then_dedup() {
        // ก + เ (reorder) + อ่ + อ้ (dedup)
        // Step 1 reorder: เก
        // Step 2 dedup:   เก้
        let input = "\u{0E01}\u{0E40}\u{0E48}\u{0E49}";
        // After reorder of ก+เ → เก, then consonant ก is processed next time.
        // The tone run อ่อ้ follows ก — keep last (อ้).
        // Expected: เก้
        let result = normalize(input);
        let rebuilt: String = result.chars().collect();
        // Must start with เ, then ก, then one tone mark (อ้)
        let result_chars: Vec<char> = rebuilt.chars().collect();
        assert_eq!(result_chars[0], '\u{0E40}', "expected เ first");
        assert_eq!(result_chars[1], '\u{0E01}', "expected ก second");
        assert_eq!(result_chars[2], '\u{0E49}', "expected อ้ (last tone)");
        assert_eq!(result_chars.len(), 3);
    }

    #[test]
    fn output_length_never_exceeds_input_length() {
        // Normalization can only shrink or preserve, never grow the char count.
        let inputs = [
            "กินข้าว",
            "\u{0E01}\u{0E40}",       // reorder (same length)
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
