//! Part-of-speech tagging for Thai words.
//!
//! [`PosTagger`] maps pre-segmented Thai words to their primary
//! [`PosTag`] using a hand-curated lookup table. This is a **table-driven**
//! approach — context-sensitive ML tagging is a future `#[cfg(feature = "ml")]`
//! extension.
//!
//! # Tags
//!
//! | Variant | TSV tag | Examples |
//! |---------|---------|---------|
//! | [`Noun`] | `NOUN` | คน บ้าน ปลา |
//! | [`Verb`] | `VERB` | กิน ทำ ไป |
//! | [`Adj`] | `ADJ` | ดี ใหญ่ สวย |
//! | [`Adv`] | `ADV` | มาก เร็ว เสมอ |
//! | [`Particle`] | `PART` | ครับ ค่ะ นะ |
//! | [`ProperNoun`] | `PROPN` | กรุงเทพ ไทย |
//! | [`Pronoun`] | `PRON` | ฉัน เขา เรา |
//! | [`Numeral`] | `NUM` | หนึ่ง สิบ ร้อย |
//! | [`Classifier`] | `CLAS` | ตัว ใบ อัน |
//! | [`Conjunction`] | `CONJ` | และ หรือ แต่ |
//! | [`Auxiliary`] | `AUX` | ได้ ต้อง กำลัง |
//! | [`Determiner`] | `DET` | นี้ นั้น ทุก |
//! | [`Preposition`] | `PREP` | ใน บน ตาม |
//!
//! # Data format
//!
//! Tab-separated text file, one entry per line:
//!
//! ```text
//! # Thai word<TAB>POS_TAG
//! กิน<TAB>VERB
//! ข้าว<TAB>NOUN
//! ดี<TAB>ADJ
//! ```
//!
//! Lines beginning with `#` and blank lines are ignored.
//! Duplicate keys: last entry wins.
//!
//! # Example
//!
//! ```rust
//! use kham_core::pos::{PosTagger, PosTag};
//!
//! let tagger = PosTagger::builtin();
//! assert_eq!(tagger.tag("กิน"), Some(PosTag::Verb));
//! assert_eq!(tagger.tag("ข้าว"), Some(PosTag::Noun));
//! assert_eq!(tagger.tag("xyz"), None);
//! ```

use alloc::collections::BTreeMap;
use alloc::string::String;

static BUILTIN_POS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pos_th.bin"));

/// Part-of-speech category for a Thai word.
///
/// Assigned by table lookup — reflects the primary/most common POS when a word
/// has multiple uses (e.g. ดี is tagged `Adj` even though it can modify verbs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PosTag {
    /// Common noun — คน บ้าน ปลา
    Noun,
    /// Verb — กิน ทำ ไป
    Verb,
    /// Adjective — ดี ใหญ่ สวย
    Adj,
    /// Adverb — มาก เร็ว เสมอ
    Adv,
    /// Sentence-final or modal particle — ครับ ค่ะ นะ
    Particle,
    /// Proper noun — กรุงเทพ ไทย
    ProperNoun,
    /// Pronoun — ฉัน เขา เรา
    Pronoun,
    /// Cardinal numeral — หนึ่ง สิบ ร้อย
    Numeral,
    /// Classifier (unit word) — ตัว ใบ อัน
    Classifier,
    /// Conjunction — และ หรือ แต่
    Conjunction,
    /// Auxiliary / modal verb — ได้ ต้อง กำลัง
    Auxiliary,
    /// Determiner / demonstrative — นี้ นั้น ทุก
    Determiner,
    /// Preposition / spatial word — ใน บน ตาม
    Preposition,
}

impl PosTag {
    /// Parse a TSV tag string (e.g. `"VERB"`) into a [`PosTag`].
    ///
    /// Returns `None` for unrecognised strings.
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "NOUN" => Some(Self::Noun),
            "VERB" => Some(Self::Verb),
            "ADJ" => Some(Self::Adj),
            "ADV" => Some(Self::Adv),
            "PART" => Some(Self::Particle),
            "PROPN" => Some(Self::ProperNoun),
            "PRON" => Some(Self::Pronoun),
            "NUM" => Some(Self::Numeral),
            "CLAS" => Some(Self::Classifier),
            "CONJ" => Some(Self::Conjunction),
            "AUX" => Some(Self::Auxiliary),
            "DET" => Some(Self::Determiner),
            "PREP" => Some(Self::Preposition),
            _ => None,
        }
    }

    /// The canonical TSV tag string for this variant.
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::Noun => "NOUN",
            Self::Verb => "VERB",
            Self::Adj => "ADJ",
            Self::Adv => "ADV",
            Self::Particle => "PART",
            Self::ProperNoun => "PROPN",
            Self::Pronoun => "PRON",
            Self::Numeral => "NUM",
            Self::Classifier => "CLAS",
            Self::Conjunction => "CONJ",
            Self::Auxiliary => "AUX",
            Self::Determiner => "DET",
            Self::Preposition => "PREP",
        }
    }

    /// A human-readable label for use in output and bindings.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noun => "Noun",
            Self::Verb => "Verb",
            Self::Adj => "Adj",
            Self::Adv => "Adv",
            Self::Particle => "Particle",
            Self::ProperNoun => "ProperNoun",
            Self::Pronoun => "Pronoun",
            Self::Numeral => "Numeral",
            Self::Classifier => "Classifier",
            Self::Conjunction => "Conjunction",
            Self::Auxiliary => "Auxiliary",
            Self::Determiner => "Determiner",
            Self::Preposition => "Preposition",
        }
    }
}

