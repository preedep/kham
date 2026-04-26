//! Thai number normalization.
//!
//! Two independent normalization paths:
//!
//! 1. **Thai digit → ASCII**: converts Thai digit characters (๐–๙, U+0E50–U+0E59)
//!    to their ASCII equivalents. Non-digit characters are passed through unchanged.
//!
//! 2. **Spelled-out Thai number words → integer**: parses a full Thai cardinal number
//!    word (e.g. `หนึ่งร้อยยี่สิบสาม`) into its numeric value (`123`).
//!
//! ## Thai digit mapping
//!
//! | Thai | ASCII |
//! |------|-------|
//! | ๐    | 0     |
//! | ๑    | 1     |
//! | ๒    | 2     |
//! | ๓    | 3     |
//! | ๔    | 4     |
//! | ๕    | 5     |
//! | ๖    | 6     |
//! | ๗    | 7     |
//! | ๘    | 8     |
//! | ๙    | 9     |
//!
//! ## Thai number word grammar (cardinal)
//!
//! | Word   | Value       | Notes                                    |
//! |--------|-------------|------------------------------------------|
//! | ศูนย์  | 0           |                                          |
//! | หนึ่ง  | 1           |                                          |
//! | เอ็ด   | 1           | units position after `สิบ` only          |
//! | ยี่     | 2           | tens position only (`ยี่สิบ` = 20)       |
//! | สอง   | 2           |                                          |
//! | สาม   | 3           |                                          |
//! | สี่     | 4           |                                          |
//! | ห้า    | 5           |                                          |
//! | หก    | 6           |                                          |
//! | เจ็ด   | 7           |                                          |
//! | แปด   | 8           |                                          |
//! | เก้า   | 9           |                                          |
//! | สิบ    | ×10         | preceding digit optional (default 1)     |
//! | ร้อย   | ×100        | preceding digit optional                 |
//! | พัน    | ×1 000      | preceding digit optional                 |
//! | หมื่น  | ×10 000     | preceding digit optional                 |
//! | แสน   | ×100 000    | preceding digit optional                 |
//! | ล้าน   | ×1 000 000  | splits number into two sub-million groups|
//!
//! # Examples
//!
//! ```rust
//! use kham_core::number::{thai_digits_to_ascii, parse_thai_word};
//!
//! // Thai digit conversion
//! assert_eq!(thai_digits_to_ascii("๑๒๓"), "123");
//! assert_eq!(thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง"), "ธนาคาร100แห่ง");
//!
//! // Spelled-out number word parsing
//! assert_eq!(parse_thai_word("ยี่สิบ"), Some(20));
//! assert_eq!(parse_thai_word("หนึ่งร้อยยี่สิบสาม"), Some(123));
//! assert_eq!(parse_thai_word("สองล้านห้าแสน"), Some(2_500_000));
//! ```

use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Thai digit ↔ ASCII
// ---------------------------------------------------------------------------

/// Convert a single Thai digit character (๐–๙) to its ASCII equivalent.
///
/// Returns `None` for any character that is not a Thai digit.
///
/// # Examples
///
/// ```rust
/// use kham_core::number::thai_digit_to_ascii;
///
/// assert_eq!(thai_digit_to_ascii('๑'), Some('1'));
/// assert_eq!(thai_digit_to_ascii('ก'), None);
/// assert_eq!(thai_digit_to_ascii('5'), None); // ASCII digits pass through unchanged
/// ```
#[inline]
pub fn thai_digit_to_ascii(c: char) -> Option<char> {
    match c {
        '\u{0E50}' => Some('0'),
        '\u{0E51}' => Some('1'),
        '\u{0E52}' => Some('2'),
        '\u{0E53}' => Some('3'),
        '\u{0E54}' => Some('4'),
        '\u{0E55}' => Some('5'),
        '\u{0E56}' => Some('6'),
        '\u{0E57}' => Some('7'),
        '\u{0E58}' => Some('8'),
        '\u{0E59}' => Some('9'),
        _ => None,
    }
}

/// Convert all Thai digit characters in `text` to ASCII digits.
///
/// Characters that are not Thai digits are passed through unchanged.
/// Allocates a new [`String`] only when Thai digits are present;
/// otherwise returns a copy of the input.
///
/// # Examples
///
/// ```rust
/// use kham_core::number::thai_digits_to_ascii;
///
/// assert_eq!(thai_digits_to_ascii("๑๒๓"), "123");
/// assert_eq!(thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง"), "ธนาคาร100แห่ง");
/// assert_eq!(thai_digits_to_ascii("hello"), "hello");
/// assert_eq!(thai_digits_to_ascii(""), "");
/// // Mixed Thai and ASCII digits — only Thai digits are converted
/// assert_eq!(thai_digits_to_ascii("๑2๓"), "123");
/// ```
pub fn thai_digits_to_ascii(text: &str) -> String {
    if !text.chars().any(|c| thai_digit_to_ascii(c).is_some()) {
        return String::from(text);
    }
    text.chars()
        .map(|c| thai_digit_to_ascii(c).unwrap_or(c))
        .collect()
}

/// Return `true` if every character in `text` is a Thai digit (๐–๙).
///
/// Returns `false` for empty strings.
///
/// # Examples
///
/// ```rust
/// use kham_core::number::is_thai_digit_str;
///
/// assert!(is_thai_digit_str("๑๒๓"));
/// assert!(is_thai_digit_str("๐"));
/// assert!(!is_thai_digit_str("123"));
/// assert!(!is_thai_digit_str("๑2๓")); // mixed
/// assert!(!is_thai_digit_str(""));
/// ```
#[inline]
pub fn is_thai_digit_str(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| thai_digit_to_ascii(c).is_some())
}

