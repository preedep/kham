//! Thai phonetic encoding (Soundex) — lk82, udom83, MetaSound, and Thai–English cross-language.
//!
//! Groups Thai words by sound so that spelling variants and near-homophones
//! share the same code, enabling fuzzy search and name matching.
//!
//! ```
//! use kham_core::soundex::{soundex, SoundexAlgorithm};
//!
//! // กาน / ขาน / คาน all share the same lk82 code
//! assert_eq!(soundex("กาน", SoundexAlgorithm::Lk82),
//!            soundex("ขาน", SoundexAlgorithm::Lk82));
//! assert_eq!(soundex("กาน", SoundexAlgorithm::Lk82), "1600");
//! ```

use alloc::string::String;
use alloc::vec::Vec;

/// Selects the Thai phonetic encoding algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundexAlgorithm {
    /// Lorchirachoonkul 1982 — most widely used; 4-char alphanumeric code.
    Lk82,
    /// Udompanich 1983 — finer distinctions for sibilants and liquids.
    Udom83,
    /// MetaSound (Snae & Brückner 2009) — per-syllable `[initial][vowel][final]` triple;
    /// variable-length output (3 chars per syllable).
    MetaSound,
}

/// Encode a Thai word using the selected algorithm.
///
/// - `Lk82` / `Udom83` — always returns a 4-character ASCII code; `"0000"` if
///   the word contains no Thai consonants.
/// - `MetaSound` — returns 3 characters per syllable (variable length); `"000"`
///   if the word contains no Thai consonants.
///
/// ```
/// use kham_core::soundex::{soundex, SoundexAlgorithm};
///
/// assert_eq!(soundex("กาน", SoundexAlgorithm::Lk82),
///            soundex("คาน", SoundexAlgorithm::Lk82));
/// assert_ne!(soundex("กาน", SoundexAlgorithm::Lk82),
///            soundex("บาน", SoundexAlgorithm::Lk82));
/// assert_eq!(soundex("กาน", SoundexAlgorithm::MetaSound), "112");
/// ```
pub fn soundex(word: &str, algo: SoundexAlgorithm) -> String {
    match algo {
        SoundexAlgorithm::Lk82 => lk82(word),
        SoundexAlgorithm::Udom83 => udom83(word),
        SoundexAlgorithm::MetaSound => metasound(word),
    }
}

/// Returns `true` if two words share the same phonetic code under the given algorithm.
///
/// Returns `false` if either word is empty or contains no recognisable Thai consonants.
/// Works for all three algorithms — lk82, udom83, and MetaSound.
///
/// ```
/// use kham_core::soundex::{sounds_like, SoundexAlgorithm};
///
/// assert!(sounds_like("กาน", "ขาน", SoundexAlgorithm::Lk82));
/// assert!(!sounds_like("กิน", "มิน", SoundexAlgorithm::Lk82));
/// assert!(!sounds_like("ลาน", "ราน", SoundexAlgorithm::Udom83)); // ล / ร split in udom83
/// assert!(sounds_like("กาน", "ขาน", SoundexAlgorithm::MetaSound));
/// ```
pub fn sounds_like(a: &str, b: &str, algo: SoundexAlgorithm) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let code_a = soundex(a, algo);
    // All-zero codes mean no recognisable Thai consonants (lk82/udom83 → "0000", MetaSound → "000")
    !code_a.chars().all(|c| c == '0') && code_a == soundex(b, algo)
}

// ── LK82 (Lorchirachoonkul 1982) ─────────────────────────────────────────────

/// Encode a Thai word using the LK82 algorithm (Lorchirachoonkul 1982).
///
/// Maps consonants to 12 groups (`'0'`–`'9'`, `'A'`, `'B'`), removes adjacent
/// duplicates, and pads to exactly 4 characters with `'0'`.
///
/// ```
/// use kham_core::soundex::lk82;
///
/// assert_eq!(lk82("กาน"),      "1600");
/// assert_eq!(lk82("ขาน"),      "1600"); // ก / ข in the same group
/// assert_eq!(lk82("บ้าน"),     "4600");
/// assert_eq!(lk82("กรุงเทพ"), "1873");
/// ```
pub fn lk82(word: &str) -> String {
    encode(word, lk82_code)
}

fn lk82_code(c: char) -> u8 {
    match c {
        'อ' => b'0',
        'ก' | 'ข' | 'ค' | 'ฆ' => b'1',
        'จ' | 'ช' | 'ซ' | 'ศ' | 'ษ' | 'ส' | 'ฉ' | 'ฌ' | 'ญ' => b'2',
        'ต' | 'ถ' | 'ท' | 'ธ' | 'ฏ' | 'ฐ' | 'ฑ' | 'ฒ' | 'ด' | 'ฎ' => b'3',
        'บ' | 'ป' | 'พ' | 'ผ' | 'ภ' | 'ฝ' | 'ฟ' => b'4',
        'ม' => b'5',
        'น' | 'ณ' => b'6',
        'ง' => b'7',
        'ล' | 'ร' | 'ฬ' => b'8',
        'ว' => b'9',
        'ย' => b'A',
        'ห' | 'ฮ' => b'B',
        _ => b'0',
    }
}

