//! Thai Character Cluster (TCC) boundary detection.
//!
//! Implements the TCC rules from Theeramunkong et al. (2000).
//! A TCC is the smallest indivisible Thai orthographic unit — roughly
//! one leading vowel + one consonant + its upper vowels + tone mark + trailing vowel.
//!
//! ## Pattern (simplified)
//! ```text
//! TCC = LEAD? CONSONANT UPPER* TONE? (THANTHAKAT | FOLLOW | NIKHAHIT)?
//!     | NON_THAI+
//! ```
//!
//! TCC segmentation is used as a pre-pass by the main segmenter to ensure
//! that word boundaries always fall on TCC boundaries.

use alloc::vec;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Unicode character classification
// ---------------------------------------------------------------------------

/// Thai consonants ก–ฮ (U+0E01–U+0E2E), plus special vowel-consonants ฤ ฦ.
#[inline]
fn is_consonant(c: char) -> bool {
    matches!(c, '\u{0E01}'..='\u{0E2E}')
}

/// Leading vowels that appear *before* the consonant: เ แ โ ไ ใ (U+0E40–U+0E44).
#[inline]
fn is_lead_vowel(c: char) -> bool {
    matches!(c, '\u{0E40}'..='\u{0E44}')
}

/// Upper vowels / signs written above the consonant: อั อิ อี อึ อื อุ อู อฺ
/// (U+0E31, U+0E34–U+0E3A).
#[inline]
fn is_upper_vowel(c: char) -> bool {
    c == '\u{0E31}' || matches!(c, '\u{0E34}'..='\u{0E3A}')
}

/// Tone marks: อ่ อ้ อ๊ อ๋ (U+0E48–U+0E4B).
#[inline]
fn is_tone(c: char) -> bool {
    matches!(c, '\u{0E48}'..='\u{0E4B}')
}

/// Thanthakat ์ (U+0E4C) — silences a consonant.
#[inline]
fn is_thanthakat(c: char) -> bool {
    c == '\u{0E4C}'
}

/// Nikhahit อํ (U+0E4D) — the upper component of Sara Am อำ.
#[inline]
fn is_nikhahit(c: char) -> bool {
    c == '\u{0E4D}'
}

/// Follow (trailing) vowels written after the consonant: อะ อา อำ
/// (U+0E30, U+0E32–U+0E33).
#[inline]
fn is_follow_vowel(c: char) -> bool {
    c == '\u{0E30}' || matches!(c, '\u{0E32}'..='\u{0E33}')
}

/// Any character in the Thai Unicode block (U+0E00–U+0E7F).
#[inline]
fn is_thai(c: char) -> bool {
    matches!(c, '\u{0E00}'..='\u{0E7F}')
}

// ---------------------------------------------------------------------------
// Cursor — encapsulates offset arithmetic for the scanner
// ---------------------------------------------------------------------------

/// A forward-only cursor over the characters of a `&str` slice.
///
/// `base` is the byte offset of the slice's start within the original string,
/// so `end` is always a valid offset into the original string.
struct Cursor<'a> {
    chars: core::iter::Peekable<core::str::CharIndices<'a>>,
    base: usize,
    /// Byte offset of the first character **not yet consumed**, relative to
    /// the original string. Updated by every call to [`advance`].
    end: usize,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str, pos: usize) -> Self {
        Self {
            chars: text[pos..].char_indices().peekable(),
            base: pos,
            end: pos,
        }
    }

    /// Peek at the next character without consuming it.
    #[inline]
    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    /// Consume the next character, update `end`, and return it.
    #[inline]
    fn advance(&mut self) -> Option<char> {
        let (off, c) = self.chars.next()?;
        self.end = self.base + off + c.len_utf8();
        Some(c)
    }

    /// Consume the next character only if `pred` returns `true` for it.
    #[inline]
    fn advance_if(&mut self, pred: impl Fn(char) -> bool) -> bool {
        match self.chars.peek() {
            Some(&(_, c)) if pred(c) => {
                self.advance();
                true
            }
            _ => false,
        }
    }

    /// Consume characters as long as `pred` holds.
    #[inline]
    fn advance_while(&mut self, pred: impl Fn(char) -> bool) {
        while self.advance_if(&pred) {}
    }
}

// ---------------------------------------------------------------------------
// Thai TCC sub-scanners
// ---------------------------------------------------------------------------

/// Consume a maximal run of non-Thai characters (one non-Thai TCC).
fn scan_non_thai(cur: &mut Cursor<'_>) {
    cur.advance_while(|c| !is_thai(c));
}