// ---------------------------------------------------------------------------
// Spelled-out Thai number word → u64
// ---------------------------------------------------------------------------

/// Internal lexer token for the Thai number word parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumToken {
    /// A digit 0–9: ศูนย์ หนึ่ง สอง สาม สี่ ห้า หก เจ็ด แปด เก้า.
    Digit(u64),
    /// ยี่ — special form of 2, valid only in the tens position (ยี่สิบ = 20).
    Yi,
    /// เอ็ด — special form of 1, valid only in the units position after สิบ.
    Et,
    /// สิบ — ×10 multiplier.
    Sip,
    /// ร้อย — ×100 multiplier.
    Roi,
    /// พัน — ×1 000 multiplier.
    Pan,
    /// หมื่น — ×10 000 multiplier.
    Muen,
    /// แสน — ×100 000 multiplier.
    Saen,
    /// ล้าน — ×1 000 000 group separator.
    Lan,
}

/// Greedy longest-prefix match against the Thai number vocabulary.
///
/// Returns `(token, remaining_slice)` or `None` if no token starts at `s`.
fn next_num_token(s: &str) -> Option<(NumToken, &str)> {
    // Ordered so that longer / more specific prefixes come before shorter ones.
    // e.g. "เก้า" before any single-syllable word to avoid prefix collisions.
    const VOCAB: &[(&str, NumToken)] = &[
        ("ศูนย์", NumToken::Digit(0)),
        ("หนึ่ง", NumToken::Digit(1)),
        ("เอ็ด", NumToken::Et),
        ("ยี่", NumToken::Yi),
        ("สอง", NumToken::Digit(2)),
        ("สาม", NumToken::Digit(3)),
        ("สี่", NumToken::Digit(4)),
        ("ห้า", NumToken::Digit(5)),
        ("หก", NumToken::Digit(6)),
        ("เจ็ด", NumToken::Digit(7)),
        ("แปด", NumToken::Digit(8)),
        ("เก้า", NumToken::Digit(9)),
        ("สิบ", NumToken::Sip),
        ("ร้อย", NumToken::Roi),
        ("พัน", NumToken::Pan),
        ("หมื่น", NumToken::Muen),
        ("แสน", NumToken::Saen),
        ("ล้าน", NumToken::Lan),
    ];
    for &(word, tok) in VOCAB {
        if let Some(rest) = s.strip_prefix(word) {
            return Some((tok, rest));
        }
    }
    None
}

/// Parse a Thai cardinal number below one million (0–999 999).
///
/// Returns `None` when `s` contains unrecognised tokens or is structurally
/// invalid (e.g. two consecutive digit words without a multiplier between them).
/// An empty string returns `Some(0)` to support implied-zero sub-parts.
fn parse_below_lan(s: &str) -> Option<u64> {
    let mut s = s;
    let mut total: u64 = 0;
    let mut pending: Option<u64> = None; // digit waiting for its multiplier
    let mut had_sip = false; // ตรวจสอบ for เอ็ด-validity

    while !s.is_empty() {
        let (tok, rest) = next_num_token(s)?;
        match tok {
            NumToken::Digit(d) => {
                // Two consecutive digit words with no multiplier between them
                // are not valid Thai number spelling.
                if pending.is_some() {
                    return None;
                }
                pending = Some(d);
            }
            NumToken::Yi => {
                if pending.is_some() {
                    return None;
                }
                // ยี่ acts as digit-2 in front of สิบ (ยี่สิบ = 20).
                pending = Some(2);
            }
            NumToken::Et => {
                // เอ็ด is the units-1 form allowed only after สิบ.
                if !had_sip || pending.is_some() {
                    return None;
                }
                total = total.checked_add(1)?;
            }
            NumToken::Sip => {
                let coeff = pending.take().unwrap_or(1);
                total = total.checked_add(coeff.checked_mul(10)?)?;
                had_sip = true;
            }
            NumToken::Roi => {
                let coeff = pending.take().unwrap_or(1);
                total = total.checked_add(coeff.checked_mul(100)?)?;
            }
            NumToken::Pan => {
                let coeff = pending.take().unwrap_or(1);
                total = total.checked_add(coeff.checked_mul(1_000)?)?;
            }
            NumToken::Muen => {
                let coeff = pending.take().unwrap_or(1);
                total = total.checked_add(coeff.checked_mul(10_000)?)?;
            }
            NumToken::Saen => {
                let coeff = pending.take().unwrap_or(1);
                total = total.checked_add(coeff.checked_mul(100_000)?)?;
            }
            NumToken::Lan => {
                // ล้าน is resolved at the outer level; hitting it here is invalid.
                return None;
            }
        }
        s = rest;
    }

    // Any remaining pending digit is a standalone units value (e.g. the สาม in
    // หนึ่งร้อยสองสิบสาม).
    if let Some(d) = pending {
        total = total.checked_add(d)?;
    }

    Some(total)
}

/// Convert a `u64` to its decimal string representation without `std`.
fn u64_to_string(mut n: u64) -> String {
    if n == 0 {
        return String::from("0");
    }
    let mut digits: Vec<u8> = Vec::new();
    while n > 0 {
        digits.push(b'0' + (n % 10) as u8);
        n /= 10;
    }
    digits.reverse();
    // SAFETY: digits are ASCII '0'–'9', guaranteed valid UTF-8.
    String::from_utf8(digits).unwrap_or_default()
}

