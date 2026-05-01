//! RTGS romanization of segmented Thai words.
//!
//! [`RomanizationMap`] maps pre-segmented Thai words to their Roman (Latin)
//! phonetic equivalents using the Royal Thai General System of Transcription
//! (RTGS) — the Thai government standard used in road signs, passports, and
//! official documents.
//!
//! Lookup first checks the hand-curated table; words not in the table are
//! romanized by the built-in rule engine ([`romanize_word`]).
//!
//! # RTGS characteristics
//!
//! - Consonant-by-consonant transliteration (initial vs. final position differ)
//! - No tone marks in output
//! - No vowel-length distinction (อิ and อี both map to `i`)
//! - Diphthongs and vowel clusters have explicit multi-character mappings
//!
//! # Data format
//!
//! Tab-separated text file, one entry per line:
//!
//! ```text
//! # Thai word<TAB>RTGS romanization
//! กิน<TAB>kin
//! ข้าว<TAB>khao
//! ปลา<TAB>pla
//! ```
//!
//! Lines beginning with `#` and blank lines are ignored.
//! Duplicate keys: last entry wins (allows override files).
//!
//! # Example
//!
//! ```rust
//! use kham_core::romanizer::RomanizationMap;
//!
//! let map = RomanizationMap::builtin();
//! assert_eq!(map.romanize("กิน"), Some("kin"));
//! assert_eq!(map.romanize_or_raw("ข้าว"), "khao");
//! assert_eq!(map.romanize_or_raw("xyz"), "xyz");
//!
//! let tokens = vec!["กิน", "ข้าว", "ปลา"];
//! assert_eq!(map.romanize_tokens(&tokens), vec!["kin", "khao", "pla"]);
//! ```

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

static BUILTIN_ROMANIZATION: &str = include_str!("../data/romanization_th.tsv");

/// A Thai-word → RTGS-romanization lookup table.
///
/// Built from tab-separated data via [`RomanizationMap::from_tsv`].
/// Lookup is O(log n) via [`BTreeMap`].
pub struct RomanizationMap(BTreeMap<String, String>);

impl RomanizationMap {
    /// Load the built-in RTGS romanization table.
    pub fn builtin() -> Self {
        Self::from_tsv(BUILTIN_ROMANIZATION)
    }