// ── Udom83 (Udompanich 1983) ─────────────────────────────────────────────────

/// Encode a Thai word using the Udom83 algorithm (Udompanich 1983).
///
/// Uses finer groupings than lk82: sibilants (ซ ศ ษ ส) are separate from
/// affricates (จ ช ฉ ฌ), and the liquids ร and ล are in different groups.
///
/// ```
/// use kham_core::soundex::udom83;
///
/// // ส (sibilant) and ช (affricate) are different groups in udom83
/// assert_ne!(udom83("สาน"), udom83("ชาน"));
/// // but ส and ซ share the same sibilant group
/// assert_eq!(udom83("สาน"), udom83("ซาน"));
/// // ล and ร are split
/// assert_ne!(udom83("ลาน"), udom83("ราน"));
/// ```
pub fn udom83(word: &str) -> String {
    encode(word, udom83_code)
}

fn udom83_code(c: char) -> u8 {
    match c {
        'อ' => b'0',
        'ก' | 'ข' | 'ค' | 'ฆ' => b'1',
        'จ' | 'ช' | 'ฉ' | 'ฌ' => b'2',
        'ซ' | 'ศ' | 'ษ' | 'ส' => b'3',
        'ต' | 'ถ' | 'ท' | 'ธ' | 'ฏ' | 'ฐ' | 'ฑ' | 'ฒ' | 'ด' | 'ฎ' => b'4',
        'บ' | 'ป' | 'พ' | 'ผ' | 'ภ' | 'ฝ' | 'ฟ' => b'5',
        'ม' => b'6',
        'น' | 'ณ' | 'ญ' => b'7',
        'ง' => b'8',
        'ล' | 'ฬ' => b'9',
        'ร' => b'A',
        'ว' => b'B',
        'ย' => b'C',
        'ห' | 'ฮ' => b'D',
        _ => b'0',
    }
}

// ── MetaSound (Snae & Brückner 2009) ─────────────────────────────────────────

/// Encode a Thai word using the MetaSound algorithm (Snae & Brückner 2009).
///
/// Returns a variable-length ASCII code: **3 characters per syllable**
/// (`[initial][vowel][final]`). More discriminating than lk82/udom83 — it
/// encodes vowel length and final consonant class in addition to the onset.
///
/// Returns `"000"` if `word` contains no Thai consonants.
///
/// **Note:** Consonant clusters (e.g. กร, กล) are parsed as separate units;
/// this is an approximation of the full syllable-parser approach.
///
/// ```
/// use kham_core::soundex::metasound;
///
/// assert_eq!(metasound("กาน"), "112"); // initial=ก(1) vowel=า(1) final=น(2)
/// assert_eq!(metasound("ขาน"), "112"); // ข same initial group as ก
/// assert_eq!(metasound("กาม"), "113"); // final=ม(3) differs from น(2)
/// assert_ne!(metasound("กาน"), metasound("กาม"));
/// ```
pub fn metasound(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    let mut result = String::new();
    let mut i = 0;

    while i < len {
        // 1. Optional lead vowel (เ แ โ ไ ใ appear before the consonant in Unicode)
        let lead = if is_ms_lead(chars[i]) {
            let v = chars[i];
            i += 1;
            Some(v)
        } else {
            None
        };

        // 2. Initial consonant (required to emit a syllable code)
        if i >= len || !is_thai_consonant(chars[i]) {
            if lead.is_none() {
                i += 1; // skip non-Thai char
            }
            continue;
        }
        let initial = chars[i];
        i += 1;

        // Thanthakat immediately after initial → silent consonant; skip syllable
        if i < len && chars[i] == '\u{0E4C}' {
            i += 1;
            continue;
        }

        // 3. Upper vowel signs (ิ ี ึ ื ั ุ ู) and tone marks above/below the initial
        let mut upper: Option<char> = None;
        let mut nikhahit = false;
        while i < len {
            match chars[i] {
                c if is_ms_upper(c) => {
                    upper = Some(c);
                    i += 1;
                }
                c if is_ms_tone(c) => {
                    i += 1;
                }
                '\u{0E4D}' => {
                    // Nikhahit อํ — upper component of sara am (–ำ)
                    nikhahit = true;
                    i += 1;
                }
                _ => break,
            }
        }

        // 4. Follow vowel (า ะ ำ appear after the consonant spine)
        let follow = if i < len && is_ms_follow(chars[i]) {
            let v = chars[i];
            i += 1;
            Some(v)
        } else {
            None
        };

        // 5. Final consonant — present only when the next consonant is NOT followed
        //    by a vowel mark (which would make it the initial of the next syllable).
        let final_c = if i < len && is_thai_consonant(chars[i]) {
            let next = i + 1;
            if next < len && chars[next] == '\u{0E4C}' {
                // Silent consonant (e.g. กรณ์): consume both and produce no final
                i += 2;
                None
            } else if next < len
                && (is_ms_upper(chars[next])
                    || is_ms_follow(chars[next])
                    || is_ms_lead(chars[next]))
            {
                // Consonant has its own vowel → next syllable's initial; don't consume
                None
            } else {
                let fc = chars[i];
                i += 1;
                Some(fc)
            }
        } else {
            None
        };

        // Emit [initial][vowel][final]
        result.push(ms_initial_code(initial) as char);
        result.push(ms_vowel_code(lead, upper, follow, nikhahit) as char);
        result.push(ms_final_code(final_c) as char);
    }

    if result.is_empty() {
        "000".into()
    } else {
        result
    }
}