/// Parse a spelled-out Thai cardinal number word into its numeric value.
///
/// Handles the full range from 0 (`ศูนย์`) up to values bounded by `u64::MAX`.
/// Returns `None` when the input contains unrecognised characters, is empty,
/// or is structurally invalid for Thai number grammar.
///
/// ## Grammar summary
///
/// ```text
/// number  ::= sub_lan "ล้าน" sub_lan
///           | sub_lan "ล้าน"
///           | sub_lan
///
/// sub_lan ::= [digit] "แสน" sub_แสน | sub_แสน
///           | [digit] "หมื่น" sub_หมื่น | sub_หมื่น
///           | [digit] "พัน"   sub_พัน   | sub_พัน
///           | [digit] "ร้อย"  sub_ร้อย  | sub_ร้อย
///           | ["ยี่" | digit] "สิบ" unit | "สิบ" unit
///           | unit
///
/// unit    ::= digit | "เอ็ด" | ε
/// digit   ::= "ศูนย์" | "หนึ่ง" | "สอง" | … | "เก้า"
/// ```
///
/// The `ยี่` form of 2 is valid only in `ยี่สิบ` (20, 21, …, 29).
/// The `เอ็ด` form of 1 is valid only as the units digit after `สิบ`
/// (11, 21, 31, …).
///
/// # Examples
///
/// ```rust
/// use kham_core::number::parse_thai_word;
///
/// // single digits
/// assert_eq!(parse_thai_word("ศูนย์"), Some(0));
/// assert_eq!(parse_thai_word("หนึ่ง"), Some(1));
/// assert_eq!(parse_thai_word("เก้า"), Some(9));
///
/// // tens — implied-1 prefix and special forms
/// assert_eq!(parse_thai_word("สิบ"), Some(10));
/// assert_eq!(parse_thai_word("สิบเอ็ด"), Some(11));
/// assert_eq!(parse_thai_word("ยี่สิบ"), Some(20));
/// assert_eq!(parse_thai_word("ยี่สิบเอ็ด"), Some(21));
/// assert_eq!(parse_thai_word("สามสิบสี่"), Some(34));
///
/// // hundreds
/// assert_eq!(parse_thai_word("ร้อย"), Some(100));
/// assert_eq!(parse_thai_word("หนึ่งร้อย"), Some(100));
/// assert_eq!(parse_thai_word("หนึ่งร้อยยี่สิบสาม"), Some(123));
///
/// // thousands
/// assert_eq!(parse_thai_word("หนึ่งพัน"), Some(1_000));
/// assert_eq!(parse_thai_word("สองพันห้าร้อย"), Some(2_500));
///
/// // ten-thousands / hundred-thousands
/// assert_eq!(parse_thai_word("หนึ่งหมื่น"), Some(10_000));
/// assert_eq!(parse_thai_word("หนึ่งแสน"), Some(100_000));
///
/// // millions — coefficient itself can be a sub-million number
/// assert_eq!(parse_thai_word("หนึ่งล้าน"), Some(1_000_000));
/// assert_eq!(parse_thai_word("ล้าน"), Some(1_000_000));
/// assert_eq!(parse_thai_word("สองล้านห้าแสน"), Some(2_500_000));
/// assert_eq!(parse_thai_word("สิบล้าน"), Some(10_000_000));
/// assert_eq!(parse_thai_word("หนึ่งร้อยล้าน"), Some(100_000_000));
///
/// // not a number → None
/// assert_eq!(parse_thai_word("กินข้าว"), None);
/// assert_eq!(parse_thai_word(""), None);
/// ```
pub fn parse_thai_word(text: &str) -> Option<u64> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(lan_pos) = s.find("ล้าน") {
        let prefix = &s[..lan_pos];
        let suffix = &s[lan_pos + "ล้าน".len()..];

        // The millions coefficient is a sub-million number; bare "ล้าน" implies 1.
        let millions: u64 = if prefix.is_empty() {
            1
        } else {
            parse_below_lan(prefix)?
        };
        let remainder: u64 = if suffix.is_empty() {
            0
        } else {
            parse_below_lan(suffix)?
        };

        millions.checked_mul(1_000_000)?.checked_add(remainder)
    } else {
        let result = parse_below_lan(s)?;
        // parse_below_lan returns Some(0) for empty string, but we guard that
        // case at the top of this function, so a Some(0) here means ศูนย์.
        Some(result)
    }
}

/// Return the decimal string representation of a Thai number word, or `None`
/// if the input is not a recognised Thai number word.
///
/// This is a convenience wrapper over [`parse_thai_word`] that formats the
/// result as a string suitable for use as an FTS synonym.
///
/// # Examples
///
/// ```rust
/// use kham_core::number::thai_word_to_decimal;
///
/// assert_eq!(thai_word_to_decimal("ยี่สิบ"), Some(String::from("20")));
/// assert_eq!(thai_word_to_decimal("หนึ่งร้อยยี่สิบสาม"), Some(String::from("123")));
/// assert_eq!(thai_word_to_decimal("กิน"), None);
/// ```
pub fn thai_word_to_decimal(text: &str) -> Option<String> {
    parse_thai_word(text).map(u64_to_string)
}

// ---------------------------------------------------------------------------
// Number → Thai word (generator, inverse of parse_thai_word)
// ---------------------------------------------------------------------------