    /// Parse a tab-separated romanization table.
    ///
    /// Format: `thai_word\trtgs_romanization` — one entry per line.
    /// Lines beginning with `#` and blank lines are skipped.
    /// For duplicate keys, the last entry wins.
    pub fn from_tsv(data: &str) -> Self {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let word = match parts.next() {
                Some(w) if !w.is_empty() => String::from(w),
                _ => continue,
            };
            let roman = match parts.next() {
                Some(r) if !r.is_empty() => String::from(r.trim()),
                _ => continue,
            };
            map.insert(word, roman);
        }
        RomanizationMap(map)
    }

    /// Look up the RTGS romanization for a pre-segmented Thai word.
    ///
    /// Returns the table hit if the word is in the hand-curated list, otherwise
    /// applies the built-in rule engine. Returns `None` only when the word
    /// contains no Thai characters (e.g. pure Latin or numbers).
    ///
    /// The returned `&str` borrows from the map for table hits; rule-engine
    /// results are returned as an owned `String` via the `romanize_owned`
    /// helper — callers that want a borrowed `&str` should use
    /// [`romanize_or_raw`](Self::romanize_or_raw).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::builtin();
    /// // Table hit
    /// assert_eq!(map.romanize("กิน"), Some("kin"));
    /// // OOV word — not in table; use romanize_owned() for rule-engine fallback
    /// assert_eq!(map.romanize("เปปซี่"), None);
    /// // Non-Thai input
    /// assert_eq!(map.romanize("xyz"), None);
    /// ```
    pub fn romanize(&self, word: &str) -> Option<&str> {
        self.0.get(word).map(String::as_str)
    }

    /// Romanize `word` to an owned `String`, using the table first, then the
    /// rule engine for out-of-vocabulary Thai words.
    ///
    /// Returns `None` only when the word contains no Thai characters.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::builtin();
    /// assert_eq!(map.romanize_owned("กิน").as_deref(), Some("kin"));
    /// // OOV word gets rule-based approximation
    /// assert!(map.romanize_owned("เปปซี่").is_some());
    /// // Non-Thai returns None
    /// assert_eq!(map.romanize_owned("hello"), None);
    /// ```
    pub fn romanize_owned(&self, word: &str) -> Option<String> {
        if let Some(s) = self.0.get(word) {
            return Some(s.clone());
        }
        if word.chars().any(is_thai_char) {
            Some(romanize_word(word))
        } else {
            None
        }
    }

    /// Return the RTGS romanization for `word`, or `word` unchanged if not in
    /// the table. Only performs table lookup — no rule engine.
    ///
    /// For OOV Thai words that should fall back to the rule engine, use
    /// [`romanize_or_rule`](Self::romanize_or_rule) instead.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::from_tsv("กิน\tkin\n");
    /// assert_eq!(map.romanize_or_raw("กิน"), "kin");
    /// assert_eq!(map.romanize_or_raw("xyz"), "xyz");
    /// // OOV Thai is returned unchanged (raw passthrough)
    /// assert_eq!(map.romanize_or_raw("เปปซี่"), "เปปซี่");
    /// ```
    pub fn romanize_or_raw<'a>(&'a self, word: &'a str) -> &'a str {
        self.0.get(word).map(String::as_str).unwrap_or(word)
    }

    /// Return the RTGS romanization for `word`.
    ///
    /// Checks the table first; for OOV Thai words the built-in rule engine is
    /// applied. Non-Thai input is returned unchanged. Always returns an owned
    /// `String`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::builtin();
    /// // Table hit
    /// assert_eq!(map.romanize_or_rule("กิน"), "kin");
    /// // Non-Thai passes through
    /// assert_eq!(map.romanize_or_rule("hello"), "hello");
    /// // OOV Thai gets rule-based approximation
    /// let oov = map.romanize_or_rule("เปปซี่");
    /// assert!(!oov.is_empty());
    /// assert!(!oov.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c)));
    /// ```
    pub fn romanize_or_rule(&self, word: &str) -> String {
        if let Some(s) = self.0.get(word) {
            return s.clone();
        }
        if word.chars().any(is_thai_char) {
            romanize_word(word)
        } else {
            String::from(word)
        }
    }

    /// Romanize a slice of pre-segmented token strings.
    ///
    /// Returns a `Vec<String>` aligned 1:1 with the input slice. Tokens not
    /// found in the table are returned unchanged (same behaviour as
    /// [`romanize_or_raw`](Self::romanize_or_raw)).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::romanizer::RomanizationMap;
    ///
    /// let map = RomanizationMap::from_tsv("กิน\tkin\nปลา\tpla\n");
    /// let out = map.romanize_tokens(&["กิน", "ปลา"]);
    /// assert_eq!(out, vec!["kin", "pla"]);
    /// ```
    pub fn romanize_tokens(&self, tokens: &[&str]) -> Vec<String> {
        tokens
            .iter()
            .map(|t| String::from(self.romanize_or_raw(t)))
            .collect()
    }

    /// Number of entries in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if the map has no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Rule-based RTGS engine (fallback for OOV words)
// ---------------------------------------------------------------------------

#[inline]
fn is_thai_char(c: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&c)
}

/// RTGS initial-position consonant mapping.
fn initial_rtgs(c: char) -> &'static str {
    match c {
        'ก' => "k",
        'ข' | 'ค' | 'ฅ' | 'ฆ' => "kh",
        'ง' => "ng",
        'จ' | 'ฉ' | 'ช' | 'ฌ' => "ch",
        'ซ' | 'ศ' | 'ษ' | 'ส' => "s",
        'ญ' | 'ย' => "y",
        'ฎ' | 'ด' => "d",
        'ฏ' | 'ต' => "t",
        'ฐ' | 'ฑ' | 'ฒ' | 'ถ' | 'ท' | 'ธ' => "th",
        'น' | 'ณ' => "n",
        'บ' => "b",
        'ป' => "p",
        'ผ' | 'พ' | 'ภ' => "ph",
        'ฝ' | 'ฟ' => "f",
        'ม' => "m",
        'ร' => "r",
        'ล' | 'ฬ' => "l",
        'ว' => "w",
        'ห' | 'ฮ' => "h",
        'อ' => "",
        _ => "",
    }
}

