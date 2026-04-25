//! Thai date normalization.
//!
//! Parses Thai date strings (Buddhist Era or Gregorian) into a structured [`ThaiDate`]
//! and formats them back to ISO 8601 or Thai text.
//!
//! ## Supported input formats
//!
//! | Format | Example |
//! |--------|---------|
//! | Day full-month year | `5 กรกฎาคม 2567` |
//! | Day abbreviated-month year | `5 ก.ค. 2567` |
//! | With era marker | `5 ก.ค. พ.ศ. 2567` |
//! | With วันที่ prefix | `วันที่ 5 กรกฎาคม 2567` |
//! | Slash-separated | `5/7/2567` |
//! | Dash-separated | `5-7-2567` |
//! | Thai digits | `๕ ก.ค. ๒๕๖๗` |
//!
//! ## Era inference
//!
//! When no era marker is present, the year is heuristically classified:
//! - year ≥ 2300 → Buddhist Era (พ.ศ.)
//! - year < 2300 → Gregorian (ค.ศ.)
//!
//! ## Examples
//!
//! ```rust
//! use kham_core::date::{parse_thai_date, Era};
//!
//! let d = parse_thai_date("5 กรกฎาคม 2567").unwrap();
//! assert_eq!(d.day, 5);
//! assert_eq!(d.month, 7);
//! assert_eq!(d.year, 2567);
//! assert!(matches!(d.era, Era::Buddhist));
//! assert_eq!(d.to_iso8601(), "2024-07-05");
//!
//! let d2 = parse_thai_date("5/7/2567").unwrap();
//! assert_eq!(d2.to_iso8601(), "2024-07-05");
//! ```

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::number::thai_digit_to_ascii;

// (pattern_bytes, month_number) — sorted longest-first at build time
static MONTHS_BY_LEN: &[(&str, u8)] = &[
    // Full names — longest first
    ("กุมภาพันธ์", 2),
    ("พฤศจิกายน", 11),
    ("กรกฎาคม", 7),
    ("มิถุนายน", 6),
    ("สิงหาคม", 8),
    ("ธันวาคม", 12),
    ("เมษายน", 4),
    ("มีนาคม", 3),
    ("มกราคม", 1),
    ("กันยายน", 9),
    ("ตุลาคม", 10),
    ("พฤษภาคม", 5),
    // Abbreviated forms (dot-terminated)
    ("มี.ค.", 3),
    ("เม.ย.", 4),
    ("มิ.ย.", 6),
    ("ม.ค.", 1),
    ("ก.พ.", 2),
    ("พ.ค.", 5),
    ("ก.ค.", 7),
    ("ส.ค.", 8),
    ("ก.ย.", 9),
    ("ต.ค.", 10),
    ("พ.ย.", 11),
    ("ธ.ค.", 12),
];

/// Era system for a [`ThaiDate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    /// Buddhist Era (พุทธศักราช, พ.ศ.) — CE + 543.
    Buddhist,
    /// Gregorian / Christian Era (คริสต์ศักราช, ค.ศ.).
    Gregorian,
}

/// A parsed Thai date with day, month, year, and era.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThaiDate {
    /// Day of month (1–31).
    pub day: u8,
    /// Month number (1–12).
    pub month: u8,
    /// Year in the stated [`Era`].
    pub year: u32,
    /// Era system for this date.
    pub era: Era,
}

impl ThaiDate {
    /// Returns the year in Gregorian (CE).
    ///
    /// Buddhist years subtract 543; Gregorian years are returned as-is.
    pub fn gregorian_year(&self) -> i32 {
        match self.era {
            Era::Buddhist => self.year as i32 - 543,
            Era::Gregorian => self.year as i32,
        }
    }

    /// Returns the year in Buddhist Era (BE).
    ///
    /// Gregorian years add 543; Buddhist years are returned as-is.
    pub fn buddhist_year(&self) -> u32 {
        match self.era {
            Era::Buddhist => self.year,
            Era::Gregorian => self.year + 543,
        }
    }