/// Consume the TCC "head": optional leading vowel + required consonant.
///
/// `first` is the character already consumed from `cur`.
/// Returns the base consonant, or `None` if `first` starts no valid Thai TCC
/// (lone leading vowel with nothing after it, or a lone non-consonant Thai char).
fn scan_head(cur: &mut Cursor<'_>, first: char) -> Option<char> {
    if is_lead_vowel(first) {
        // Leading vowel must be immediately followed by a consonant.
        match cur.peek() {
            Some(c) if is_consonant(c) => {
                cur.advance();
                Some(c)
            }
            // Lone leading vowel — ends the TCC right here.
            _ => None,
        }
    } else if is_consonant(first) {
        Some(first)
    } else {
        // Lone Thai non-consonant (digit, punctuation …) — single-char TCC.
        None
    }
}

/// Consume zero or more upper vowels / diacritic signs above the consonant.
fn scan_upper_vowels(cur: &mut Cursor<'_>) {
    cur.advance_while(is_upper_vowel);
}

/// Consume tone marks. Swallows duplicates that appear in malformed input.
fn scan_tone_marks(cur: &mut Cursor<'_>) {
    cur.advance_while(is_tone);
}

/// Consume the optional trailing diacritic: ์, อะ, อา, อำ, or อํ.
fn scan_trailing(cur: &mut Cursor<'_>) {
    cur.advance_if(|c| is_thanthakat(c) || is_follow_vowel(c) || is_nikhahit(c));
}

// ---------------------------------------------------------------------------
// Core TCC scanner
// ---------------------------------------------------------------------------