/// Map a single digit 1–9 to its Thai word.
///
/// Returns an empty string for 0 (caller handles zero as ศูนย์ or omits it).
#[inline]
fn digit_word(d: u64) -> &'static str {
    match d {
        1 => "หนึ่ง",
        2 => "สอง",
        3 => "สาม",
        4 => "สี่",
        5 => "ห้า",
        6 => "หก",
        7 => "เจ็ด",
        8 => "แปด",
        9 => "เก้า",
        _ => "",
    }
}

/// Append the Thai word representation of `n` (1–999 999) to `out`.
fn write_below_lan(mut n: u64, out: &mut String) {
    if n >= 100_000 {
        out.push_str(digit_word(n / 100_000));
        out.push_str("แสน");
        n %= 100_000;
    }
    if n >= 10_000 {
        out.push_str(digit_word(n / 10_000));
        out.push_str("หมื่น");
        n %= 10_000;
    }
    if n >= 1_000 {
        out.push_str(digit_word(n / 1_000));
        out.push_str("พัน");
        n %= 1_000;
    }
    if n >= 100 {
        out.push_str(digit_word(n / 100));
        out.push_str("ร้อย");
        n %= 100;
    }
    if n >= 10 {
        let tens = n / 10;
        let units = n % 10;
        match tens {
            1 => out.push_str("สิบ"),  // implied-1: สิบ not หนึ่งสิบ
            2 => out.push_str("ยี่สิบ"), // special form for 20s
            _ => {
                out.push_str(digit_word(tens));
                out.push_str("สิบ");
            }
        }
        match units {
            0 => {}
            1 => out.push_str("เอ็ด"), // เอ็ด only after สิบ
            _ => out.push_str(digit_word(units)),
        }
    } else if n > 0 {
        out.push_str(digit_word(n)); // plain units 1–9, no สิบ context → หนึ่ง not เอ็ด
    }
}

/// Append the full Thai word representation of any `n > 0` to `out`.
fn write_thai_word(n: u64, out: &mut String) {
    if n >= 1_000_000 {
        write_thai_word(n / 1_000_000, out);
        out.push_str("ล้าน");
        let rem = n % 1_000_000;
        if rem > 0 {
            write_below_lan(rem, out);
        }
    } else {
        write_below_lan(n, out);
    }
}

/// Convert a `u64` to a spelled-out Thai cardinal number word.
///
/// This is the inverse of [`parse_thai_word`]: for any value `n`,
/// `parse_thai_word(u64_to_thai_word(n)) == Some(n)`.
///
/// - Zero is rendered as `ศูนย์`.
/// - The `ยี่` form is used for 20, 21, …, 29 and their multiples.
/// - The `เอ็ด` form is used for units-1 when a tens word (`สิบ`) precedes it.
/// - Higher multipliers use explicit digit prefixes (`หนึ่งร้อย` = 100, etc.).
///
/// # Examples
///
/// ```rust
/// use kham_core::number::u64_to_thai_word;
///
/// assert_eq!(u64_to_thai_word(0),   "ศูนย์");
/// assert_eq!(u64_to_thai_word(10),  "สิบ");
/// assert_eq!(u64_to_thai_word(11),  "สิบเอ็ด");
/// assert_eq!(u64_to_thai_word(20),  "ยี่สิบ");
/// assert_eq!(u64_to_thai_word(21),  "ยี่สิบเอ็ด");
/// assert_eq!(u64_to_thai_word(100), "หนึ่งร้อย");
/// assert_eq!(u64_to_thai_word(123), "หนึ่งร้อยยี่สิบสาม");
/// assert_eq!(u64_to_thai_word(1_000_000), "หนึ่งล้าน");
/// assert_eq!(u64_to_thai_word(10_000_000), "สิบล้าน");
/// ```
pub fn u64_to_thai_word(n: u64) -> String {
    if n == 0 {
        return String::from("ศูนย์");
    }
    let mut out = String::new();
    write_thai_word(n, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Thai Baht — parse and generate
// ---------------------------------------------------------------------------

/// A monetary amount in Thai Baht.
///
/// `satang` is the sub-unit (1 baht = 100 satang). Valid range is 0–99.
///
/// # Examples
///
/// ```rust
/// use kham_core::number::{BahtAmount, parse_thai_baht, to_thai_baht_text};
///
/// let amt = parse_thai_baht("หนึ่งร้อยยี่สิบสามบาทห้าสิบสตางค์").unwrap();
/// assert_eq!(amt.baht, 123);
/// assert_eq!(amt.satang, 50);
///
/// assert_eq!(to_thai_baht_text(123, 50), "หนึ่งร้อยยี่สิบสามบาทห้าสิบสตางค์");
/// assert_eq!(to_thai_baht_text(100, 0),  "หนึ่งร้อยบาทถ้วน");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BahtAmount {
    /// Whole baht.
    pub baht: u64,
    /// Satang (0–99). 100 satang = 1 baht.
    pub satang: u8,
}

/// Parse a Thai Baht currency string into a [`BahtAmount`].
///
/// Accepted forms:
///
/// | Input                                      | baht | satang |
/// |--------------------------------------------|------|--------|
/// | `หนึ่งร้อยบาทถ้วน`                          | 100  | 0      |
/// | `หนึ่งร้อยบาท`                               | 100  | 0      |
/// | `ห้าบาทยี่สิบห้าสตางค์`                      | 5    | 25     |
/// | `หนึ่งล้านบาทถ้วน`                           | 1 000 000 | 0 |
/// | `ศูนย์บาทห้าสิบสตางค์`                       | 0    | 50     |
///
/// Returns `None` when:
/// - The string contains no `บาท`.
/// - The baht part is not a recognised Thai number word.
/// - The satang part is present but not a recognised Thai number word.
/// - The satang value exceeds 99.
///
/// # Examples
///
/// ```rust
/// use kham_core::number::{parse_thai_baht, BahtAmount};
///
/// assert_eq!(
///     parse_thai_baht("หนึ่งร้อยยี่สิบสามบาทถ้วน"),
///     Some(BahtAmount { baht: 123, satang: 0 })
/// );
/// assert_eq!(
///     parse_thai_baht("ห้าบาทยี่สิบห้าสตางค์"),
///     Some(BahtAmount { baht: 5, satang: 25 })
/// );
/// assert_eq!(parse_thai_baht("กินข้าว"), None);
/// assert_eq!(parse_thai_baht(""), None);
/// ```
pub fn parse_thai_baht(text: &str) -> Option<BahtAmount> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }

    let (baht_part, after_baht) = s.split_once("บาท")?;

    let baht = parse_thai_word(baht_part.trim())?;

    let satang_str = after_baht.trim();
    let satang: u8 = if satang_str.is_empty() || satang_str == "ถ้วน" {
        0
    } else if let Some(san_word) = satang_str.strip_suffix("สตางค์") {
        let val = parse_thai_word(san_word.trim())?;
        if val > 99 {
            return None;
        }
        val as u8
    } else {
        return None;
    };

    Some(BahtAmount { baht, satang })
}