    /// Formats the date as an ISO 8601 string (`YYYY-MM-DD`) using Gregorian year.
    ///
    /// ```rust
    /// use kham_core::date::parse_thai_date;
    /// let d = parse_thai_date("5 กรกฎาคม 2567").unwrap();
    /// assert_eq!(d.to_iso8601(), "2024-07-05");
    /// ```
    pub fn to_iso8601(&self) -> String {
        let y = self.gregorian_year();
        let mut out = String::with_capacity(10);
        // year — 4 digits, possibly negative (rare)
        if y < 0 {
            out.push('-');
            push_padded(&mut out, (-y) as u32, 4);
        } else {
            push_padded(&mut out, y as u32, 4);
        }
        out.push('-');
        push_padded(&mut out, self.month as u32, 2);
        out.push('-');
        push_padded(&mut out, self.day as u32, 2);
        out
    }

    /// Formats the date as Thai text: `"5 กรกฎาคม พ.ศ. 2567"`.
    ///
    /// The year is always expressed in Buddhist Era.
    ///
    /// ```rust
    /// use kham_core::date::parse_thai_date;
    /// let d = parse_thai_date("5/7/2567").unwrap();
    /// assert_eq!(d.to_thai_text(), "5 กรกฎาคม พ.ศ. 2567");
    /// ```
    pub fn to_thai_text(&self) -> String {
        let month_name = MONTH_NAMES_FULL[(self.month as usize) - 1];
        let be = self.buddhist_year();
        let mut out = String::new();
        push_decimal(&mut out, self.day as u32);
        out.push(' ');
        out.push_str(month_name);
        out.push_str(" พ.ศ. ");
        push_decimal(&mut out, be);
        out
    }
}

static MONTH_NAMES_FULL: &[&str] = &[
    "มกราคม",
    "กุมภาพันธ์",
    "มีนาคม",
    "เมษายน",
    "พฤษภาคม",
    "มิถุนายน",
    "กรกฎาคม",
    "สิงหาคม",
    "กันยายน",
    "ตุลาคม",
    "พฤศจิกายน",
    "ธันวาคม",
];

// ── Parsing ──────────────────────────────────────────────────────────────────

/// Parses a Thai date string into a [`ThaiDate`].
///
/// Returns `None` if the input cannot be recognized as a valid date.
///
/// ```rust
/// use kham_core::date::{parse_thai_date, Era};
///
/// let d = parse_thai_date("๕ ก.ค. ๒๕๖๗").unwrap();
/// assert_eq!(d.day, 5);
/// assert_eq!(d.month, 7);
/// assert_eq!(d.year, 2567);
/// assert!(matches!(d.era, Era::Buddhist));
/// ```
pub fn parse_thai_date(text: &str) -> Option<ThaiDate> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // Convert Thai digits to ASCII uniformly before parsing
    let ascii = to_ascii_digits(text);
    let s = ascii.trim();

    // Try numeric formats first (D/M/Y and D-M-Y)
    if let Some(d) = parse_numeric(s) {
        return Some(d);
    }
    // Try word-based format
    parse_word_based(s)
}

/// Formats a Buddhist Era date as Thai text: `"5 กรกฎาคม พ.ศ. 2567"`.
///
/// Returns `None` if `month` is not in 1–12 or `day` is not in 1–31.
pub fn format_thai_date(year_be: u32, month: u8, day: u8) -> Option<String> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let d = ThaiDate {
        day,
        month,
        year: year_be,
        era: Era::Buddhist,
    };
    Some(d.to_thai_text())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Converts every Thai digit character in `s` to its ASCII equivalent.
fn to_ascii_digits(s: &str) -> String {
    s.chars()
        .map(|c| thai_digit_to_ascii(c).unwrap_or(c))
        .collect()
}

/// Appends `n` to `out` without leading zeros.
fn push_decimal(out: &mut String, n: u32) {
    let mut buf: Vec<u8> = Vec::new();
    if n == 0 {
        out.push('0');
        return;
    }
    let mut v = n;
    while v > 0 {
        buf.push(b'0' + (v % 10) as u8);
        v /= 10;
    }
    buf.reverse();
    for b in buf {
        out.push(b as char);
    }
}

/// Appends `n` zero-padded to `width` into `out`.
fn push_padded(out: &mut String, n: u32, width: usize) {
    let mut buf: Vec<u8> = Vec::new();
    let mut v = n;
    if v == 0 {
        buf.push(b'0');
    } else {
        while v > 0 {
            buf.push(b'0' + (v % 10) as u8);
            v /= 10;
        }
    }
    buf.reverse();
    let pad = width.saturating_sub(buf.len());
    for _ in 0..pad {
        out.push('0');
    }
    for b in buf {
        out.push(b as char);
    }
}

