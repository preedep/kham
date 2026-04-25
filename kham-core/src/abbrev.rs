//! Thai abbreviation expansion.
//!
//! [`AbbrevMap`] maps abbreviated Thai forms (including dot-containing patterns
//! such as `ก.ค.` and `พ.ศ.`) to their canonical expansions. It is used in two
//! ways:
//!
//! 1. **Pre-tokenisation** — [`AbbrevMap::expand_text`] scans raw text and
//!    replaces every known abbreviation with its primary expansion before the
//!    text is passed to the segmenter. This is how the FTS pipeline uses it.
//!
//! 2. **Post-tokenisation lookup** — [`AbbrevMap::lookup`] checks whether a
//!    single already-segmented token is a known abbreviation and returns all
//!    its expansions.
//!
//! ## Data format
//!
//! A tab-separated file, one rule per line:
//!
//! ```text
//! # abbreviated_form    primary_expansion   [alt1   alt2...]
//! ก.ค.    กรกฎาคม
//! พ.ศ.    พุทธศักราช
//! ดร.     ดอกเตอร์
//! อ.      อาจารย์     อำเภอ
//! ```
//!
//! Lines beginning with `#` and blank lines are ignored. Duplicate keys are
//! merged — later entries append to the expansion list.
//!
//! ## Greedy matching
//!
//! [`expand_text`] performs greedy **longest-first** matching: at each position
//! the longest matching key is consumed. This ensures compound forms like
//! `ศ.ดร.` (11 bytes) shadow the shorter `ศ.` (4 bytes) when both are in the
//! map.
//!
//! # Examples
//!
//! ```rust
//! use kham_core::abbrev::AbbrevMap;
//!
//! let map = AbbrevMap::builtin();
//!
//! // Pre-tokenisation expansion
//! assert_eq!(map.expand_text("วันที่5ก.ค.2567"), "วันที่5กรกฎาคม2567");
//! assert_eq!(map.expand_text("พ.ศ.2567"), "พุทธศักราช2567");
//!
//! // Single-token lookup
//! let exps = map.lookup("ดร.").unwrap();
//! assert_eq!(exps, &["ดอกเตอร์"]);
//! ```
//!
//! [`expand_text`]: AbbrevMap::expand_text

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Reverse;

// ---------------------------------------------------------------------------
// AbbrevMap
// ---------------------------------------------------------------------------

/// Abbreviation lookup and expansion table.
///
/// Construct once via [`AbbrevMap::builtin`] or [`AbbrevMap::from_tsv`] and
/// reuse across calls.
pub struct AbbrevMap {
    /// O(log n) lookup by exact key.
    map: BTreeMap<String, Vec<String>>,
    /// Keys paired with their primary expansion, sorted by byte-length DESC.
    /// Used by [`expand_text`] for greedy longest-first scanning.
    ///
    /// [`expand_text`]: AbbrevMap::expand_text
    sorted: Vec<(String, String)>,
}

impl AbbrevMap {
    /// Create an empty map with no entries.
    pub fn empty() -> Self {
        Self {
            map: BTreeMap::new(),
            sorted: Vec::new(),
        }
    }

    /// Load the built-in Thai abbreviation dictionary.
    ///
    /// Includes month abbreviations (`ม.ค.`–`ธ.ค.`), era markers (`พ.ศ.`,
    /// `ค.ศ.`), academic titles (`ดร.`, `ศ.`, `รศ.`, `ผศ.`), and common
    /// administrative abbreviations (`จ.`, `ต.`, `ถ.`, `ซ.`).
    pub fn builtin() -> Self {
        Self::from_tsv(include_str!("../data/abbrev_th.tsv"))
    }

