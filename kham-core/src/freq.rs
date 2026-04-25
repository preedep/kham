//! Word frequency table built from the Thai National Corpus (TNC).
//!
//! The built-in data comes from `tnc_freq.txt` (CC0), which maps Thai words to
//! their raw occurrence counts in the TNC. Frequencies are used by the newmm
//! DAG scorer to prefer more common segmentations when multiple paths have the
//! same number of dictionary matches.

use alloc::collections::BTreeMap;
use alloc::string::String;

static BUILTIN_FREQ_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tnc_freq.bin"));

/// A word→frequency lookup table backed by the Thai National Corpus (TNC).
///
/// Frequencies are raw TNC occurrence counts. Words absent from the table
/// return 0, which is a safe default (the DP scorer simply ignores them).
///
/// The built-in table is loaded with [`FreqMap::builtin`]. Custom tables can
/// be constructed from any tab-separated `word\tcount` text via [`FreqMap::from_tsv`].
pub struct FreqMap(BTreeMap<String, u32>);

impl FreqMap {
    /// Parse a tab-separated `word\tcount` text (one entry per line).
    pub fn from_tsv(data: &str) -> Self {
        let mut map = BTreeMap::new();
        for line in data.lines() {
            if let Some((word, freq_str)) = line.split_once('\t') {
                if let Ok(freq) = freq_str.trim().parse::<u32>() {
                    map.insert(String::from(word), freq);
                }
            }
        }
        FreqMap(map)
    }

    /// Load the built-in TNC frequency table.
    ///
    /// Parses the embedded `tnc_freq.txt` (106k entries) into a [`BTreeMap`].
    /// This is a pay-once startup cost; the returned [`FreqMap`] should be
    /// reused across segmentation calls (e.g. stored in [`Tokenizer`]).
    ///
    /// [`Tokenizer`]: crate::Tokenizer
    pub fn builtin() -> Self {
        Self::from_tsv(&crate::decompress_builtin(BUILTIN_FREQ_DATA))
    }

    /// Look up a word's frequency; returns 0 if not found.
    #[inline]
    pub fn get(&self, word: &str) -> u32 {
        self.0.get(word).copied().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_tsv parsing ──────────────────────────────────────────────────────

    #[test]
    fn parses_tab_separated_entries() {
        let m = FreqMap::from_tsv("กิน\t1234\nข้าว\t5678\n");
        assert_eq!(m.get("กิน"), 1234);
        assert_eq!(m.get("ข้าว"), 5678);
    }

    #[test]
    fn blank_lines_are_skipped() {
        let m = FreqMap::from_tsv("\n\nกิน\t10\n\n");
        assert_eq!(m.get("กิน"), 10);
    }

    #[test]
    fn line_without_tab_is_skipped() {
        let m = FreqMap::from_tsv("noop\nกิน\t42\n");
        assert_eq!(m.get("noop"), 0);
        assert_eq!(m.get("กิน"), 42);
    }

    #[test]
    fn non_numeric_count_is_skipped() {
        let m = FreqMap::from_tsv("กิน\tabc\nข้าว\t99\n");
        assert_eq!(m.get("กิน"), 0);
        assert_eq!(m.get("ข้าว"), 99);
    }

    #[test]
    fn later_duplicate_overwrites_earlier() {
        let m = FreqMap::from_tsv("กิน\t10\nกิน\t99\n");
        assert_eq!(m.get("กิน"), 99);
    }

    #[test]
    fn whitespace_trimmed_from_count() {
        let m = FreqMap::from_tsv("กิน\t  42  \n");
        assert_eq!(m.get("กิน"), 42);
    }

    // ── get edge cases ────────────────────────────────────────────────────────

    #[test]
    fn unknown_word_returns_zero() {
        let m = FreqMap::from_tsv("กิน\t100\n");
        assert_eq!(m.get("xyz"), 0);
    }

    #[test]
    fn empty_lookup_returns_zero() {
        let m = FreqMap::from_tsv("กิน\t100\n");
        assert_eq!(m.get(""), 0);
    }

    #[test]
    fn empty_input_produces_empty_map() {
        let m = FreqMap::from_tsv("");
        assert_eq!(m.get("กิน"), 0);
    }

    // ── built-in data ─────────────────────────────────────────────────────────

    #[test]
    fn builtin_loads_without_panic() {
        let _ = FreqMap::builtin();
    }

    #[test]
    fn builtin_has_expected_entry_count() {
        let m = FreqMap::builtin();
        let count = m.0.len();
        assert!(count > 100_000, "expected >100k TNC entries, got {count}");
    }

    #[test]
    fn builtin_common_words_have_nonzero_freq() {
        let m = FreqMap::builtin();
        for word in &["กิน", "ข้าว", "ไป", "มา", "คน", "ที่", "นี้"]
        {
            assert!(
                m.get(word) > 0,
                "expected '{word}' to have non-zero TNC freq"
            );
        }
    }

    #[test]
    fn builtin_unknown_word_returns_zero() {
        let m = FreqMap::builtin();
        assert_eq!(m.get("กขคงจฉชซ"), 0);
    }

    #[test]
    fn builtin_high_freq_words_outrank_rare_words() {
        let m = FreqMap::builtin();
        // "ที่" (relativiser, extremely common) should rank above "มะม่วงหิมพานต์"
        assert!(
            m.get("ที่") > m.get("มะม่วงหิมพานต์"),
            "expected 'ที่' to have higher TNC freq than 'มะม่วงหิมพานต์'"
        );
    }

    // ── freq influences segmentation (integration) ────────────────────────────

    #[test]
    fn fewer_tokens_preferred_over_split_components() {
        use crate::Tokenizer;
        use alloc::vec::Vec;
        // "ตากลม" is in the dictionary as a compound word (1 token).
        // Fewer-tokens priority means the compound wins over ตา|กลม or ตาก|ลม (2 tokens each).
        // This matches PyThaiNLP newmm behaviour.
        let tok = Tokenizer::new();
        let tokens = tok.segment("ตากลม");
        let words: Vec<&str> = tokens.iter().map(|t| t.text).collect();
        assert_eq!(
            words,
            alloc::vec!["ตากลม"],
            "compound word should be preferred over split — got {words:?}"
        );
    }
}