/// Infers era from a bare year number.
fn infer_era(year: u32) -> Era {
    if year >= 2300 {
        Era::Buddhist
    } else {
        Era::Gregorian
    }
}

/// Parses `D/M/Y` or `D-M-Y` (separator can be `/` or `-`).
fn parse_numeric(s: &str) -> Option<ThaiDate> {
    let sep = if s.contains('/') {
        '/'
    } else if s.contains('-') {
        '-'
    } else {
        return None;
    };

    let parts: Vec<&str> = s.splitn(3, sep).collect();
    if parts.len() != 3 {
        return None;
    }
    let day: u8 = parts[0].trim().parse().ok()?;
    let month: u8 = parts[1].trim().parse().ok()?;
    let year: u32 = parts[2].trim().parse().ok()?;

    if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
        return None;
    }
    let era = infer_era(year);
    Some(ThaiDate {
        day,
        month,
        year,
        era,
    })
}

/// Parses word-based formats like `5 กรกฎาคม 2567` or `วันที่ 5 ก.ค. พ.ศ. 2567`.
fn parse_word_based(s: &str) -> Option<ThaiDate> {
    // Strip optional วันที่ prefix (14 UTF-8 bytes — 5 chars × 3 bytes + space × 1 byte? No, วันที่ = 4 Thai chars = 12 bytes, plus space)
    let s = s
        .strip_prefix("วันที่")
        .map(|t| t.trim_start())
        .unwrap_or(s);

    // Consume leading digits (day)
    let (day_str, rest) = take_digits(s);
    if day_str.is_empty() {
        return None;
    }
    let day: u8 = day_str.parse().ok()?;
    if !(1..=31).contains(&day) {
        return None;
    }

    let rest = rest.trim_start();

    // Match month name (greedy longest-first)
    let (month, rest) = match_month(rest)?;

    let rest = rest.trim_start();

    // Optional era marker
    let (era_opt, rest) = parse_era_marker(rest);
    let rest = rest.trim_start();

    // Year
    let (year_str, _) = take_digits(rest);
    if year_str.is_empty() {
        return None;
    }
    let year: u32 = year_str.parse().ok()?;

    let era = era_opt.unwrap_or_else(|| infer_era(year));

    Some(ThaiDate {
        day,
        month,
        year,
        era,
    })
}

/// Consumes leading ASCII digit characters and returns `(digits, remainder)`.
fn take_digits(s: &str) -> (&str, &str) {
    let end = s
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    (&s[..end], &s[end..])
}

/// Tries to match a month name at the start of `s` (longest-first).
/// Returns `(month_number, remainder)` or `None`.
fn match_month(s: &str) -> Option<(u8, &str)> {
    for &(pattern, month) in MONTHS_BY_LEN {
        if let Some(rest) = s.strip_prefix(pattern) {
            return Some((month, rest));
        }
    }
    None
}