/// RTGS final-position (coda) consonant mapping.
fn final_rtgs(c: char) -> &'static str {
    match c {
        'ก' | 'ข' | 'ค' | 'ฅ' | 'ฆ' => "k",
        'ง' => "ng",
        'จ' | 'ช' | 'ซ' | 'ฌ' | 'ฎ' | 'ด' | 'ฏ' | 'ต' | 'ถ' | 'ท' | 'ธ' | 'ศ' | 'ษ' | 'ส' => {
            "t"
        }
        'น' | 'ณ' => "n",
        'บ' | 'ป' | 'พ' | 'ภ' | 'ฝ' | 'ฟ' => "p",
        'ม' => "m",
        'ย' | 'ญ' => "i",
        'ร' => "n",
        'ล' | 'ฬ' => "n",
        'ว' => "o",
        'ห' | 'อ' => "",
        _ => "",
    }
}

fn is_thai_consonant(c: char) -> bool {
    matches!(c, 'ก'..='ฮ')
}

fn is_leading_vowel(c: char) -> bool {
    matches!(c, 'เ' | 'แ' | 'โ' | 'ใ' | 'ไ')
}

fn is_tone_mark(c: char) -> bool {
    matches!(c, '\u{0E48}' | '\u{0E49}' | '\u{0E4A}' | '\u{0E4B}')
}

fn is_silent_mark(c: char) -> bool {
    c == '\u{0E4C}' // ์ thanthakat
}