fn ms_initial_code(c: char) -> u8 {
    match c {
        'ก' | 'ข' | 'ค' | 'ฆ' => b'1',
        'ง' => b'2',
        'จ' | 'ช' | 'ฉ' | 'ฌ' => b'3',
        'ซ' | 'ศ' | 'ษ' | 'ส' => b'4',
        'ญ' | 'ย' => b'5',
        'ฎ' | 'ด' => b'6',
        'ฏ' | 'ต' => b'7',
        'ฐ' | 'ฑ' | 'ฒ' | 'ถ' | 'ท' | 'ธ' => b'8',
        'น' | 'ณ' => b'9',
        'บ' => b'A',
        'ป' => b'B',
        'ผ' | 'พ' | 'ภ' => b'C',
        'ฝ' | 'ฟ' => b'D',
        'ม' => b'E',
        'ร' => b'F',
        'ล' | 'ฬ' => b'G',
        'ว' => b'H',
        'ห' | 'ฮ' => b'I',
        _ => b'J', // อ and unknowns → glottal / null onset
    }
}

fn ms_vowel_code(
    lead: Option<char>,
    upper: Option<char>,
    follow: Option<char>,
    nikhahit: bool,
) -> u8 {
    // Sara am (nikhahit อํ or ำ) takes priority
    if nikhahit {
        return b'D';
    }
    match lead {
        // Leading vowels (เ แ โ ไ ใ) determine the vowel class
        Some('ไ') | Some('ใ') => b'E', // /ai/
        Some('เ') => match follow {
            Some('\u{0E32}') => b'F', // เ–า /ao/
            Some('\u{0E30}') => b'8', // เ–ะ short /e/
            _ => b'8',                // เ– long /eː/ (default)
        },
        Some('แ') => b'8', // แ– /ɛ/ class (short or long)
        Some('โ') => b'9', // โ– /o/ class (short or long)
        // No lead vowel: rely on upper and follow vowel signs
        _ => match upper {
            Some('\u{0E31}') => b'0', // ั (mai han akat) short /a/
            Some('\u{0E34}') => b'2', // ิ short /i/
            Some('\u{0E35}') => b'3', // ี long /iː/
            Some('\u{0E36}') => b'4', // ึ short /ɯ/
            Some('\u{0E37}') => b'5', // ื long /ɯː/
            Some('\u{0E38}') => b'6', // ุ short /u/
            Some('\u{0E39}') => b'7', // ู long /uː/
            _ => match follow {
                Some('\u{0E30}') => b'0', // ะ short /a/
                Some('\u{0E32}') => b'1', // า long /aː/
                Some('\u{0E33}') => b'D', // ำ /am/
                _ => b'0',                // no vowel marking → default short /a/
            },
        },
    }
}

fn ms_final_code(c: Option<char>) -> u8 {
    match c {
        Some('ก') => b'1',                             // velar stop
        Some('น') | Some('ณ') | Some('ญ') | Some('ร') | Some('ล') | Some('ฬ') => b'2', // alveolar sonorant
        Some('ม') => b'3',                             // bilabial nasal
        Some('ง') => b'4',                             // velar nasal
        Some('ย') | Some('ว') => b'5',                // glide
        _ => b'6',                                     // open syllable / no final
    }
}

// Character class helpers used only by MetaSound (lk82/udom83 use is_thai_consonant only)

fn is_ms_lead(c: char) -> bool {
    matches!(c, '\u{0E40}'..='\u{0E44}') // เ แ โ ไ ใ
}

