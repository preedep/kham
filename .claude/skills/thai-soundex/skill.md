---
name: thai-soundex
description: Implement Thai phonetic encoding (Soundex) algorithms in kham-core. Use when implementing lk82, udom83, MetaSound, or cross-language soundex; designing the soundex module API; integrating phonetic codes into FtsTokenizer synonyms; or writing phonetic-matching tests.
metadata:
  domain: nlp
  triggers: soundex, phonetic, lk82, udom83, MetaSound, phonetic encoding, fuzzy name matching
  role: specialist
---

# Thai Phonetic Encoding (Soundex) — kham-core

Specialist for `kham-core/src/soundex.rs` — phonetic encoding of Thai words for fuzzy search,
name matching, and spell-correction.

## Background: Why Thai Needs Its Own Soundex

English Soundex groups Roman letters by articulation class (labial, dental, velar…).
Thai phonetics has a different structure:

- **44 consonants** mapped to ~20 distinct sounds (many are spelling variants of the same sound)
- **3 consonant classes** (สูง/กลาง/ต่ำ) that affect tone but not the consonant sound itself
- **Initial consonant** — the onset; the primary discriminator in soundex
- **Final consonant** — only 8 final sounds possible in Thai (ก, น, ม, ง, ย, ว, and the glottal stop + open)
- **Vowel nuclei** — long/short distinction; Thai soundex systems usually drop vowels entirely
- **Tones** — 5 tones; always ignored in phonetic encoding

A Thai word is phonologically: **[initial consonant] + [vowel] + [final consonant]**

Soundex systems exploit the many-to-one mapping of Thai spelling to pronunciation:
- ค ข ฆ → all pronounced /kʰ/ initially → same soundex code
- ต ถ ท ธ ฏ ฐ ฑ ฒ → all stop at /t/ or /tʰ/ → grouped together
- น ณ → both /n/ → same code

## Algorithms

See detailed reference files:

| Algorithm | File | Complexity | Priority |
|---|---|---|---|
| lk82 | `references/lk82.md` | Low | High — most widely deployed |
| udom83 | `references/udom83.md` | Low | High — implement alongside lk82 |
| MetaSound | `references/metasound.md` | Medium | Medium |
| Thai–English cross-language | `references/cross-language.md` | Medium | Low |
| HMM + trigram hybrid | — | High | Deferred (needs ML) |

## Module Layout

```
kham-core/
└── src/
    └── soundex.rs     # all soundex variants; no data file needed (tables are small, inline)
```

No TSV data file — the consonant-group tables are small enough to be `const` arrays or `match`
expressions directly in source.

## API Design

```rust
// kham-core/src/soundex.rs

/// Encode a Thai word using the LK82 algorithm (Lorchirachoonkul 1982).
///
/// Returns a 4-character ASCII code. Returns an empty string if `word`
/// contains no Thai consonants.
///
/// ```
/// use kham_core::soundex::lk82;
/// assert_eq!(lk82("รถ"), lk82("รด"));   // same initial sound group
/// ```
pub fn lk82(word: &str) -> alloc::string::String { … }

/// Encode a Thai word using the Udom83 algorithm (Udompanich 1983).
pub fn udom83(word: &str) -> alloc::string::String { … }

/// Encode a Thai word using MetaSound (Snae & Brückner 2009).
pub fn metasound(word: &str) -> alloc::string::String { … }

/// Returns true if two words have the same LK82 code (phonetically similar).
pub fn sounds_like_lk82(a: &str, b: &str) -> bool {
    !a.is_empty() && lk82(a) == lk82(b)
}

/// Same as above for Udom83.
pub fn sounds_like_udom83(a: &str, b: &str) -> bool {
    !a.is_empty() && udom83(a) == udom83(b)
}
```

## General Implementation Pattern

All rule-based soundex algorithms follow the same skeleton:

```rust
pub fn lk82(word: &str) -> String {
    // 1. Collect Thai consonants (skip vowel marks, tone marks, non-Thai)
    // 2. Map each consonant to its group code via the encoding table
    // 3. Remove consecutive duplicate codes (like English Soundex)
    // 4. Pad/truncate to fixed length (lk82: 4 chars)
    // 5. Return ASCII code string
}
```

Key helpers needed:
- `is_thai_consonant(c: char) -> bool` — `'\u{0E01}'..='\u{0E2E}'`
- `is_thai_vowel_mark(c: char) -> bool` — sara characters and tone marks to skip
- `consonant_to_lk82_code(c: char) -> Option<u8>` — the encoding table

## Thai Character Ranges (useful for implementation)

```rust
// Thai consonants: U+0E01–U+0E2E (ก to ฮ), 44 letters
const THAI_CONSONANTS: std::ops::RangeInclusive<char> = '\u{0E01}'..='\u{0E2E}';