/// Render a baht + satang amount as Thai currency text.
///
/// - When `satang == 0` the suffix `ถ้วน` (exact, no satang) is appended.
/// - When `satang > 0` the satang amount is spelled out followed by `สตางค์`.
///
/// The satang parameter should be in the range 0–99 (1 baht = 100 satang).
/// Values above 99 are accepted and rendered as-is but are semantically odd.
///
/// # Examples
///
/// ```rust
/// use kham_core::number::to_thai_baht_text;
///
/// assert_eq!(to_thai_baht_text(0,   0),  "ศูนย์บาทถ้วน");
/// assert_eq!(to_thai_baht_text(1,   0),  "หนึ่งบาทถ้วน");
/// assert_eq!(to_thai_baht_text(100, 0),  "หนึ่งร้อยบาทถ้วน");
/// assert_eq!(to_thai_baht_text(21,  50), "ยี่สิบเอ็ดบาทห้าสิบสตางค์");
/// assert_eq!(to_thai_baht_text(1_000_000, 0), "หนึ่งล้านบาทถ้วน");
/// assert_eq!(to_thai_baht_text(0,   25), "ศูนย์บาทยี่สิบห้าสตางค์");
/// ```
pub fn to_thai_baht_text(baht: u64, satang: u8) -> String {
    let mut out = u64_to_thai_word(baht);
    out.push_str("บาท");
    if satang == 0 {
        out.push_str("ถ้วน");
    } else {
        out.push_str(&u64_to_thai_word(satang as u64));
        out.push_str("สตางค์");
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── thai_digit_to_ascii ───────────────────────────────────────────────────

    #[test]
    fn thai_digits_map_correctly() {
        let pairs = [
            ('๐', '0'),
            ('๑', '1'),
            ('๒', '2'),
            ('๓', '3'),
            ('๔', '4'),
            ('๕', '5'),
            ('๖', '6'),
            ('๗', '7'),
            ('๘', '8'),
            ('๙', '9'),
        ];
        for (thai, ascii) in pairs {
            assert_eq!(thai_digit_to_ascii(thai), Some(ascii), "failed for {thai}");
        }
    }

    #[test]
    fn non_digit_returns_none() {
        assert_eq!(thai_digit_to_ascii('ก'), None);
        assert_eq!(thai_digit_to_ascii('5'), None);
        assert_eq!(thai_digit_to_ascii(' '), None);
    }

    // ── thai_digits_to_ascii ──────────────────────────────────────────────────

    #[test]
    fn converts_all_thai_digits() {
        assert_eq!(thai_digits_to_ascii("๐๑๒๓๔๕๖๗๘๙"), "0123456789");
    }

    #[test]
    fn passthrough_ascii_only() {
        assert_eq!(thai_digits_to_ascii("hello 123"), "hello 123");
    }

    #[test]
    fn empty_string_passthrough() {
        assert_eq!(thai_digits_to_ascii(""), "");
    }

    #[test]
    fn mixed_thai_digit_in_sentence() {
        assert_eq!(thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง"), "ธนาคาร100แห่ง");
    }

    #[test]
    fn mixed_thai_and_ascii_digits() {
        assert_eq!(thai_digits_to_ascii("๑2๓"), "123");
    }

    #[test]
    fn no_allocation_when_no_thai_digits() {
        // We just verify correctness; allocation behaviour is an impl detail.
        let result = thai_digits_to_ascii("no thai digits here");
        assert_eq!(result, "no thai digits here");
    }

    // ── is_thai_digit_str ─────────────────────────────────────────────────────

    #[test]
    fn all_thai_digits() {
        assert!(is_thai_digit_str("๑๒๓"));
        assert!(is_thai_digit_str("๐"));
    }

    #[test]
    fn mixed_is_false() {
        assert!(!is_thai_digit_str("๑2๓"));
        assert!(!is_thai_digit_str("๑ก"));
    }

    #[test]
    fn ascii_only_is_false() {
        assert!(!is_thai_digit_str("123"));
    }

    #[test]
    fn empty_is_false() {
        assert!(!is_thai_digit_str(""));
    }

    // ── u64_to_string ─────────────────────────────────────────────────────────

    #[test]
    fn zero_formats_correctly() {
        assert_eq!(u64_to_string(0), "0");
    }

    #[test]
    fn small_number_formats_correctly() {
        assert_eq!(u64_to_string(42), "42");
    }

    #[test]
    fn large_number_formats_correctly() {
        assert_eq!(u64_to_string(1_000_000), "1000000");
    }

    // ── parse_thai_word — single digits ──────────────────────────────────────

    #[test]
    fn zero() {
        assert_eq!(parse_thai_word("ศูนย์"), Some(0));
    }

    #[test]
    fn one_to_nine() {
        let cases = [
            ("หนึ่ง", 1u64),
            ("สอง", 2),
            ("สาม", 3),
            ("สี่", 4),
            ("ห้า", 5),
            ("หก", 6),
            ("เจ็ด", 7),
            ("แปด", 8),
            ("เก้า", 9),
        ];
        for (word, expected) in cases {
            assert_eq!(parse_thai_word(word), Some(expected), "failed for {word}");
        }
    }

    // ── parse_thai_word — tens ────────────────────────────────────────────────

    #[test]
    fn ten_implied_one() {
        assert_eq!(parse_thai_word("สิบ"), Some(10));
    }

    #[test]
    fn eleven_uses_et() {
        assert_eq!(parse_thai_word("สิบเอ็ด"), Some(11));
    }

    #[test]
    fn twelve_to_nineteen() {
        let cases = [
            ("สิบสอง", 12u64),
            ("สิบสาม", 13),
            ("สิบสี่", 14),
            ("สิบห้า", 15),
            ("สิบหก", 16),
            ("สิบเจ็ด", 17),
            ("สิบแปด", 18),
            ("สิบเก้า", 19),
        ];
        for (word, expected) in cases {
            assert_eq!(parse_thai_word(word), Some(expected), "failed for {word}");
        }
    }

    #[test]
    fn twenty_uses_yi() {
        assert_eq!(parse_thai_word("ยี่สิบ"), Some(20));
    }

    #[test]
    fn twenty_one_yi_et() {
        assert_eq!(parse_thai_word("ยี่สิบเอ็ด"), Some(21));
    }

    #[test]
    fn thirty_four() {
        assert_eq!(parse_thai_word("สามสิบสี่"), Some(34));
    }

    #[test]
    fn ninety_nine() {
        assert_eq!(parse_thai_word("เก้าสิบเก้า"), Some(99));
    }

    // ── parse_thai_word — hundreds ────────────────────────────────────────────

    #[test]
    fn hundred_implied_one() {
        assert_eq!(parse_thai_word("ร้อย"), Some(100));
    }

    #[test]
    fn one_hundred_explicit() {
        assert_eq!(parse_thai_word("หนึ่งร้อย"), Some(100));
    }

    #[test]
    fn one_hundred_twenty_three() {
        assert_eq!(parse_thai_word("หนึ่งร้อยยี่สิบสาม"), Some(123));
    }

    #[test]
    fn two_hundred() {
        assert_eq!(parse_thai_word("สองร้อย"), Some(200));
    }

    #[test]
    fn nine_hundred_ninety_nine() {
        assert_eq!(parse_thai_word("เก้าร้อยเก้าสิบเก้า"), Some(999));
    }

    // ── parse_thai_word — thousands ───────────────────────────────────────────

    #[test]
    fn one_thousand() {
        assert_eq!(parse_thai_word("หนึ่งพัน"), Some(1_000));
        assert_eq!(parse_thai_word("พัน"), Some(1_000));
    }

    #[test]
    fn two_thousand_five_hundred() {
        assert_eq!(parse_thai_word("สองพันห้าร้อย"), Some(2_500));
    }

    #[test]
    fn ten_thousand() {
        assert_eq!(parse_thai_word("หนึ่งหมื่น"), Some(10_000));
        assert_eq!(parse_thai_word("หมื่น"), Some(10_000));
    }

    #[test]
    fn hundred_thousand() {
        assert_eq!(parse_thai_word("หนึ่งแสน"), Some(100_000));
        assert_eq!(parse_thai_word("แสน"), Some(100_000));
    }

    // ── parse_thai_word — millions ────────────────────────────────────────────

    #[test]
    fn one_million_explicit() {
        assert_eq!(parse_thai_word("หนึ่งล้าน"), Some(1_000_000));
    }

    #[test]
    fn one_million_implied() {
        assert_eq!(parse_thai_word("ล้าน"), Some(1_000_000));
    }

    #[test]
    fn ten_million() {
        assert_eq!(parse_thai_word("สิบล้าน"), Some(10_000_000));
    }

    #[test]
    fn hundred_million() {
        assert_eq!(parse_thai_word("หนึ่งร้อยล้าน"), Some(100_000_000));
    }

    #[test]
    fn two_million_five_hundred_thousand() {
        assert_eq!(parse_thai_word("สองล้านห้าแสน"), Some(2_500_000));
    }

    #[test]
    fn complex_seven_digit() {
        // 3,456,789
        assert_eq!(
            parse_thai_word("สามล้านสี่แสนห้าหมื่นหกพันเจ็ดร้อยแปดสิบเก้า"),
            Some(3_456_789)
        );
    }

    // ── parse_thai_word — invalid / None ─────────────────────────────────────

    #[test]
    fn empty_returns_none() {
        assert_eq!(parse_thai_word(""), None);
    }

    #[test]
    fn whitespace_only_returns_none() {
        assert_eq!(parse_thai_word("   "), None);
    }

    #[test]
    fn non_number_word_returns_none() {
        assert_eq!(parse_thai_word("กินข้าว"), None);
        assert_eq!(parse_thai_word("ประเทศไทย"), None);
    }

    #[test]
    fn et_without_sip_is_invalid() {
        assert_eq!(parse_thai_word("เอ็ด"), None);
        assert_eq!(parse_thai_word("ร้อยเอ็ด"), None);
    }

    #[test]
    fn consecutive_digits_invalid() {
        // หนึ่งสอง = two digit words with no multiplier is invalid
        assert_eq!(parse_thai_word("หนึ่งสอง"), None);
    }

    // ── thai_word_to_decimal ──────────────────────────────────────────────────

    #[test]
    fn word_to_decimal_converts() {
        assert_eq!(thai_word_to_decimal("ยี่สิบ"), Some(String::from("20")));
        assert_eq!(
            thai_word_to_decimal("หนึ่งร้อยยี่สิบสาม"),
            Some(String::from("123"))
        );
    }

    #[test]
    fn word_to_decimal_none_for_non_number() {
        assert_eq!(thai_word_to_decimal("กิน"), None);
    }

    // ── trim handling ─────────────────────────────────────────────────────────

    #[test]
    fn leading_trailing_whitespace_trimmed() {
        assert_eq!(parse_thai_word("  สิบ  "), Some(10));
    }

    // ── u64_to_thai_word ──────────────────────────────────────────────────────

    #[test]
    fn zero_word() {
        assert_eq!(u64_to_thai_word(0), "ศูนย์");
    }

    #[test]
    fn single_digits_word() {
        let cases = [
            (1u64, "หนึ่ง"),
            (2, "สอง"),
            (3, "สาม"),
            (4, "สี่"),
            (5, "ห้า"),
            (6, "หก"),
            (7, "เจ็ด"),
            (8, "แปด"),
            (9, "เก้า"),
        ];
        for (n, word) in cases {
            assert_eq!(u64_to_thai_word(n), word, "failed for {n}");
        }
    }

    #[test]
    fn ten_implied_one_word() {
        assert_eq!(u64_to_thai_word(10), "สิบ");
    }

    #[test]
    fn eleven_et_form() {
        assert_eq!(u64_to_thai_word(11), "สิบเอ็ด");
    }

    #[test]
    fn twelve_to_nineteen_word() {
        let cases = [(12u64, "สิบสอง"), (15, "สิบห้า"), (19, "สิบเก้า")];
        for (n, word) in cases {
            assert_eq!(u64_to_thai_word(n), word);
        }
    }

    #[test]
    fn twenty_yi_form() {
        assert_eq!(u64_to_thai_word(20), "ยี่สิบ");
    }

    #[test]
    fn twenty_one_yi_et_word() {
        assert_eq!(u64_to_thai_word(21), "ยี่สิบเอ็ด");
    }

    #[test]
    fn thirty_four_word() {
        assert_eq!(u64_to_thai_word(34), "สามสิบสี่");
    }

    #[test]
    fn one_hundred_word() {
        assert_eq!(u64_to_thai_word(100), "หนึ่งร้อย");
    }

    #[test]
    fn one_hundred_twenty_three_word() {
        assert_eq!(u64_to_thai_word(123), "หนึ่งร้อยยี่สิบสาม");
    }

    #[test]
    fn one_hundred_one_no_et() {
        // เอ็ด only after สิบ — 101 has no สิบ so units=1 → หนึ่ง
        assert_eq!(u64_to_thai_word(101), "หนึ่งร้อยหนึ่ง");
    }

    #[test]
    fn one_hundred_eleven_et() {
        assert_eq!(u64_to_thai_word(111), "หนึ่งร้อยสิบเอ็ด");
    }

    #[test]
    fn one_thousand_word() {
        assert_eq!(u64_to_thai_word(1_000), "หนึ่งพัน");
    }

    #[test]
    fn ten_thousand_word() {
        assert_eq!(u64_to_thai_word(10_000), "หนึ่งหมื่น");
    }

    #[test]
    fn hundred_thousand_word() {
        assert_eq!(u64_to_thai_word(100_000), "หนึ่งแสน");
    }

    #[test]
    fn one_million_word() {
        assert_eq!(u64_to_thai_word(1_000_000), "หนึ่งล้าน");
    }

    #[test]
    fn ten_million_word() {
        assert_eq!(u64_to_thai_word(10_000_000), "สิบล้าน");
    }

    #[test]
    fn complex_seven_digit_word() {
        assert_eq!(
            u64_to_thai_word(3_456_789),
            "สามล้านสี่แสนห้าหมื่นหกพันเจ็ดร้อยแปดสิบเก้า"
        );
    }

    // ── large numbers (> u32::MAX) ────────────────────────────────────────────

    #[test]
    fn ten_billion_word() {
        // 10,000,000,000 — previously overflowed u32 in WASM binding
        assert_eq!(u64_to_thai_word(10_000_000_000), "หนึ่งหมื่นล้าน");
    }

    #[test]
    fn hundred_billion_word() {
        assert_eq!(u64_to_thai_word(100_000_000_000), "หนึ่งแสนล้าน");
    }

    #[test]
    fn baht_text_ten_billion() {
        assert_eq!(to_thai_baht_text(10_000_000_000, 0), "หนึ่งหมื่นล้านบาทถ้วน");
    }

    // ── roundtrip parse_thai_word ↔ u64_to_thai_word ─────────────────────────

    #[test]
    fn roundtrip_parse_then_generate() {
        let cases = [
            0u64, 1, 9, 10, 11, 20, 21, 99, 100, 101, 111, 999, 1_000, 10_000, 100_000, 1_000_000,
            10_000_000, 3_456_789, 10_000_000_000, 100_000_000_000,
        ];
        for n in cases {
            let word = u64_to_thai_word(n);
            let parsed = parse_thai_word(&word);
            assert_eq!(parsed, Some(n), "roundtrip failed for {n}: word={word:?}");
        }
    }

    // ── parse_thai_baht ───────────────────────────────────────────────────────

    #[test]
    fn baht_exact_no_satang() {
        assert_eq!(
            parse_thai_baht("หนึ่งร้อยยี่สิบสามบาทถ้วน"),
            Some(BahtAmount {
                baht: 123,
                satang: 0
            })
        );
    }

    #[test]
    fn baht_with_satang() {
        assert_eq!(
            parse_thai_baht("ห้าบาทยี่สิบห้าสตางค์"),
            Some(BahtAmount {
                baht: 5,
                satang: 25
            })
        );
    }

    #[test]
    fn baht_no_suffix_implies_zero_satang() {
        assert_eq!(
            parse_thai_baht("หนึ่งร้อยบาท"),
            Some(BahtAmount {
                baht: 100,
                satang: 0
            })
        );
    }

    #[test]
    fn baht_zero_baht_with_satang() {
        assert_eq!(
            parse_thai_baht("ศูนย์บาทห้าสิบสตางค์"),
            Some(BahtAmount {
                baht: 0,
                satang: 50
            })
        );
    }

    #[test]
    fn baht_million() {
        assert_eq!(
            parse_thai_baht("หนึ่งล้านบาทถ้วน"),
            Some(BahtAmount {
                baht: 1_000_000,
                satang: 0
            })
        );
    }

    #[test]
    fn baht_satang_eleven() {
        // สิบเอ็ด satang = 11
        assert_eq!(
            parse_thai_baht("สองบาทสิบเอ็ดสตางค์"),
            Some(BahtAmount {
                baht: 2,
                satang: 11
            })
        );
    }

    #[test]
    fn baht_satang_fifty() {
        assert_eq!(
            parse_thai_baht("หนึ่งร้อยบาทห้าสิบสตางค์"),
            Some(BahtAmount {
                baht: 100,
                satang: 50
            })
        );
    }

    #[test]
    fn baht_satang_above_99_is_none() {
        // หนึ่งร้อย = 100 which is > 99 satang
        assert_eq!(parse_thai_baht("หนึ่งบาทหนึ่งร้อยสตางค์"), None);
    }

    #[test]
    fn baht_no_baht_marker_is_none() {
        assert_eq!(parse_thai_baht("หนึ่งร้อยยี่สิบสาม"), None);
    }

    #[test]
    fn baht_non_number_is_none() {
        assert_eq!(parse_thai_baht("กินข้าวบาทถ้วน"), None);
    }

    #[test]
    fn baht_empty_is_none() {
        assert_eq!(parse_thai_baht(""), None);
    }

    #[test]
    fn baht_unrecognised_satang_suffix_is_none() {
        assert_eq!(parse_thai_baht("หนึ่งบาทมาก"), None);
    }

    // ── to_thai_baht_text ─────────────────────────────────────────────────────

    #[test]
    fn baht_text_zero_exact() {
        assert_eq!(to_thai_baht_text(0, 0), "ศูนย์บาทถ้วน");
    }

    #[test]
    fn baht_text_one_exact() {
        assert_eq!(to_thai_baht_text(1, 0), "หนึ่งบาทถ้วน");
    }

    #[test]
    fn baht_text_hundred_exact() {
        assert_eq!(to_thai_baht_text(100, 0), "หนึ่งร้อยบาทถ้วน");
    }

    #[test]
    fn baht_text_with_satang() {
        assert_eq!(to_thai_baht_text(21, 50), "ยี่สิบเอ็ดบาทห้าสิบสตางค์");
    }

    #[test]
    fn baht_text_million_exact() {
        assert_eq!(to_thai_baht_text(1_000_000, 0), "หนึ่งล้านบาทถ้วน");
    }

    #[test]
    fn baht_text_zero_baht_with_satang() {
        assert_eq!(to_thai_baht_text(0, 25), "ศูนย์บาทยี่สิบห้าสตางค์");
    }

    #[test]
    fn baht_text_satang_eleven() {
        assert_eq!(to_thai_baht_text(2, 11), "สองบาทสิบเอ็ดสตางค์");
    }

    // ── roundtrip parse_thai_baht ↔ to_thai_baht_text ────────────────────────

    #[test]
    fn baht_roundtrip() {
        let cases = [
            (0u64, 0u8),
            (1, 0),
            (100, 0),
            (123, 50),
            (5, 25),
            (1_000_000, 0),
            (21, 11),
            (0, 99),
        ];
        for (baht, satang) in cases {
            let text = to_thai_baht_text(baht, satang);
            let parsed = parse_thai_baht(&text);
            assert_eq!(
                parsed,
                Some(BahtAmount { baht, satang }),
                "roundtrip failed for ({baht}, {satang}): text={text:?}"
            );
        }
    }
}