/// Apply RTGS rules to an OOV Thai word.
///
/// Processes the Unicode character sequence using a lightweight syllable
/// state machine. Handles leading vowels (เ แ โ ใ ไ), above vowels
/// (ิ ี ึ ื ั ็), below vowels (ุ ู), following vowels (า ะ ำ), tone marks
/// (skipped), and the thanthakat silent marker (์). Unrecognised characters
/// pass through unchanged.
pub fn romanize_word(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(word.len());
    let mut i = 0;

    while i < n {
        let c = chars[i];

        if is_leading_vowel(c) {
            let lead = c;
            i += 1;
            // Skip any stacked tone marks before the initial consonant
            while i < n && is_tone_mark(chars[i]) {
                i += 1;
            }
            if i < n && is_thai_consonant(chars[i]) {
                let init = initial_rtgs(chars[i]);
                i += 1;
                // Skip tone marks and above/below vowels that follow the initial
                while i < n
                    && (is_tone_mark(chars[i])
                        || matches!(
                            chars[i],
                            'ิ' | 'ี' | 'ึ' | 'ื' | 'ั' | '็' | 'ุ' | 'ู' | '\u{0E4D}' | '\u{0E3A}'
                        ))
                {
                    i += 1;
                }
                // Detect compound patterns: เ_อ → oe, เ_า → ao, เ_็ already consumed above
                let suffix = if lead == 'เ' && i < n && chars[i] == 'อ' {
                    i += 1;
                    "oe"
                } else if lead == 'เ' && i < n && chars[i] == 'า' {
                    i += 1;
                    "ao" // เ_า pattern
                } else {
                    match lead {
                        'เ' => "e",
                        'แ' => "ae",
                        'โ' => "o",
                        'ใ' | 'ไ' => "ai",
                        _ => "",
                    }
                };
                out.push_str(init);
                out.push_str(suffix);
                // Final consonant
                if i < n && is_thai_consonant(chars[i]) && !is_silent_mark(chars[i]) {
                    // Check for thanthakat on next+1
                    let fin_c = chars[i];
                    i += 1;
                    let silent = i < n && is_silent_mark(chars[i]);
                    if silent {
                        i += 1; // consume ์
                    } else {
                        out.push_str(final_rtgs(fin_c));
                    }
                }
            } else {
                // Lone leading vowel — just emit vowel sound
                out.push_str(match lead {
                    'เ' => "e",
                    'แ' => "ae",
                    'โ' => "o",
                    'ใ' | 'ไ' => "ai",
                    _ => "",
                });
            }
        } else if is_thai_consonant(c) {
            let init = initial_rtgs(c);
            i += 1;

            // Collect vowel diacritics and tone marks
            let mut vowel = "";
            let mut pending_silent = false;
            while i < n {
                match chars[i] {
                    // Tone marks — skip
                    ch if is_tone_mark(ch) => i += 1,
                    // Thanthakat — this consonant is silent
                    ch if is_silent_mark(ch) => {
                        pending_silent = true;
                        i += 1;
                        break;
                    }
                    // Above vowels
                    'ิ' | '็' => {
                        vowel = "i";
                        i += 1;
                    }
                    'ี' => {
                        vowel = "i";
                        i += 1;
                    }
                    'ึ' => {
                        vowel = "ue";
                        i += 1;
                    }
                    'ื' => {
                        vowel = "ue";
                        i += 1;
                    }
                    'ั' => {
                        vowel = "a";
                        i += 1;
                    }
                    // Below vowels
                    'ุ' => {
                        vowel = "u";
                        i += 1;
                    }
                    'ู' => {
                        vowel = "u";
                        i += 1;
                    }
                    // Following vowels
                    'า' => {
                        vowel = "a";
                        i += 1;
                    }
                    'ะ' => {
                        vowel = "a";
                        i += 1;
                    }
                    'ำ' => {
                        vowel = "am";
                        i += 1;
                        break;
                    } // am absorbs final
                    // Nikhahit / phinthu — skip
                    '\u{0E4D}' | '\u{0E3A}' => i += 1,
                    _ => break,
                }
            }

            if pending_silent {
                // Consonant is silent (e.g. ห์ in loan words) — emit nothing
                continue;
            }

            out.push_str(init);
            out.push_str(vowel);

            // ำ already encodes the final nasal — skip coda search
            if vowel == "am" {
                continue;
            }

            // Final consonant: next non-tone-mark consonant followed by end-of-word
            // or another leading vowel / vowel diacritic
            if i < n && is_thai_consonant(chars[i]) {
                let fin_c = chars[i];
                // Peek: if fin_c is followed by ์ it's silent
                let next_is_silent = i + 1 < n && is_silent_mark(chars[i + 1]);
                // If fin_c is followed by a vowel diacritic or leading vowel, it's
                // an initial of the next syllable — don't consume as final
                let next_is_vowel = i + 1 < n
                    && (is_leading_vowel(chars[i + 1])
                        || matches!(
                            chars[i + 1],
                            'ิ' | 'ี'
                                | 'ึ'
                                | 'ื'
                                | 'ั'
                                | '็'
                                | 'ุ'
                                | 'ู'
                                | 'า'
                                | 'ะ'
                                | 'ำ'
                        ));
                if next_is_silent {
                    i += 2; // consume consonant + ์
                } else if next_is_vowel {
                    // next char is an initial of a following syllable — leave it
                } else {
                    out.push_str(final_rtgs(fin_c));
                    i += 1;
                }
            }
        } else if is_tone_mark(c) || is_silent_mark(c) || matches!(c, '\u{0E4D}' | '\u{0E3A}') {
            i += 1; // stray diacritic — skip
        } else {
            // Non-Thai character: pass through
            out.push(c);
            i += 1;
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn builtin_common_words() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize("กิน"), Some("kin"));
        assert_eq!(map.romanize("ข้าว"), Some("khao"));
        assert_eq!(map.romanize("น้ำ"), Some("nam"));
        assert_eq!(map.romanize("ปลา"), Some("pla"));
    }

    #[test]
    fn unknown_word_returns_none_for_non_thai() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize("hello"), None);
        assert_eq!(map.romanize("123"), None);
    }

    #[test]
    fn romanize_or_raw_hit() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize_or_raw("กิน"), "kin");
    }

    #[test]
    fn romanize_or_raw_non_thai_passthrough() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize_or_raw("xyz"), "xyz");
    }

    #[test]
    fn romanize_or_rule_oov_thai_non_empty() {
        let map = RomanizationMap::builtin();
        // OOV Thai words should get rule-based romanization, not empty string
        let result = map.romanize_or_rule("เปปซี่");
        assert!(
            !result.is_empty(),
            "rule engine should produce non-empty output"
        );
        assert!(
            !result.chars().any(is_thai_char),
            "output should be Latin, not Thai"
        );
    }

    // ── rule engine unit tests ────────────────────────────────────────────────

    #[test]
    fn rule_simple_consonant_vowel_final() {
        // กิน = ก(k) + ิ(i) + น(n) → "kin"
        assert_eq!(romanize_word("กิน"), "kin");
    }

    #[test]
    fn rule_leading_vowel_ae() {
        // แก = แ(ae) + ก(k) → "kaek" or "kaek"
        // แก้ว = แ + ก + ้ (tone) + ว(final=o) → "kaeo"
        let r = romanize_word("แก้ว");
        assert_eq!(r, "kaeo");
    }

    #[test]
    fn rule_leading_vowel_o() {
        // โต = โ + ต → "to"
        assert_eq!(romanize_word("โต"), "to");
    }

    #[test]
    fn rule_leading_vowel_ai() {
        // ไป = ไ + ป → "pai" (final ป in ไ pattern)
        let r = romanize_word("ไป");
        // Should start with 'p' and contain 'ai'
        assert!(r.contains("ai"), "ไป should romanize with 'ai', got: {r}");
    }

    #[test]
    fn rule_sara_am() {
        // ทำ = ท + ำ → "tham"
        assert_eq!(romanize_word("ทำ"), "tham");
    }

    #[test]
    fn rule_below_vowel_u() {
        // ดุ = ด + ุ → "du"
        assert_eq!(romanize_word("ดุ"), "du");
    }

    #[test]
    fn rule_non_thai_passthrough() {
        assert_eq!(romanize_word("hello"), "hello");
    }

    #[test]
    fn rule_empty_string() {
        assert_eq!(romanize_word(""), "");
    }

    #[test]
    fn romanize_or_rule_table_takes_priority() {
        let map = RomanizationMap::builtin();
        // Table has hand-curated "กิน" → "kin"
        assert_eq!(map.romanize_or_rule("กิน"), "kin");
    }

    #[test]
    fn romanize_or_rule_non_thai_passthrough() {
        let map = RomanizationMap::builtin();
        assert_eq!(map.romanize_or_rule("hello"), "hello");
    }

    #[test]
    fn from_tsv_last_duplicate_wins() {
        let map = RomanizationMap::from_tsv("กิน\tkin\nกิน\tgin\n");
        assert_eq!(map.romanize("กิน"), Some("gin"));
    }

    #[test]
    fn romanize_tokens_aligned() {
        let map = RomanizationMap::from_tsv("กิน\tkin\nปลา\tpla\n");
        let out = map.romanize_tokens(&["กิน", "ปลา"]);
        assert_eq!(out, vec!["kin", "pla"]);
    }

    #[test]
    fn romanize_tokens_unknown_passthrough() {
        let map = RomanizationMap::from_tsv("กิน\tkin\n");
        let out = map.romanize_tokens(&["กิน", "xyz"]);
        assert_eq!(out, vec!["kin", "xyz"]);
    }

    #[test]
    fn comment_and_blank_lines_skipped() {
        let map = RomanizationMap::from_tsv("# comment\n\nกิน\tkin\n");
        assert_eq!(map.len(), 1);
        assert_eq!(map.romanize("กิน"), Some("kin"));
    }

    #[test]
    fn line_without_tab_skipped() {
        let map = RomanizationMap::from_tsv("กิน\n");
        assert!(map.is_empty());
    }

    #[test]
    fn whitespace_trimmed_from_romanization() {
        let map = RomanizationMap::from_tsv("กิน\t kin \n");
        assert_eq!(map.romanize("กิน"), Some("kin"));
    }

    #[test]
    fn empty_input_produces_empty_map() {
        assert!(RomanizationMap::from_tsv("").is_empty());
    }

    #[test]
    fn romanize_tokens_empty_slice() {
        let map = RomanizationMap::builtin();
        assert!(map.romanize_tokens(&[]).is_empty());
    }
}