// Vowel signs / diacritics to skip (appear above/below/around consonants):
// U+0E30–U+0E3A (สระ), U+0E40–U+0E44 (leading vowels), U+0E47–U+0E4E (mai)
// Tone marks: U+0E48–U+0E4B (mai ek, tho, tri, jattawa)
// Thanthakat (silent): U+0E4C
// Nikkhahit: U+0E4D
```

## no_std / alloc Rules

- No `std` — use `alloc::string::String`, `alloc::vec::Vec`
- Encoding tables as `const` arrays or `match` — no heap allocation for the table itself
- Only the output `String` is heap-allocated

## Integration with FtsTokenizer

The natural place is as an additional synonym source alongside RTGS romanization:

```rust
// Proposed builder method (future)
FtsTokenizer::builder()
    .soundex(SoundexAlgorithm::Lk82)   // adds lk82 code to FtsToken::synonyms
    .build()
```

In `FtsToken::synonyms`, for a Thai token:
```
กิน → synonyms: ["kin" (RTGS), "7100" (lk82 code)]
```

This enables: searching the lk82 code `"7100"` matches documents containing `กิน`,
`คิน`, `ขิน` etc. (same soundex group).

**Caution:** soundex codes are short and collision-prone. Only add the code to synonyms
when there is genuine value (e.g., name-entity fields). Do not add soundex codes to all
Thai tokens by default — it will hurt FTS precision significantly.

## Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Same-sounding words must share a code
    #[test]
    fn lk82_groups_same_initial_sound() {
        // ก and ข and ค all have /k/ / /kʰ/ initials → same group
        assert_eq!(lk82("กาน"), lk82("ขาน"));
        assert_eq!(lk82("กาน"), lk82("คาน"));
    }

    // Different initial sounds must differ
    #[test]
    fn lk82_separates_different_initials() {
        assert_ne!(lk82("กาน"), lk82("ปาน"));
        assert_ne!(lk82("มาน"), lk82("นาน"));
    }

    // Non-Thai input → empty or pass-through
    #[test]
    fn lk82_empty_on_no_consonants() {
        assert_eq!(lk82(""), "");
        assert_eq!(lk82("123"), "");
    }

    // Padding to fixed length
    #[test]
    fn lk82_always_four_chars() {
        assert_eq!(lk82("ก").len(), 4);
        assert_eq!(lk82("กระทรวงศึกษาธิการ").len(), 4);
    }

    // sounds_like helpers
    #[test]
    fn sounds_like_lk82_symmetric() {
        assert!(sounds_like_lk82("รถ", "รด"));
        assert!(!sounds_like_lk82("กิน", "มิน"));
    }
}
```

## Common Pitfalls

| Pitfall | Fix |
|---|---|
| Processing vowel marks as consonants | Skip `U+0E30–U+0E4E` before consonant lookup |
| Forgetting leading vowels (เ, แ, โ, ไ, ใ) | `U+0E40–U+0E44` appear before the consonant in text but phonologically follow it — skip for soundex |
| Not removing consecutive duplicate codes | Like English Soundex: `ก` immediately followed by `ข` → emit code only once |
| Fixed-length truncation off-by-one | Code should be exactly N chars; pad with `'0'` if short, truncate if long |
| Applying soundex to non-Thai tokens | Guard: return `""` immediately if word has no Thai consonants |
| Using tone class (สูง/กลาง/ต่ำ) in grouping | Tone class affects tone, not consonant sound — ignore it; group by **sound**, not class |

## Implementation Checklist

- [ ] Create `kham-core/src/soundex.rs` with `lk82` and `udom83` functions
- [ ] Register `pub mod soundex` in `kham-core/src/lib.rs`
- [ ] Add `sounds_like_lk82` / `sounds_like_udom83` convenience wrappers
- [ ] Unit tests in `soundex.rs` (same-sound grouping, fixed length, non-Thai input)
- [ ] Doc comments with Thai+English examples on all public items
- [ ] Implement `MetaSound` after lk82/udom83 are stable
- [ ] Add `soundex` builder option to `FtsTokenizer` (optional, can be Phase 2)

## References

- PyThaiNLP implementation (Apache-2.0): <https://github.com/PyThaiNLP/pythainlp/tree/master/pythainlp/soundex>
- lk82 original paper: Lorchirachoonkul, V. (1982). *A Phonetic Coding System for Thai Language.*
- udom83 original paper: Udompanich, U. (1983). *Thai Phonetic Encoding.*
- MetaSound: Snae, C., & Brückner, M. (2009). *Novel Phonetic Name Matching Algorithm with a Statistical Ontology for Analysing Names Given in Accordance with Thai Astrology.* WSEAS Transactions on Computers.
- Thai–English: Suwanvisat, P., & Prasitjutrakul, S. (1998). *Thai–English Cross-Language Phonetic Retrieval.* NECTEC.
