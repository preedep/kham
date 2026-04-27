//! Thai spell correction using edit distance and phonetic ranking.
//!
//! [`SpellChecker`] finds words in the built-in dictionary that are close to a
//! given input, ranked first by phonetic similarity (lk82 Soundex), then by
//! edit distance, then by TNC corpus frequency.
//!
//! ```rust
//! use kham_core::spell::SpellChecker;
//!
//! let checker = SpellChecker::builtin();
//! let suggestions = checker.suggestions("กาน", 5);
//! // All candidates are within edit distance 2
//! assert!(suggestions.iter().all(|s| s.edit_distance <= 2));
//! // Phonetically similar words appear first
//! if let Some(first) = suggestions.first() {
//!     assert!(first.soundex_match || first.edit_distance <= 1);
//! }
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use crate::dict::BUILTIN_WORDS;
use crate::freq::FreqMap;
use crate::soundex::lk82;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A spelling suggestion returned by [`SpellChecker::suggestions`].
///
/// Suggestions are sorted by:
/// 1. `soundex_match` descending — phonetically similar words first
/// 2. `edit_distance` ascending — closer edits first
/// 3. `freq_score` descending — more common words first
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Candidate word from the built-in dictionary.
    pub word: String,
    /// Levenshtein edit distance between the input and this candidate.
    pub edit_distance: u8,
    /// `true` if the lk82 phonetic codes of the input and candidate match.
    pub soundex_match: bool,
    /// TNC frequency score; `0` if the word is not in the frequency table.
    pub freq_score: u32,
}

/// Thai spell checker using edit distance and lk82 phonetic ranking.
///
/// Candidates are drawn from the built-in 62k-word dictionary and filtered to
/// edit distance ≤ 2. Phonetically similar candidates (matching lk82 code) are
/// ranked above purely orthographic matches.
///
/// # Examples
///
/// ```rust
/// use kham_core::spell::SpellChecker;
///
/// let checker = SpellChecker::builtin();
///
/// // Misspelled word — expect near-miss suggestions
/// let suggestions = checker.suggestions("กานข้าว", 5);
/// assert!(suggestions.iter().all(|s| s.edit_distance <= 2));
/// ```
///
/// Correctly spelled word — edit_distance 0 if it is in the dictionary:
///
/// ```rust
/// use kham_core::spell::SpellChecker;
///
/// let checker = SpellChecker::builtin();
/// let suggestions = checker.suggestions("กิน", 10);
/// assert!(suggestions.iter().any(|s| s.word == "กิน" && s.edit_distance == 0));
/// ```
pub struct SpellChecker {
    words_text: &'static str,
    freq: FreqMap,
}

impl SpellChecker {
    /// Create a spell checker backed by the built-in dictionary and TNC
    /// frequency table.
    ///
    /// Construction loads the TNC frequency map (~106k entries) — reuse the
    /// returned instance rather than calling `builtin()` on every query.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::spell::SpellChecker;
    ///
    /// let checker = SpellChecker::builtin();
    /// assert!(!checker.suggestions("สวัดสี", 5).is_empty());
    /// ```
    pub fn builtin() -> Self {
        Self {
            words_text: BUILTIN_WORDS,
            freq: FreqMap::builtin(),
        }
    }

    /// Return up to `max_n` spelling suggestions for `word`.
    ///
    /// Only candidates with edit distance ≤ 2 are returned. Results are sorted
    /// by phonetic match (lk82), then edit distance, then TNC frequency.
    ///
    /// Returns an empty `Vec` when `word` is empty or `max_n` is zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::spell::SpellChecker;
    ///
    /// let checker = SpellChecker::builtin();
    ///
    /// // Empty input → no suggestions
    /// assert!(checker.suggestions("", 5).is_empty());
    ///
    /// // max_n = 0 → no suggestions
    /// assert!(checker.suggestions("กาน", 0).is_empty());
    ///
    /// // Results respect the distance threshold
    /// let suggs = checker.suggestions("กิน", 10);
    /// assert!(suggs.iter().all(|s| s.edit_distance <= 2));
    /// ```
    pub fn suggestions(&self, word: &str, max_n: usize) -> Vec<Suggestion> {
        if word.is_empty() || max_n == 0 {
            return Vec::new();
        }

        let word_chars: Vec<char> = word.chars().collect();
        let word_len = word_chars.len();
        let input_lk82 = lk82(word);

        let mut results: Vec<Suggestion> = Vec::new();

        for line in self.words_text.lines() {
            let candidate = line.trim();
            if candidate.is_empty() || candidate.starts_with('#') {
                continue;
            }

            // Length pre-filter: Levenshtein distance ≤ 2 implies |len_a - len_b| ≤ 2.
            let cand_len = candidate.chars().count();
            if cand_len + 2 < word_len || word_len + 2 < cand_len {
                continue;
            }

            let dist = levenshtein(&word_chars, candidate, 2);
            if dist > 2 {
                continue;
            }

            let soundex_match = lk82(candidate) == input_lk82;
            let freq_score = self.freq.get(candidate);

            results.push(Suggestion {
                word: String::from(candidate),
                edit_distance: dist as u8,
                soundex_match,
                freq_score,
            });
        }

        // Sort: soundex_match DESC, edit_distance ASC, freq_score DESC
        results.sort_unstable_by(|a, b| {
            b.soundex_match
                .cmp(&a.soundex_match)
                .then(a.edit_distance.cmp(&b.edit_distance))
                .then(b.freq_score.cmp(&a.freq_score))
        });

        results.truncate(max_n);
        results
    }
}