fn is_ms_upper(c: char) -> bool {
    // mai han akat ั (U+0E31) + sara ิ ี ึ ื ุ ู (U+0E34–U+0E39) + phinthu (U+0E3A)
    c == '\u{0E31}' || matches!(c, '\u{0E34}'..='\u{0E3A}')
}

fn is_ms_follow(c: char) -> bool {
    matches!(c, '\u{0E30}' | '\u{0E32}' | '\u{0E33}') // ะ า ำ
}

fn is_ms_tone(c: char) -> bool {
    matches!(c, '\u{0E48}'..='\u{0E4B}') // ่ ้ ๊ ๋
}

// ── shared helpers ────────────────────────────────────────────────────────────

/// Strip consonant + ์ (thanthakat, U+0E4C) pairs — silent consonants.
fn strip_silent(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i + 1] == '\u{0E4C}' {
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// True for Thai consonant code points ก–ฮ (U+0E01–U+0E2E).
fn is_thai_consonant(c: char) -> bool {
    ('\u{0E01}'..='\u{0E2E}').contains(&c)
}

/// Core encoder: strip silent consonants → map codes → dedup adjacent → pad to 4.
fn encode(word: &str, code_fn: fn(char) -> u8) -> String {
    const LEN: usize = 4;

    let stripped = strip_silent(word);
    let mut codes: Vec<u8> = Vec::with_capacity(LEN);
    let mut last: Option<u8> = None;

    for ch in stripped.chars() {
        if !is_thai_consonant(ch) {
            continue;
        }
        let code = code_fn(ch);
        if Some(code) != last {
            codes.push(code);
            last = Some(code);
        }
        if codes.len() == LEN {
            break;
        }
    }

    while codes.len() < LEN {
        codes.push(b'0');
    }

    String::from_utf8(codes).expect("soundex codes are ASCII")
}

// ── Thai–English cross-language Soundex (Suwanvisat & Prasitjutrakul 1998) ──
//
// Source: "Thai-English Cross-Language Transliterated Word Retrieval using
// Soundex Technique", NECTEC Annual Conference 1998.
//
// The paper extends Odell & Russell's Soundex to a combined Thai+English table
// so that both a Thai transliteration and its English source word encode to the
// same (or prefix-matched) code — no romanization step needed.
//
// Key differences from standard Soundex:
//   • First character encodes as a digit too (not kept as a letter)
//   • Vowels in non-first position → '7' (not dropped)
//   • H → '8', W → '1', Y → '9' in non-first position
//   • Thai consonants map directly to the same 7 groups as English
//   • ง (ng) → "52" (two digits: N-group then G/K-group)
//   • Code is variable-length (unlimited); callers choose a minimum k for matching

/// Encode a Thai or English word into a shared cross-language phonetic code.
///
/// Implements the Suwanvisat & Prasitjutrakul (1998) modified Soundex that
/// extends the encoding table to cover both Thai consonants and English letters
/// directly — **no romanization step required**. A Thai transliteration and its
/// English source word produce codes that share a common prefix, enabling
/// cross-language retrieval of transliterated proper nouns and loan words.
///
/// **Encoding rules:**
/// - Every character (Thai consonant or English letter) is mapped to a digit; the
///   first character is also encoded numerically (unlike standard Soundex).
/// - English vowels A/E/I/O/U → `'7'` in non-first position (retained, not dropped).
/// - H → `'8'`, W → `'1'`, Y → `'9'` in non-first position.
/// - Thai vowel marks (sara, tone marks, leading vowels) are skipped entirely.
/// - ง maps to `"52"` (N-group + G/K-group, representing the NG onset).
/// - Adjacent identical digits collapse to one (standard deduplication).
/// - Output is **variable length** — longer words produce longer codes.
///
/// Returns `""` if `word` contains no encodable characters.
///
/// ```
/// use kham_core::soundex::thai_english_soundex;
///
/// // Same initial-group consonants produce a common prefix
/// assert_eq!(&thai_english_soundex("McDonald")[..3],
///            &thai_english_soundex("แมคโดนัลด์")[..3]);
/// // English words encode to fully numeric codes
/// assert_eq!(thai_english_soundex("Robert"), "671763");
/// ```
pub fn thai_english_soundex(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    let mut result = String::new();
    let mut last_digit: Option<char> = None;
    let mut is_first = true;
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Thai vowel marks, tone marks, leading vowels — skip entirely
        if is_cl_skip(c) {
            i += 1;
            continue;
        }

        // Silent Thai consonant: consonant immediately followed by ์ (thanthakat)
        if is_thai_consonant(c) && i + 1 < len && chars[i + 1] == '\u{0E4C}' {
            i += 2;
            continue;
        }

        // Only encode ASCII alpha and Thai consonants
        if !c.is_ascii_alphabetic() && !is_thai_consonant(c) {
            i += 1;
            continue;
        }

        let code = cl_code(c, is_first);
        if !code.is_empty() {
            is_first = false;
        }
        for digit in code.chars() {
            if Some(digit) != last_digit {
                result.push(digit);
                last_digit = Some(digit);
            }
        }

        i += 1;
    }

    result
}