/// Scan one TCC starting at `pos` in `text` and return the byte offset of
/// the first character *after* the TCC.
///
/// Returns `None` only when `pos >= text.len()`.
fn scan_one_tcc(text: &str, pos: usize) -> Option<usize> {
    let mut cur = Cursor::new(text, pos);
    let first = cur.advance()?;

    // Non-Thai run → one flat TCC.
    if !is_thai(first) {
        scan_non_thai(&mut cur);
        return Some(cur.end);
    }

    // Thai TCC: LEAD? CONSONANT UPPER* TONE? TRAIL?
    let consonant = match scan_head(&mut cur, first) {
        Some(c) => c,
        // Lone leading vowel or non-consonant Thai char — TCC ends here.
        None => return Some(cur.end),
    };

    // ฤ (U+0E24) and ฦ (U+0E26) are standalone vowel-consonants; nothing attaches.
    if !matches!(consonant, '\u{0E24}' | '\u{0E26}') {
        scan_upper_vowels(&mut cur);
        scan_tone_marks(&mut cur);
        scan_trailing(&mut cur);
    }

    Some(cur.end)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the byte offsets of every TCC boundary in `text`.
///
/// The returned slice always starts with `0` and ends with `text.len()`.
/// Slicing `text` with consecutive pairs of offsets gives the individual TCCs.
///
/// # Example
///
/// ```rust
/// use kham_core::tcc::tcc_boundaries;
///
/// // "กิน" — กิ is one TCC (ก + อิ), น is another
/// let bounds = tcc_boundaries("กิน");
/// assert_eq!(bounds, vec![0, 6, 9]); // กิ=6 bytes, น=3 bytes
/// assert_eq!(*bounds.first().unwrap(), 0);
/// assert_eq!(*bounds.last().unwrap(), "กิน".len());
/// ```
///
/// ```rust
/// use kham_core::tcc::tcc_boundaries;
///
/// // Mixed script: "hi" (Latin) + "สวัสดี" (Thai)
/// let bounds = tcc_boundaries("hiสวัสดี");
/// // "hi" → one non-Thai TCC, then Thai TCCs
/// assert_eq!(bounds[0], 0);
/// assert_eq!(bounds[1], 2); // "hi" = 2 bytes
/// ```
pub fn tcc_boundaries(text: &str) -> Vec<usize> {
    if text.is_empty() {
        return vec![0];
    }

    let mut bounds = Vec::with_capacity(text.len() / 3 + 2);
    bounds.push(0);

    let mut pos = 0;
    while pos < text.len() {
        match scan_one_tcc(text, pos) {
            Some(next) if next > pos => {
                bounds.push(next);
                pos = next;
            }
            // Safety net: advance by one UTF-8 char to avoid infinite loop.
            _ => {
                let next = text[pos..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| pos + i)
                    .unwrap_or(text.len());
                bounds.push(next);
                pos = next;
            }
        }
    }

    bounds
}

/// Iterate over the TCCs in `text` as `&str` slices.
///
/// # Example
///
/// ```rust
/// use kham_core::tcc::tcc_iter;
///
/// // "เกม": เก (lead vowel เ + consonant ก) is TCC 1, ม is TCC 2
/// let tccs: Vec<&str> = tcc_iter("เกม").collect();
/// assert_eq!(tccs, vec!["เก", "ม"]);
/// ```
pub fn tcc_iter(text: &str) -> impl Iterator<Item = &str> {
    TccIter { text, pos: 0 }
}

struct TccIter<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Iterator for TccIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.text.len() {
            return None;
        }
        let end = scan_one_tcc(self.text, self.pos)?;
        let slice = &self.text[self.pos..end];
        self.pos = end;
        Some(slice)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn tccs(text: &str) -> Vec<&str> {
        tcc_iter(text).collect()
    }

    #[test]
    fn empty() {
        assert_eq!(tcc_boundaries(""), vec![0]);
        assert_eq!(tccs(""), Vec::<&str>::new());
    }

    #[test]
    fn single_consonant() {
        // ก = U+0E01, 3 bytes
        assert_eq!(tccs("ก"), vec!["ก"]);
    }

    #[test]
    fn consonant_upper_vowel() {
        // กิ = ก (U+0E01) + อิ (U+0E34) = 6 bytes → 1 TCC
        assert_eq!(tccs("กิ"), vec!["กิ"]);
    }

    #[test]
    fn consonant_upper_tone() {
        // กิ้ = ก + อิ + ้ = 9 bytes → 1 TCC
        assert_eq!(tccs("กิ้"), vec!["กิ้"]);
    }

    #[test]
    fn two_consonants() {
        // กน → 2 TCCs
        assert_eq!(tccs("กน"), vec!["ก", "น"]);
    }

    #[test]
    fn gin_two_tccs() {
        // กิน → กิ (TCC1) + น (TCC2)
        assert_eq!(tccs("กิน"), vec!["กิ", "น"]);
        let b = tcc_boundaries("กิน");
        assert_eq!(b, vec![0, 6, 9]);
    }

    #[test]
    fn lead_vowel() {
        // เก = เ + ก → 1 TCC (lead vowel attaches to following consonant)
        assert_eq!(tccs("เก"), vec!["เก"]);
    }

    #[test]
    fn lead_vowel_with_tone() {
        // เก้ = เ + ก + ้
        assert_eq!(tccs("เก้"), vec!["เก้"]);
    }

    #[test]
    fn follow_vowel_aa() {
        // กา = ก + อา → 1 TCC
        assert_eq!(tccs("กา"), vec!["กา"]);
    }

    #[test]
    fn follow_vowel_sara_am() {
        // กำ = ก + อำ → 1 TCC
        assert_eq!(tccs("กำ"), vec!["กำ"]);
    }

    #[test]
    fn thanthakat() {
        // กร์ = ก + ร + ์ → but ก and ร are separate consonants so:
        // ก (TCC1), ร์ (TCC2 — ร + thanthakat)
        assert_eq!(tccs("กร์"), vec!["ก", "ร์"]);
    }

    #[test]
    fn non_thai_run() {
        // "hello" → single non-Thai TCC
        assert_eq!(tccs("hello"), vec!["hello"]);
    }

    #[test]
    fn mixed_script() {
        // "hi" + กิน → ["hi", "กิ", "น"]
        assert_eq!(tccs("hiกิน"), vec!["hi", "กิ", "น"]);
    }

    #[test]
    fn thai_digit() {
        // ๑ (U+0E51) is a Thai digit — standalone TCC
        assert_eq!(tccs("๑"), vec!["๑"]);
    }

    #[test]
    fn sawasdee() {
        // สวัสดี — classic greeting, 5 chars, 3 TCCs: สวั สดี? Let's verify
        // ส (U+0E2A), ว (U+0E27), ั (U+0E31), ส (U+0E2A), ด (U+0E14), ี (U+0E35)
        // TCC1: สว ั → ส + วั? No — ั (upper vowel) attaches to preceding consonant ว
        // Actually: ส (TCC1), วั (TCC2), ส (TCC3), ดี (TCC4)
        let result = tccs("สวัสดี");
        // Verify coverage: joining all TCCs gives back original
        assert_eq!(result.join(""), "สวัสดี");
        // Verify count (4 TCCs for สวัสดี)
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn boundary_coverage() {
        // Every boundary pair must be valid UTF-8 slice of original
        let text = "ธนาคาร100แห่ง";
        let bounds = tcc_boundaries(text);
        // First and last are correct
        assert_eq!(bounds[0], 0);
        assert_eq!(*bounds.last().unwrap(), text.len());
        // All intermediate boundaries are valid char boundaries
        for &b in &bounds {
            assert!(
                text.is_char_boundary(b),
                "offset {b} is not a char boundary"
            );
        }
        // Joining the slices reconstructs the original
        let rebuilt: alloc::string::String = bounds.windows(2).map(|w| &text[w[0]..w[1]]).collect();
        assert_eq!(rebuilt, text);
    }
}