// ---------------------------------------------------------------------------
// Levenshtein distance (character-level, with early exit)
// ---------------------------------------------------------------------------

/// Compute Levenshtein distance between `a_chars` (pre-collected) and `b` (str).
///
/// Returns `max_dist + 1` as soon as the running minimum exceeds `max_dist`,
/// avoiding unnecessary work for clearly dissimilar pairs.
fn levenshtein(a_chars: &[char], b: &str, max_dist: usize) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // Length difference is a hard lower bound on edit distance.
    if m.abs_diff(n) > max_dist {
        return max_dist + 1;
    }

    // Two-row DP. `prev[j]` = edit distance for a[0..i-1] vs b[0..j].
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = alloc::vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        let mut row_min = i;

        for j in 1..=n {
            let substitution_cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + substitution_cost);
            if curr[j] < row_min {
                row_min = curr[j];
            }
        }

        // Every cell in this row exceeds max_dist — no path can recover.
        if row_min > max_dist {
            return max_dist + 1;
        }

        core::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> SpellChecker {
        SpellChecker::builtin()
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(checker().suggestions("", 5).is_empty());
    }

    #[test]
    fn zero_max_n_returns_empty() {
        assert!(checker().suggestions("กาน", 0).is_empty());
    }

    #[test]
    fn all_results_within_threshold() {
        let suggs = checker().suggestions("กาน", 20);
        assert!(
            suggs.iter().all(|s| s.edit_distance <= 2),
            "got out-of-range result: {suggs:?}"
        );
    }

    #[test]
    fn respects_max_n() {
        let suggs = checker().suggestions("กาน", 3);
        assert!(suggs.len() <= 3);
    }

    #[test]
    fn word_in_dict_appears_as_dist_zero() {
        // "กิน" is definitely in the built-in dictionary.
        let suggs = checker().suggestions("กิน", 10);
        let exact = suggs.iter().find(|s| s.word == "กิน");
        assert!(exact.is_some(), "expected กิน in suggestions");
        assert_eq!(exact.unwrap().edit_distance, 0);
    }

    #[test]
    fn sorted_soundex_first_then_distance() {
        let suggs = checker().suggestions("กาน", 20);
        // Verify sort order: soundex DESC, edit_distance ASC, freq DESC
        for window in suggs.windows(2) {
            let (a, b) = (&window[0], &window[1]);
            let ok = a.soundex_match > b.soundex_match
                || (a.soundex_match == b.soundex_match && a.edit_distance < b.edit_distance)
                || (a.soundex_match == b.soundex_match
                    && a.edit_distance == b.edit_distance
                    && a.freq_score >= b.freq_score);
            assert!(ok, "sort order violated: {a:?} before {b:?}");
        }
    }

    #[test]
    fn transposition_within_distance_two() {
        // "สวสัดี" is a character transposition of "สวัสดี" (ั and ส swapped)
        // Standard Levenshtein counts this as 2 edits — within the threshold.
        let suggs = checker().suggestions("สวสัดี", 5);
        let hit = suggs.iter().find(|s| s.word == "สวัสดี");
        assert!(
            hit.is_some(),
            "expected สวัสดี in suggestions for สวสัดี; got: {suggs:?}"
        );
        assert!(hit.unwrap().edit_distance <= 2);
    }

    #[test]
    fn single_char_deletion_suggestion() {
        // "กินข้า" is "กินข้าว" with the final ว deleted — edit distance 1.
        let suggs = checker().suggestions("กินข้า", 5);
        let hit = suggs.iter().find(|s| s.word == "กินข้าว");
        assert!(
            hit.is_some(),
            "expected กินข้าว in suggestions for กินข้า; got: {suggs:?}"
        );
        assert_eq!(hit.unwrap().edit_distance, 1);
    }

    // Levenshtein unit tests ------------------------------------------------

    fn lev(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        levenshtein(&a_chars, b, 255)
    }

    #[test]
    fn lev_identical() {
        assert_eq!(lev("กิน", "กิน"), 0);
    }

    #[test]
    fn lev_single_substitution() {
        assert_eq!(lev("กาน", "กาล"), 1);
    }

    #[test]
    fn lev_single_deletion() {
        assert_eq!(lev("กินข้าว", "กินข้า"), 1);
    }

    #[test]
    fn lev_single_insertion() {
        assert_eq!(lev("กินข้า", "กินข้าว"), 1);
    }

    #[test]
    fn lev_transposition_is_two() {
        // Standard Levenshtein: transposition = delete + insert = 2
        assert_eq!(lev("สวสัดี", "สวัสดี"), 2);
    }

    #[test]
    fn lev_empty_strings() {
        assert_eq!(lev("", ""), 0);
        assert_eq!(lev("กิน", ""), 3);
        assert_eq!(lev("", "กิน"), 3);
    }

    #[test]
    fn lev_early_exit_above_max() {
        let a_chars: Vec<char> = "กิน".chars().collect();
        // Distance is 5 but max_dist is 2 — should return 3 (max_dist + 1).
        assert_eq!(levenshtein(&a_chars, "สวัสดีครับ", 2), 3);
    }
}