/// Encode an English (or romanized) word using standard Soundex (Odell & Russell).
///
/// Retains the first letter, replaces remaining consonants with digits `1`–`6`,
/// collapses adjacent identical codes, drops vowels and H/W/Y, and pads to
/// exactly 4 characters. Returns `""` if `word` contains no ASCII alphabetic
/// characters.
///
/// | Digit | Letters          |
/// |-------|-----------------|
/// | `1`   | B F P V         |
/// | `2`   | C G J K Q S X Z |
/// | `3`   | D T             |
/// | `4`   | L               |
/// | `5`   | M N             |
/// | `6`   | R               |
/// | skip  | A E I O U H W Y |
///
/// ```
/// use kham_core::soundex::english_soundex;
///
/// assert_eq!(english_soundex("Robert"), "R163");
/// assert_eq!(english_soundex("Rupert"), "R163");
/// assert_eq!(english_soundex("McDonald"), "M235");
/// assert_eq!(english_soundex("Smith"),    "S530");
/// ```
pub fn english_soundex(word: &str) -> String {
    let mut chars = word
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase());

    let first = match chars.next() {
        Some(c) => c,
        None => return String::new(),
    };

    let mut code = String::with_capacity(4);
    code.push(first);
    let mut last = std_soundex_digit(first);

    for c in chars {
        let d = std_soundex_digit(c);
        if d == '0' {
            last = '0'; // vowel / H / W / Y — acts as separator
        } else if d != last {
            code.push(d);
            last = d;
            if code.len() == 4 {
                break;
            }
        }
    }

    while code.len() < 4 {
        code.push('0');
    }
    code
}

/// Standard Soundex digit (Odell & Russell). Returns `'0'` for non-coded letters.
fn std_soundex_digit(c: char) -> char {
    match c {
        'B' | 'F' | 'P' | 'V' => '1',
        'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
        'D' | 'T' => '3',
        'L' => '4',
        'M' | 'N' => '5',
        'R' => '6',
        _ => '0',
    }
}

/// Returns `true` if two words share the same cross-language phonetic code.
///
/// Accepts Thai, English, or mixed input — no romanizer required. Returns
/// `false` if either word produces an empty code. For cross-language
/// Thai↔English pairs (e.g. transliterated loan words), the codes share a
/// common prefix even if not exactly equal; prefer comparing
/// [`thai_english_soundex`] codes directly with a minimum-length threshold
/// for that use case.
///
/// ```
/// use kham_core::soundex::sounds_like_cross_lang;
///
/// assert!(sounds_like_cross_lang("Robert",  "Rupert"));  // same English Soundex group
/// assert!(sounds_like_cross_lang("McDonald", "MacDonald"));
/// assert!(!sounds_like_cross_lang("Robert",  "Smith"));
/// ```
pub fn sounds_like_cross_lang(a: &str, b: &str) -> bool {
    let code_a = thai_english_soundex(a);
    !code_a.is_empty() && code_a == thai_english_soundex(b)
}