/// Strips a leading era marker (`พ.ศ.` or `ค.ศ.`) and returns `(Some(era), rest)`.
/// If no marker is found returns `(None, original)`.
fn parse_era_marker(s: &str) -> (Option<Era>, &str) {
    if let Some(rest) = s.strip_prefix("พ.ศ.") {
        return (Some(Era::Buddhist), rest);
    }
    if let Some(rest) = s.strip_prefix("ค.ศ.") {
        return (Some(Era::Gregorian), rest);
    }
    (None, s)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Era conversion ───────────────────────────────────────────────────────

    #[test]
    fn buddhist_to_gregorian() {
        let d = ThaiDate {
            day: 1,
            month: 1,
            year: 2567,
            era: Era::Buddhist,
        };
        assert_eq!(d.gregorian_year(), 2024);
    }

    #[test]
    fn gregorian_to_buddhist() {
        let d = ThaiDate {
            day: 1,
            month: 1,
            year: 2024,
            era: Era::Gregorian,
        };
        assert_eq!(d.buddhist_year(), 2567);
    }

    // ── ISO 8601 formatting ──────────────────────────────────────────────────

    #[test]
    fn iso8601_buddhist() {
        let d = ThaiDate {
            day: 5,
            month: 7,
            year: 2567,
            era: Era::Buddhist,
        };
        assert_eq!(d.to_iso8601(), "2024-07-05");
    }

    #[test]
    fn iso8601_single_digit_day_month() {
        let d = ThaiDate {
            day: 3,
            month: 3,
            year: 2567,
            era: Era::Buddhist,
        };
        assert_eq!(d.to_iso8601(), "2024-03-03");
    }

    #[test]
    fn iso8601_gregorian() {
        let d = ThaiDate {
            day: 1,
            month: 1,
            year: 2024,
            era: Era::Gregorian,
        };
        assert_eq!(d.to_iso8601(), "2024-01-01");
    }

    // ── Thai text formatting ─────────────────────────────────────────────────

    #[test]
    fn to_thai_text_basic() {
        let d = ThaiDate {
            day: 5,
            month: 7,
            year: 2567,
            era: Era::Buddhist,
        };
        assert_eq!(d.to_thai_text(), "5 กรกฎาคม พ.ศ. 2567");
    }

    #[test]
    fn to_thai_text_gregorian_converts_to_be() {
        let d = ThaiDate {
            day: 1,
            month: 1,
            year: 2024,
            era: Era::Gregorian,
        };
        assert_eq!(d.to_thai_text(), "1 มกราคม พ.ศ. 2567");
    }

    // ── format_thai_date ─────────────────────────────────────────────────────

    #[test]
    fn format_thai_date_valid() {
        assert_eq!(
            format_thai_date(2567, 7, 5),
            Some(String::from("5 กรกฎาคม พ.ศ. 2567"))
        );
    }

    #[test]
    fn format_thai_date_invalid_month() {
        assert_eq!(format_thai_date(2567, 0, 1), None);
        assert_eq!(format_thai_date(2567, 13, 1), None);
    }

    #[test]
    fn format_thai_date_invalid_day() {
        assert_eq!(format_thai_date(2567, 1, 0), None);
        assert_eq!(format_thai_date(2567, 1, 32), None);
    }

    // ── parse_thai_date: full month name ─────────────────────────────────────

    #[test]
    fn parse_full_month_name_be() {
        let d = parse_thai_date("5 กรกฎาคม 2567").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
        assert_eq!(d.era, Era::Buddhist);
    }

    #[test]
    fn parse_full_month_explicit_be_marker() {
        let d = parse_thai_date("5 กรกฎาคม พ.ศ. 2567").unwrap();
        assert_eq!(d.era, Era::Buddhist);
        assert_eq!(d.year, 2567);
    }

    #[test]
    fn parse_full_month_explicit_ce_marker() {
        let d = parse_thai_date("5 กรกฎาคม ค.ศ. 2024").unwrap();
        assert_eq!(d.era, Era::Gregorian);
        assert_eq!(d.year, 2024);
    }

    #[test]
    fn parse_wanthi_prefix() {
        let d = parse_thai_date("วันที่ 5 กรกฎาคม 2567").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
    }

    #[test]
    fn parse_wanthi_prefix_with_era() {
        let d = parse_thai_date("วันที่ 5 กรกฎาคม พ.ศ. 2567").unwrap();
        assert_eq!(d.era, Era::Buddhist);
    }

    // ── parse_thai_date: abbreviated month ──────────────────────────────────

    #[test]
    fn parse_abbreviated_month() {
        let d = parse_thai_date("5 ก.ค. 2567").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
    }

    #[test]
    fn parse_abbreviated_month_with_era() {
        let d = parse_thai_date("5 ก.ค. พ.ศ. 2567").unwrap();
        assert_eq!(d.era, Era::Buddhist);
        assert_eq!(d.month, 7);
    }

    #[test]
    fn parse_all_abbreviated_months() {
        let cases: &[(&str, u8)] = &[
            ("5 ม.ค. 2567", 1),
            ("5 ก.พ. 2567", 2),
            ("5 มี.ค. 2567", 3),
            ("5 เม.ย. 2567", 4),
            ("5 พ.ค. 2567", 5),
            ("5 มิ.ย. 2567", 6),
            ("5 ก.ค. 2567", 7),
            ("5 ส.ค. 2567", 8),
            ("5 ก.ย. 2567", 9),
            ("5 ต.ค. 2567", 10),
            ("5 พ.ย. 2567", 11),
            ("5 ธ.ค. 2567", 12),
        ];
        for &(input, expected_month) in cases {
            let d = parse_thai_date(input)
                .unwrap_or_else(|| panic!("failed to parse: {input}"));
            assert_eq!(d.month, expected_month, "month mismatch for: {input}");
        }
    }

    #[test]
    fn parse_all_full_months() {
        let cases: &[(&str, u8)] = &[
            ("1 มกราคม 2567", 1),
            ("1 กุมภาพันธ์ 2567", 2),
            ("1 มีนาคม 2567", 3),
            ("1 เมษายน 2567", 4),
            ("1 พฤษภาคม 2567", 5),
            ("1 มิถุนายน 2567", 6),
            ("1 กรกฎาคม 2567", 7),
            ("1 สิงหาคม 2567", 8),
            ("1 กันยายน 2567", 9),
            ("1 ตุลาคม 2567", 10),
            ("1 พฤศจิกายน 2567", 11),
            ("1 ธันวาคม 2567", 12),
        ];
        for &(input, expected_month) in cases {
            let d = parse_thai_date(input)
                .unwrap_or_else(|| panic!("failed to parse: {input}"));
            assert_eq!(d.month, expected_month, "month mismatch for: {input}");
        }
    }

    // ── parse_thai_date: numeric formats ────────────────────────────────────

    #[test]
    fn parse_slash_separated() {
        let d = parse_thai_date("5/7/2567").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
        assert_eq!(d.era, Era::Buddhist);
        assert_eq!(d.to_iso8601(), "2024-07-05");
    }

    #[test]
    fn parse_dash_separated() {
        let d = parse_thai_date("5-7-2567").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
    }

    #[test]
    fn parse_numeric_gregorian_year() {
        let d = parse_thai_date("1/1/2024").unwrap();
        assert_eq!(d.era, Era::Gregorian);
        assert_eq!(d.year, 2024);
    }

    #[test]
    fn parse_numeric_invalid_month() {
        assert!(parse_thai_date("5/13/2567").is_none());
        assert!(parse_thai_date("5/0/2567").is_none());
    }

    #[test]
    fn parse_numeric_invalid_day() {
        assert!(parse_thai_date("32/1/2567").is_none());
        assert!(parse_thai_date("0/1/2567").is_none());
    }

    // ── parse_thai_date: Thai digits ────────────────────────────────────────

    #[test]
    fn parse_thai_digits_abbreviated_month() {
        let d = parse_thai_date("๕ ก.ค. ๒๕๖๗").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
    }

    #[test]
    fn parse_thai_digits_full_month() {
        let d = parse_thai_date("๕ กรกฎาคม ๒๕๖๗").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
    }

    #[test]
    fn parse_thai_digits_numeric() {
        let d = parse_thai_date("๕/๗/๒๕๖๗").unwrap();
        assert_eq!(d.day, 5);
        assert_eq!(d.month, 7);
        assert_eq!(d.year, 2567);
    }

    // ── Era inference ─────────────────────────────────────────────────────────

    #[test]
    fn infer_era_buddhist() {
        assert_eq!(infer_era(2567), Era::Buddhist);
        assert_eq!(infer_era(2300), Era::Buddhist);
    }

    #[test]
    fn infer_era_gregorian() {
        assert_eq!(infer_era(2024), Era::Gregorian);
        assert_eq!(infer_era(1999), Era::Gregorian);
    }

    // ── Edge cases ────────────────────────────────────────────────────────────

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_thai_date("").is_none());
        assert!(parse_thai_date("   ").is_none());
    }

    #[test]
    fn parse_garbage_returns_none() {
        assert!(parse_thai_date("hello world").is_none());
        assert!(parse_thai_date("กินข้าว").is_none());
    }

    #[test]
    fn roundtrip_iso8601() {
        let d = parse_thai_date("5 กรกฎาคม 2567").unwrap();
        assert_eq!(d.to_iso8601(), "2024-07-05");
    }

    #[test]
    fn roundtrip_thai_text() {
        let d = parse_thai_date("5/7/2567").unwrap();
        assert_eq!(d.to_thai_text(), "5 กรกฎาคม พ.ศ. 2567");
    }

    #[test]
    fn leading_trailing_whitespace() {
        let d = parse_thai_date("  5 กรกฎาคม 2567  ").unwrap();
        assert_eq!(d.day, 5);
    }
}