/// Table-driven POS tagger backed by a `BTreeMap<word, PosTag>`.
///
/// Construct once with [`PosTagger::builtin`] and reuse across calls.
pub struct PosTagger(BTreeMap<String, PosTag>);

impl PosTagger {
    /// Load the built-in POS table (hand-curated, ~200 common Thai words).
    pub fn builtin() -> Self {
        Self::from_tsv(&crate::decompress_builtin(BUILTIN_POS))
    }

    /// Parse a tab-separated POS table.
    ///
    /// Format: `thai_word\tPOS_TAG` — one entry per line.
    /// Lines beginning with `#` and blank lines are skipped.
    /// Unknown tag strings are skipped silently.
    /// For duplicate keys, the last entry wins.
    pub fn from_tsv(data: &str) -> Self {
        let mut map: BTreeMap<String, PosTag> = BTreeMap::new();
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
            let tag_str = match parts.next() {
                Some(t) if !t.is_empty() => t.trim(),
                _ => continue,
            };
            if let Some(tag) = PosTag::from_tag(tag_str) {
                map.insert(word, tag);
            }
        }
        PosTagger(map)
    }

    /// Look up the primary POS tag for a pre-segmented word.
    ///
    /// Returns `None` if the word is not in the table.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::pos::{PosTagger, PosTag};
    ///
    /// let tagger = PosTagger::from_tsv("กิน\tVERB\nข้าว\tNOUN\n");
    /// assert_eq!(tagger.tag("กิน"), Some(PosTag::Verb));
    /// assert_eq!(tagger.tag("xyz"), None);
    /// ```
    pub fn tag(&self, word: &str) -> Option<PosTag> {
        self.0.get(word).copied()
    }

    /// Number of entries in the table.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if the table has no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_map_is_non_empty() {
        let t = PosTagger::builtin();
        assert!(t.len() > 50, "expected at least 50 built-in entries");
    }

    #[test]
    fn common_nouns() {
        let t = PosTagger::builtin();
        assert_eq!(t.tag("ข้าว"), Some(PosTag::Noun));
        assert_eq!(t.tag("บ้าน"), Some(PosTag::Noun));
        assert_eq!(t.tag("น้ำ"), Some(PosTag::Noun));
    }

    #[test]
    fn common_verbs() {
        let t = PosTagger::builtin();
        assert_eq!(t.tag("กิน"), Some(PosTag::Verb));
        assert_eq!(t.tag("ดู"), Some(PosTag::Verb));
        assert_eq!(t.tag("ทำ"), Some(PosTag::Verb));
    }

    #[test]
    fn adjectives() {
        let t = PosTagger::builtin();
        assert_eq!(t.tag("ดี"), Some(PosTag::Adj));
        assert_eq!(t.tag("ใหญ่"), Some(PosTag::Adj));
    }

    #[test]
    fn conjunctions() {
        let t = PosTagger::builtin();
        assert_eq!(t.tag("และ"), Some(PosTag::Conjunction));
        assert_eq!(t.tag("หรือ"), Some(PosTag::Conjunction));
    }

    #[test]
    fn auxiliaries() {
        let t = PosTagger::builtin();
        assert_eq!(t.tag("ได้"), Some(PosTag::Auxiliary));
        assert_eq!(t.tag("ต้อง"), Some(PosTag::Auxiliary));
        assert_eq!(t.tag("กำลัง"), Some(PosTag::Auxiliary));
    }

    #[test]
    fn unknown_word_returns_none() {
        let t = PosTagger::builtin();
        assert_eq!(t.tag("เปปซี่"), None);
        assert_eq!(t.tag(""), None);
    }

    #[test]
    fn from_tsv_last_duplicate_wins() {
        let t = PosTagger::from_tsv("ดี\tADJ\nดี\tADV\n");
        assert_eq!(t.tag("ดี"), Some(PosTag::Adv));
    }

    #[test]
    fn from_tsv_unknown_tag_skipped() {
        let t = PosTagger::from_tsv("กิน\tXXX\n");
        assert_eq!(t.tag("กิน"), None);
    }

    #[test]
    fn from_tsv_comment_and_blank_skipped() {
        let t = PosTagger::from_tsv("# comment\n\nกิน\tVERB\n");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn from_tsv_empty_input() {
        assert!(PosTagger::from_tsv("").is_empty());
    }

    #[test]
    fn pos_tag_roundtrip() {
        let tags = [
            PosTag::Noun,
            PosTag::Verb,
            PosTag::Adj,
            PosTag::Adv,
            PosTag::Particle,
            PosTag::ProperNoun,
            PosTag::Pronoun,
            PosTag::Numeral,
            PosTag::Classifier,
            PosTag::Conjunction,
            PosTag::Auxiliary,
            PosTag::Determiner,
            PosTag::Preposition,
        ];
        for tag in tags {
            assert_eq!(PosTag::from_tag(tag.as_tag()), Some(tag));
        }
    }
}