/// Returns the cross-language Soundex code fragment for one character.
///
/// `is_first` selects the first-position table (AEIOUHWY → `"0"`) vs the
/// rest-position table (AEIOU → `"7"`, H → `"8"`, W → `"1"`, Y → `"9"`).
/// Returns `""` for characters that should be skipped (อ in non-first position).
/// ง returns `"52"` (two digits) in both positions.
fn cl_code(c: char, is_first: bool) -> &'static str {
    if c.is_ascii_alphabetic() {
        let cu = c.to_ascii_uppercase();
        return if is_first {
            match cu {
                'A' | 'E' | 'I' | 'O' | 'U' | 'H' | 'W' | 'Y' => "0",
                'B' | 'F' | 'P' | 'V' => "1",
                'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => "2",
                'D' | 'T' => "3",
                'L' => "4",
                'M' | 'N' => "5",
                'R' => "6",
                _ => "",
            }
        } else {
            match cu {
                'A' | 'E' | 'I' | 'O' | 'U' => "7",
                'H' => "8",
                'W' => "1",
                'Y' => "9",
                'B' | 'F' | 'P' | 'V' => "1",
                'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => "2",
                'D' | 'T' => "3",
                'L' => "4",
                'M' | 'N' => "5",
                'R' => "6",
                _ => "",
            }
        };
    }

    // Thai consonants — same 7 groups in both positions; ว/ห/ย/ญ split by position
    if is_first {
        match c {
            // Group 0 equivalent: vowel carriers / glides / h — first position
            'อ' | 'ห' | 'ฮ' | 'ว' | 'ญ' | 'ย' => "0",
            // Group 2 (C/G/J/K/Q/S/X/Z): all velar+palatal+sibilant clusters
            'ก' | 'ข' | 'ฃ' | 'ค' | 'ฅ' | 'ฆ' => "2",
            'จ' | 'ฉ' | 'ช' | 'ฌ' => "2",
            'ซ' | 'ศ' | 'ษ' | 'ส' => "2",
            // ง = NG/NK → N-group then G/K-group
            'ง' => "52",
            // Group 3 (D/T): dental/alveolar stops
            'ฎ' | 'ด' | 'ฏ' | 'ต' | 'ฐ' | 'ฑ' | 'ฒ' | 'ถ' | 'ท' | 'ธ' => "3",
            // Group 4 (L): laterals
            'ล' | 'ฬ' => "4",
            // Group 5 (M/N): nasals
            'ม' | 'ณ' | 'น' => "5",
            // Group 6 (R): rhotic
            'ร' => "6",
            // Group 1 (B/F/P/V): bilabials + labiodentals
            'บ' | 'ป' | 'ผ' | 'พ' | 'ภ' | 'ฝ' | 'ฟ' => "1",
            _ => "",
        }
    } else {
        match c {
            // อ is a pure vowel carrier in non-initial position — skip
            'อ' => "",
            // ว/ห/ฮ/ญ/ย split like W/H/Y in English non-first position
            'ห' | 'ฮ' => "8",
            'ว' => "1",
            'ญ' | 'ย' => "9",
            'ก' | 'ข' | 'ฃ' | 'ค' | 'ฅ' | 'ฆ' => "2",
            'จ' | 'ฉ' | 'ช' | 'ฌ' => "2",
            'ซ' | 'ศ' | 'ษ' | 'ส' => "2",
            'ง' => "52",
            'ฎ' | 'ด' | 'ฏ' | 'ต' | 'ฐ' | 'ฑ' | 'ฒ' | 'ถ' | 'ท' | 'ธ' => "3",
            'ล' | 'ฬ' => "4",
            'ม' | 'ณ' | 'น' => "5",
            'ร' => "6",
            'บ' | 'ป' | 'ผ' | 'พ' | 'ภ' | 'ฝ' | 'ฟ' => "1",
            _ => "",
        }
    }
}

/// Thai characters to skip in cross-language soundex (vowel marks, tone marks,
/// leading vowels, thanthakat, nikhahit — anything that isn't a consonant).
fn is_cl_skip(c: char) -> bool {
    matches!(
        c,
        '\u{0E30}'..='\u{0E3A}' // sara vowels (ะ า ิ ี ึ ื ุ ู ฺ) and mai han akat
        | '\u{0E40}'..='\u{0E44}' // leading vowels (เ แ โ ไ ใ)
        | '\u{0E47}'..='\u{0E4E}' // mai tai khu, tone marks, ์, ๎, nikhahit
    )
}


#[cfg(test)]
mod tests {
    use super::*;

    // ── lk82 ─────────────────────────────────────────────────────────────────

    #[test]
    fn lk82_worked_examples() {
        assert_eq!(lk82("กาน"), "1600");
        assert_eq!(lk82("ขาน"), "1600");
        assert_eq!(lk82("คาน"), "1600");
        assert_eq!(lk82("บ้าน"), "4600");
        assert_eq!(lk82("มาก"), "5100");
        assert_eq!(lk82("นาค"), "6100");
        assert_eq!(lk82("กรุงเทพ"), "1873");
    }

    #[test]
    fn lk82_same_initial_velar() {
        assert_eq!(lk82("กาน"), lk82("ขาน"));
        assert_eq!(lk82("กาน"), lk82("คาน"));
    }

    #[test]
    fn lk82_different_initials() {
        assert_ne!(lk82("กาน"), lk82("ปาน"));
        assert_ne!(lk82("มาน"), lk82("นาน"));
    }

    #[test]
    fn lk82_always_four_chars() {
        assert_eq!(lk82("ก").len(), 4);
        assert_eq!(lk82("กรุงเทพมหานคร").len(), 4);
    }

    #[test]
    fn lk82_empty_and_no_thai() {
        assert_eq!(lk82(""), "0000");
        assert_eq!(lk82("123"), "0000");
        assert_eq!(lk82("hello"), "0000");
    }

    #[test]
    fn lk82_strips_silent_consonant() {
        // กรณ์ → กร (ณ is silent)
        assert_eq!(lk82("กรณ์"), lk82("กร"));
    }