    /// Parse a tab-separated abbreviation table.
    ///
    /// Format: `abbrev\tprimary_expansion[\talt1\talt2…]` — one rule per line.
    /// Lines beginning with `#` and blank lines are skipped.
    /// Duplicate keys are merged: later entries append to the expansion list
    /// and the *last* duplicate's first column becomes the primary expansion
    /// used by [`expand_text`].
    pub fn from_tsv(data: &str) -> Self {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let key = match cols.next() {
                Some(k) if !k.is_empty() => String::from(k),
                _ => continue,
            };
            let expansions: Vec<String> = cols
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            if expansions.is_empty() {
                continue;
            }
            map.entry(key).or_default().extend(expansions);
        }

        // Build the sorted pairs (key, primary_expansion) for expand_text.
        // Primary = first expansion in the merged list.
        let mut sorted: Vec<(String, String)> =
            map.iter().map(|(k, v)| (k.clone(), v[0].clone())).collect();
        // Longest key first so greedy matching prefers ศ.ดร. over ศ.
        sorted.sort_by_key(|pair| Reverse(pair.0.len()));

        Self { map, sorted }
    }

    /// Return all expansions for `abbrev`, or `None` if not in the map.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::abbrev::AbbrevMap;
    ///
    /// let map = AbbrevMap::builtin();
    /// let exps = map.lookup("ก.ค.").unwrap();
    /// assert_eq!(exps[0], "กรกฎาคม");
    ///
    /// // Ambiguous abbreviation — multiple expansions.
    /// let exps = map.lookup("อ.").unwrap();
    /// assert!(exps.contains(&"อาจารย์".to_string()));
    ///
    /// assert_eq!(map.lookup("unknown"), None);
    /// ```
    pub fn lookup(&self, abbrev: &str) -> Option<&[String]> {
        self.map.get(abbrev).map(Vec::as_slice)
    }

    /// Scan `text` and replace every occurrence of a known abbreviation key
    /// with its **primary** expansion (the first expansion in the TSV row).
    ///
    /// Matching is **greedy and longest-first**: at each position the longest
    /// matching key is consumed before advancing. Characters that do not start
    /// any key are copied through unchanged.
    ///
    /// Returns an owned [`String`]. Allocates only when at least one replacement
    /// is made; otherwise returns a copy of the input.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kham_core::abbrev::AbbrevMap;
    ///
    /// let map = AbbrevMap::builtin();
    ///
    /// assert_eq!(map.expand_text("ก.ค."), "กรกฎาคม");
    /// assert_eq!(map.expand_text("5ก.ค.2567"), "5กรกฎาคม2567");
    /// assert_eq!(map.expand_text("ไม่มีอะไร"), "ไม่มีอะไร");
    ///
    /// // Compound title: ศ.ดร. matched before ศ.
    /// assert_eq!(map.expand_text("ศ.ดร.สมชาย"), "ศาสตราจารย์ดอกเตอร์สมชาย");
    /// ```
    pub fn expand_text(&self, text: &str) -> String {
        if self.sorted.is_empty() || text.is_empty() {
            return String::from(text);
        }

        let mut result = String::with_capacity(text.len());
        let mut i = 0usize; // byte position

        'outer: while i < text.len() {
            let remaining = &text[i..];
            for (key, expansion) in &self.sorted {
                if remaining.starts_with(key.as_str()) {
                    result.push_str(expansion);
                    i += key.len();
                    continue 'outer;
                }
            }
            // No key matched — copy one Unicode scalar value through unchanged.
            let c = remaining.chars().next().unwrap();
            result.push(c);
            i += c.len_utf8();
        }

        result
    }

    /// Return the number of entries in the map.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Return `true` if the map contains no entries.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mini() -> AbbrevMap {
        AbbrevMap::from_tsv("ก.ค.\tกรกฎาคม\nพ.ศ.\tพุทธศักราช\nดร.\tดอกเตอร์\nศ.ดร.\tศาสตราจารย์ดอกเตอร์\nศ.\tศาสตราจารย์\nอ.\tอาจารย์\tอำเภอ\n")
    }

    // ── construction ─────────────────────────────────────────────────────────

    #[test]
    fn empty_has_no_entries() {
        let m = AbbrevMap::empty();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn from_tsv_parses_entries() {
        let m = mini();
        assert!(!m.is_empty());
        assert!(m.len() >= 5);
    }

    #[test]
    fn from_tsv_skips_comments_and_blanks() {
        let m = AbbrevMap::from_tsv("# comment\n\nก.ค.\tกรกฎาคม\n");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn from_tsv_skips_lines_without_expansion() {
        let m = AbbrevMap::from_tsv("ก.ค.\n");
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn from_tsv_duplicate_keys_merge() {
        let m = AbbrevMap::from_tsv("อ.\tอาจารย์\nอ.\tอำเภอ\n");
        let exps = m.lookup("อ.").unwrap();
        assert!(exps.contains(&String::from("อาจารย์")));
        assert!(exps.contains(&String::from("อำเภอ")));
    }

    // ── lookup ────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_known_key() {
        let m = mini();
        let exps = m.lookup("ก.ค.").expect("ก.ค. should be in map");
        assert_eq!(exps, &[String::from("กรกฎาคม")]);
    }

    #[test]
    fn lookup_unknown_key_returns_none() {
        let m = mini();
        assert_eq!(m.lookup("xyz"), None);
    }

    #[test]
    fn lookup_ambiguous_returns_all() {
        let m = mini();
        let exps = m.lookup("อ.").unwrap();
        assert!(exps.contains(&String::from("อาจารย์")));
        assert!(exps.contains(&String::from("อำเภอ")));
    }

    // ── expand_text ───────────────────────────────────────────────────────────

    #[test]
    fn expand_single_abbreviation() {
        let m = mini();
        assert_eq!(m.expand_text("ก.ค."), "กรกฎาคม");
    }

    #[test]
    fn expand_in_context() {
        let m = mini();
        assert_eq!(m.expand_text("5ก.ค.2567"), "5กรกฎาคม2567");
    }

    #[test]
    fn expand_multiple_abbreviations() {
        let m = mini();
        assert_eq!(m.expand_text("พ.ศ.2567ก.ค."), "พุทธศักราช2567กรกฎาคม");
    }

    #[test]
    fn expand_no_match_returns_original() {
        let m = mini();
        assert_eq!(m.expand_text("ไม่มีอะไร"), "ไม่มีอะไร");
    }

    #[test]
    fn expand_empty_input() {
        let m = mini();
        assert_eq!(m.expand_text(""), "");
    }

    #[test]
    fn expand_greedy_longest_first() {
        // ศ.ดร. (longer) must win over ศ. (shorter) at the same position.
        let m = mini();
        assert_eq!(m.expand_text("ศ.ดร.สมชาย"), "ศาสตราจารย์ดอกเตอร์สมชาย");
    }

    #[test]
    fn expand_shorter_key_after_no_long_match() {
        // ศ. alone (no ดร. following) → ศาสตราจารย์
        let m = mini();
        assert_eq!(m.expand_text("ศ.สมชาย"), "ศาสตราจารย์สมชาย");
    }

    #[test]
    fn expand_empty_map_returns_original() {
        let m = AbbrevMap::empty();
        assert_eq!(m.expand_text("ก.ค."), "ก.ค.");
    }

    // ── builtin ───────────────────────────────────────────────────────────────

    #[test]
    fn builtin_has_all_months() {
        let m = AbbrevMap::builtin();
        let months = [
            ("ม.ค.", "มกราคม"),
            ("ก.พ.", "กุมภาพันธ์"),
            ("มี.ค.", "มีนาคม"),
            ("เม.ย.", "เมษายน"),
            ("พ.ค.", "พฤษภาคม"),
            ("มิ.ย.", "มิถุนายน"),
            ("ก.ค.", "กรกฎาคม"),
            ("ส.ค.", "สิงหาคม"),
            ("ก.ย.", "กันยายน"),
            ("ต.ค.", "ตุลาคม"),
            ("พ.ย.", "พฤศจิกายน"),
            ("ธ.ค.", "ธันวาคม"),
        ];
        for (abbr, expected) in months {
            let exps = m
                .lookup(abbr)
                .unwrap_or_else(|| panic!("{abbr} missing from builtin"));
            assert_eq!(
                exps[0], expected,
                "primary expansion of {abbr} should be {expected}"
            );
        }
    }

    #[test]
    fn builtin_has_era_markers() {
        let m = AbbrevMap::builtin();
        assert!(m.lookup("พ.ศ.").is_some());
        assert!(m.lookup("ค.ศ.").is_some());
    }

    #[test]
    fn builtin_expands_date_sentence() {
        let m = AbbrevMap::builtin();
        let result = m.expand_text("วันที่5ก.ค.พ.ศ.2567");
        assert!(result.contains("กรกฎาคม"), "got: {result}");
        assert!(result.contains("พุทธศักราช"), "got: {result}");
    }

    // ── month ambiguity — ต.ค. over ต. ──────────────────────────────────────

    #[test]
    fn october_matches_before_tambon() {
        // ต.ค. (ตุลาคม) must beat ต. (ตำบล) at the same position.
        let m = AbbrevMap::builtin();
        let result = m.expand_text("ต.ค.นี้");
        assert_eq!(result, "ตุลาคมนี้", "got: {result}");
    }

    #[test]
    fn tambon_matches_when_not_october() {
        let m = AbbrevMap::builtin();
        // ต.สุขุมวิท — ต. not followed by ค. → expands to ตำบล
        let result = m.expand_text("ต.สุขุมวิท");
        assert_eq!(result, "ตำบลสุขุมวิท", "got: {result}");
    }

    // ── police ranks ──────────────────────────────────────────────────────────

    #[test]
    fn police_generals_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("พล.ต.อ.วิชัย"), "พลตำรวจเอกวิชัย");
        assert_eq!(m.expand_text("พล.ต.ท.สมชาย"), "พลตำรวจโทสมชาย");
        assert_eq!(m.expand_text("พล.ต.ต.สมศรี"), "พลตำรวจตรีสมศรี");
    }

    #[test]
    fn police_officers_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("พ.ต.อ.ณรงค์"), "พันตำรวจเอกณรงค์");
        assert_eq!(m.expand_text("ร.ต.อ.มานะ"), "ร้อยตำรวจเอกมานะ");
        assert_eq!(m.expand_text("ด.ต.ประสิทธิ์"), "ดาบตำรวจประสิทธิ์");
    }

    #[test]
    fn police_ncos_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("ส.ต.อ.บุญมี"), "สิบตำรวจเอกบุญมี");
        assert_eq!(m.expand_text("ส.ต.ต.สุรชัย"), "สิบตำรวจตรีสุรชัย");
    }

    // ── army ranks ────────────────────────────────────────────────────────────

    #[test]
    fn army_generals_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("พล.อ.ประยุทธ์"), "พลเอกประยุทธ์");
        assert_eq!(m.expand_text("พล.ท.สกล"), "พลโทสกล");
        assert_eq!(m.expand_text("พล.ต.ชาติ"), "พลตรีชาติ");
    }

    #[test]
    fn army_officers_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("พ.อ.วีระ"), "พันเอกวีระ");
        assert_eq!(m.expand_text("ร.อ.ธนู"), "ร้อยเอกธนู");
        assert_eq!(m.expand_text("จ.ส.อ.สมพร"), "จ่าสิบเอกสมพร");
    }

    // ── police vs army disambiguation ─────────────────────────────────────────

    #[test]
    fn police_longer_form_shadows_army_shorter_form() {
        let m = AbbrevMap::builtin();
        // พล.ต.อ. (police general) must NOT expand as พล.ต. (army major general)
        let result = m.expand_text("พล.ต.อ.สมบัติ");
        assert_eq!(result, "พลตำรวจเอกสมบัติ", "got: {result}");
        // พล.ต. standalone → army
        let result2 = m.expand_text("พล.ต.วิเชียร");
        assert_eq!(result2, "พลตรีวิเชียร", "got: {result2}");
    }

    #[test]
    fn พตอ_is_police_not_army() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("พ.ต.อ.กล้า"), "พันตำรวจเอกกล้า");
        // พ.ต. alone is army
        assert_eq!(m.expand_text("พ.ต.ดำ"), "พันตรีดำ");
    }

    // ── navy and air force ────────────────────────────────────────────────────

    #[test]
    fn navy_admirals_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("พล.ร.อ.ชุมพล"), "พลเรือเอกชุมพล");
        assert_eq!(m.expand_text("พล.ร.ต.สิทธิ"), "พลเรือตรีสิทธิ");
    }

    #[test]
    fn airforce_generals_shadow_army_พลอ() {
        let m = AbbrevMap::builtin();
        // พล.อ.อ. must expand as Air Force, not as พล.อ. (Army) + อ.
        assert_eq!(m.expand_text("พล.อ.อ.ประจิน"), "พลอากาศเอกประจิน");
        assert_eq!(m.expand_text("พล.อ.ท.มานัต"), "พลอากาศโทมานัต");
    }

    // ── state enterprises ─────────────────────────────────────────────────────

    #[test]
    fn state_enterprises_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.lookup("กฟผ.").unwrap()[0], "การไฟฟ้าฝ่ายผลิตแห่งประเทศไทย");
        assert_eq!(m.lookup("กฟน.").unwrap()[0], "การไฟฟ้านครหลวง");
        assert_eq!(m.lookup("กฟภ.").unwrap()[0], "การไฟฟ้าส่วนภูมิภาค");
        assert_eq!(m.lookup("รฟท.").unwrap()[0], "การรถไฟแห่งประเทศไทย");
        assert_eq!(
            m.lookup("รฟม.").unwrap()[0],
            "การรถไฟฟ้าขนส่งมวลชนแห่งประเทศไทย"
        );
        assert_eq!(m.lookup("ปตท.").unwrap()[0], "การปิโตรเลียมแห่งประเทศไทย");
    }

    #[test]
    fn banking_agencies_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(
            m.lookup("ธ.ก.ส.").unwrap()[0],
            "ธนาคารเพื่อการเกษตรและสหกรณ์การเกษตร"
        );
        assert_eq!(m.lookup("ธอส.").unwrap()[0], "ธนาคารอาคารสงเคราะห์");
        assert_eq!(m.lookup("กบข.").unwrap()[0], "กองทุนบำเหน็จบำนาญข้าราชการ");
    }

    // ── government agencies ───────────────────────────────────────────────────

    #[test]
    fn government_agencies_expand() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.lookup("กทม.").unwrap()[0], "กรุงเทพมหานคร");
        assert_eq!(m.lookup("กกต.").unwrap()[0], "คณะกรรมการการเลือกตั้ง");
        assert_eq!(
            m.lookup("ป.ป.ช.").unwrap()[0],
            "คณะกรรมการป้องกันและปราบปรามการทุจริตแห่งชาติ"
        );
        assert_eq!(
            m.lookup("สสส.").unwrap()[0],
            "สำนักงานกองทุนสนับสนุนการสร้างเสริมสุขภาพ"
        );
    }

    #[test]
    fn กทม_expands_in_text() {
        let m = AbbrevMap::builtin();
        assert_eq!(m.expand_text("กทม."), "กรุงเทพมหานคร");
        assert_eq!(m.expand_text("ผู้ว่ากทม."), "ผู้ว่ากรุงเทพมหานคร");
    }
}