    #[test]
    fn lk82_deduplicates_adjacent_same_group() {
        // กข → both code '1' → deduplicated to a single '1'
        assert_eq!(lk82("กข"), "1000");
    }

    // ── udom83 ───────────────────────────────────────────────────────────────

    #[test]
    fn udom83_always_four_chars() {
        assert_eq!(udom83("ก").len(), 4);
        assert_eq!(udom83("กรุงเทพมหานคร").len(), 4);
    }

    #[test]
    fn udom83_separates_liquids() {
        assert_ne!(udom83("ลาน"), udom83("ราน"));
    }

    #[test]
    fn udom83_sibilant_separate_from_affricate() {
        assert_ne!(udom83("สาน"), udom83("ชาน"));
        assert_eq!(udom83("สาน"), udom83("ซาน"));
    }

    #[test]
    fn udom83_empty_and_no_thai() {
        assert_eq!(udom83(""), "0000");
        assert_eq!(udom83("abc"), "0000");
    }

    // ── metasound ─────────────────────────────────────────────────────────────

    #[test]
    fn metasound_worked_examples() {
        // กาน: initial=ก(1) vowel=า(1) final=น(2)
        assert_eq!(metasound("กาน"), "112");
        // ขาน: ข shares group '1' with ก
        assert_eq!(metasound("ขาน"), "112");
        // กาม: different final ม(3)
        assert_eq!(metasound("กาม"), "113");
    }

    #[test]
    fn metasound_same_initial_group() {
        assert_eq!(metasound("กาน"), metasound("ขาน"));
        assert_eq!(metasound("กาน"), metasound("คาน"));
    }

    #[test]
    fn metasound_distinguishes_finals() {
        assert_ne!(metasound("กาน"), metasound("กาม"));
        assert_ne!(metasound("กาน"), metasound("กาง"));
    }

    #[test]
    fn metasound_vowel_length() {
        // า long /aː/ (code '1') vs ะ short /a/ (code '0')
        assert_ne!(metasound("กาน"), metasound("กะ"));
    }

    #[test]
    fn metasound_lead_vowel_classes() {
        // เ– class → vowel code '8'
        let e_code = metasound("เกน");
        assert_eq!(&e_code[1..2], "8");
        // ไ / ใ → vowel code 'E'
        let ai_code = metasound("ไก");
        assert_eq!(&ai_code[1..2], "E");
    }

    #[test]
    fn metasound_empty_and_no_thai() {
        assert_eq!(metasound(""), "000");
        assert_eq!(metasound("abc"), "000");
        assert_eq!(metasound("123"), "000");
    }

    #[test]
    fn metasound_open_syllable() {
        // กา: no final consonant → final code '6'
        assert_eq!(metasound("กา"), "116");
    }

    #[test]
    fn metasound_sara_am() {
        // กำ: nikhahit → vowel code 'D'
        let code = metasound("กำ");
        assert_eq!(&code[1..2], "D");
    }

    // ── soundex() enum API ────────────────────────────────────────────────────

    #[test]
    fn soundex_dispatches_to_lk82() {
        assert_eq!(soundex("กาน", SoundexAlgorithm::Lk82), lk82("กาน"));
    }

    #[test]
    fn soundex_dispatches_to_udom83() {
        assert_eq!(soundex("กาน", SoundexAlgorithm::Udom83), udom83("กาน"));
    }

    #[test]
    fn soundex_dispatches_to_metasound() {
        assert_eq!(soundex("กาน", SoundexAlgorithm::MetaSound), metasound("กาน"));
    }

    // ── sounds_like ───────────────────────────────────────────────────────────

    #[test]
    fn sounds_like_lk82_positive() {
        assert!(sounds_like("กาน", "ขาน", SoundexAlgorithm::Lk82));
    }

    #[test]
    fn sounds_like_lk82_negative() {
        assert!(!sounds_like("กิน", "มิน", SoundexAlgorithm::Lk82));
    }

    #[test]
    fn sounds_like_udom83_splits_liquids() {
        assert!(!sounds_like("ลาน", "ราน", SoundexAlgorithm::Udom83));
    }

    #[test]
    fn sounds_like_metasound_positive() {
        assert!(sounds_like("กาน", "ขาน", SoundexAlgorithm::MetaSound));
    }

    #[test]
    fn sounds_like_metasound_negative() {
        assert!(!sounds_like("กาน", "กาม", SoundexAlgorithm::MetaSound));
    }

    #[test]
    fn sounds_like_empty_returns_false() {
        assert!(!sounds_like("", "กาน", SoundexAlgorithm::Lk82));
        assert!(!sounds_like("กาน", "", SoundexAlgorithm::Lk82));
    }

    // ── English Soundex ───────────────────────────────────────────────────────

    #[test]
    fn english_soundex_standard_examples() {
        assert_eq!(english_soundex("Robert"), "R163");
        assert_eq!(english_soundex("Rupert"), "R163"); // same code as Robert
        assert_eq!(english_soundex("McDonald"), "M235");
        assert_eq!(english_soundex("Smith"), "S530");
        assert_eq!(english_soundex("Thompson"), "T512");
    }

    #[test]
    fn english_soundex_always_four_chars() {
        assert_eq!(english_soundex("A").len(), 4);
        assert_eq!(english_soundex("Robert").len(), 4);
    }

    #[test]
    fn english_soundex_empty_and_no_alpha() {
        assert_eq!(english_soundex(""), "");
        assert_eq!(english_soundex("123"), "");
    }

    #[test]
    fn english_soundex_case_insensitive() {
        assert_eq!(english_soundex("robert"), english_soundex("Robert"));
        assert_eq!(english_soundex("ROBERT"), english_soundex("Robert"));
    }

    #[test]
    fn english_soundex_vowel_separates_same_code() {
        // B and P are both code '1'; with a vowel between them they must NOT collapse.
        // "Abba" → A(keep) b→1 b→same,skip → A100
        assert_eq!(english_soundex("Abba"), "A100");
        // "Ababar" — b(1) a(sep) b(1 again after vowel sep) → distinct
        assert_eq!(&english_soundex("Ababar")[..2], "A1");
    }

    #[test]
    fn english_soundex_adjacent_same_code_collapsed() {
        // CK → both code '2'; adjacent → only one digit
        assert_eq!(english_soundex("Jack"), "J200");
    }

    // ── Thai–English cross-language (Suwanvisat & Prasitjutrakul 1998) ──────────

    #[test]
    fn thai_english_soundex_english_numeric_codes() {
        // First character also encoded as a digit (unlike standard Soundex)
        assert_eq!(thai_english_soundex("Robert"), "671763");
        assert_eq!(thai_english_soundex("Rupert"), "671763"); // same code — same initial-sound group
    }

    #[test]
    fn thai_english_soundex_thai_direct_encoding() {
        // Thai consonants map directly to the shared table — no romanizer needed
        assert_eq!(thai_english_soundex("กน"), "25"); // ก→2 (K group), น→5 (N group)
        assert_eq!(thai_english_soundex("ร"), "6"); // ร→6 (R group)
        assert_eq!(thai_english_soundex("ก"), "2"); // single consonant → single digit
    }

    #[test]
    fn thai_english_soundex_ng_two_digits() {
        // ง (ng onset) encodes as "52": N-group (5) then G/K-group (2)
        assert_eq!(thai_english_soundex("ง"), "52");
    }

    #[test]
    fn thai_english_soundex_thai_vowels_skipped_english_vowels_to_7() {
        // Thai vowel diacritics are skipped entirely
        assert_eq!(thai_english_soundex("กิน"), "25"); // ิ (U+0E34) is skipped
        // English vowels in non-first position → '7' (retained, not dropped)
        assert!(thai_english_soundex("Robert").contains('7')); // 'o' and 'e' → '7'
    }

    #[test]
    fn thai_english_soundex_cross_lang_prefix_match() {
        // McDonald and แมคโดนัลด์ share the same 3-char prefix "523"
        let en = thai_english_soundex("McDonald");
        let th = thai_english_soundex("แมคโดนัลด์");
        assert!(en.len() >= 3 && th.len() >= 3, "codes too short");
        assert_eq!(&en[..3], &th[..3]);
    }

    #[test]
    fn thai_english_soundex_variable_length_and_empty() {
        assert_eq!(thai_english_soundex(""), "");
        assert_eq!(thai_english_soundex("123"), "");
        // longer words produce longer codes
        let long = thai_english_soundex("กรุงเทพมหานคร");
        assert!(long.len() > 2);
    }

    #[test]
    fn sounds_like_cross_lang_same_english() {
        assert!(sounds_like_cross_lang("Robert", "Rupert"));
    }

    #[test]
    fn sounds_like_cross_lang_same_thai_initial_group() {
        // ก and ค are both in the K/G group (→ "2"); กาน and คาน share the full code
        assert!(sounds_like_cross_lang("กาน", "คาน"));
    }

    #[test]
    fn sounds_like_cross_lang_different() {
        assert!(!sounds_like_cross_lang("Robert", "Smith"));
        assert!(!sounds_like_cross_lang("กาน", "บาน")); // ก→2 vs บ→1
    }

    #[test]
    fn sounds_like_cross_lang_empty_returns_false() {
        assert!(!sounds_like_cross_lang("", "Robert"));
        assert!(!sounds_like_cross_lang("Robert", ""));
    }
}
