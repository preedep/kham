---
name: romanization
description: Build and test Thai romanization (RTGS) in kham-core. Use when implementing the romanizer module, authoring romanization_th.tsv, writing romanization tests, or integrating romanized output into FtsTokenizer synonyms.
metadata:
  domain: nlp
  triggers: romanize, RTGS, transliteration, romanization_th.tsv, RomanizationMap
  role: specialist
---

# Thai Romanization — kham-core

Specialist for `kham-core/src/romanizer` — table-driven RTGS transliteration of segmented Thai words.

## Standard: RTGS (1999)

**Royal Thai General System of Transcription** (ราชบัณฑิตยสถาน, B.E. 2542 / 1999) is the Thai government official standard:
- Consonant-by-consonant mapping (initial vs. final position differ)
- No tone marks in output
- No vowel-length distinction in output (long/short vowels map identically)
- **Romanization reflects actual pronunciation, not spelling** — e.g. ทราบ /saːp/ = `sap`, ศรี /siː/ = `si`
- No diacritics on Roman letters (ü, ā, etc. are NOT RTGS — use two-letter sequences instead)
- Used in road signs, passports, official documents

Reference: ราชบัณฑิตยสถาน หลักเกณฑ์การถอดอักษรไทยเป็นอักษรโรมัน พ.ศ. 2542

## RTGS Consonant Table (initial / final)

| Thai | Initial | Final |
|------|---------|-------|
| ก | k | k |
| ข ค ฆ | kh | k |
| ง | ng | ng |
| จ ฉ ช ฌ | ch | t |
| ซ ศ ษ ส | s | t |
| ญ | y | n |
| ณ น | n | n |
| ด ฎ | d | t |
| ต ฏ | t | t |
| ถ ท ธ ฐ ฑ ฒ | th | t |
| บ | b | p |
| ป | p | p |
| ผ พ ภ | ph | p |
| ฝ ฟ | f | — (rare as final) |
| ม | m | m |
| ย | y | i (part of diphthong) |
| ร | r | n |
| ล ฬ | l | n |
| ว | w | o or w (part of diphthong) |
| ห ฮ | h | — |
| อ | — (glottal/vowel carrier) | — |

**Note:** ห นำ (leading ห) is a tone marker — silent in romanization. ร ร (ร หัน) with no following consonant = an (อัน).

## RTGS Vowel Table

| Thai vowel form | RTGS | Notes |
|----------------|------|-------|
| –ะ / –ั / –า / อ (inherent) | a | short and long both = a |
| –ิ / –ี | i | short and long both = i |
| –ึ / –ื | ue | short and long both = ue |
| –ุ / –ู | u | short and long both = u |
| เ–ะ / เ– | e | |
| แ–ะ / แ– | ae | |
| เ–อะ / เ–อ | oe | e.g. เธอ = thoe, เบอร์ = boe |
| โ–ะ / โ– / –อ / เ–าะ | o | |
| ไ– / ใ– | ai | |
| เ–า | ao | |
| –ำ | am | |
| เ–ีย / เ–ียะ | ia | |
| เ–ือ / เ–ือะ | uea | |
| –ัว / อัว / –วะ | ua | |

## Special Rules

1. **Pronunciation-based**: Romanize the spoken form, not the spelled form.
   - ทราบ /saːp/ → `sap` (ทร cluster becomes ส sound)
   - ศรี /siː/ → `si` (ศร cluster becomes ส sound, ร silent)
   - จันทร์ /t͡ɕan/ → `chan` (final ร is silent before ์)

2. **Hyphenation**: Use a hyphen when ambiguity arises from a vowel-initial syllable following a vowel-final syllable, or when ⟨ng⟩ would be misread.
   - สะอาด → `sa-at` (not `saat`)

3. **Compound words / proper names**: Written together without spaces.
   - รถไฟ → `rotfai`
   - กรุงเทพ → `krungthep`

4. **Double consonants**: Written as a single consonant in RTGS (gemination not marked).
   - บัตร /bat/ → `bat` (final cluster simplified)

## Module Layout

```
kham-core/
├── src/
│   └── romanizer.rs          # RomanizationMap struct + impl
└── data/
    └── romanization_th.tsv   # built-in RTGS table (hand-curated)
```

## Data File Format (`romanization_th.tsv`)

```tsv
# Thai word → RTGS romanization (one entry per line)
# Format: <thai_word><TAB><rtgs_romanization>
# Lines starting with # are comments; blank lines ignored
# Duplicate keys: last entry wins (allows domain override)
กิน	kin
ข้าว	khao
ปลา	pla
น้ำ	nam
ไฟ	fai
```

Rules:
- Tab-separated, exactly 2 columns
- Thai word is post-normalize (same form as segmenter output)
- RTGS output is lowercase Latin only — **no diacritics** (ü → ue, ā → a)
- Do not include whitespace tokens
- Sort entries alphabetically by Thai word for readability

## Common ue/uea Romanizations (frequent error source)

| Thai | Wrong (uses ü) | Correct RTGS |
|------|---------------|--------------|
| มือ | müe | mue |
| ซื้อ | sü / sue | sue |
| ยืน | yün | yuen |
| ดึง | düng | dueng |
| ดื่ม | düm | duem |
| กึ่ง | küng | kueng |
| ฝึก | fük | fuek |
| เรื่อง | rüang | rueang |
| เสือ | süa | suea |
| เครื่อง | khrüang | khrueang |

## API

```rust
use kham_core::romanizer::RomanizationMap;

// Built-in table (embedded via include_str! at compile time)
let map = RomanizationMap::builtin();

// Custom table (domain-specific overrides)
let map = RomanizationMap::from_tsv("กิน\tkin\nข้าว\tkhao\n");

// Lookup a single pre-segmented word
map.romanize("กิน")           // → Some("kin")
map.romanize("xyz")           // → None
map.romanize_or_raw("กิน")   // → "kin"
map.romanize_or_raw("xyz")   // → "xyz"

// Romanize a full token list (output aligned 1:1 with input)
let tokens = vec!["กิน", "ข้าว", "ปลา"];
map.romanize_tokens(&tokens)  // → ["kin", "khao", "pla"]
```

## Implementation Rules

- `no_std` / `alloc`-only — no `std` imports; follow same pattern as `SynonymMap`
- Backed by `BTreeMap<String, String>` — consistent with `SynonymMap`
- `romanize()` returns `Option<&str>` borrowing from map internals — zero-copy for hits
- `from_tsv()` parser: skip `#` lines and blank lines; split on first `\t`; last duplicate wins
- Do NOT implement a rule-based phonetic engine — table lookup only
- Rule-based RTGS engine is a future `#[cfg(feature = "phonetic")]` extension

## Integration with FtsTokenizer

Romanizations can be added as synonyms in `FtsToken.synonyms` so Thai words match Latin-script queries:

```rust
// FtsTokenizer builder (future integration)
FtsTokenizer::builder()
    .romanization(RomanizationMap::builtin())  // adds RTGS form to synonyms
    .build()
```

In PostgreSQL: searching `kin` would match documents containing `กิน` via the synonym chain.

## Writing Tests

Place tests in `kham-core/src/romanizer.rs` (unit) and `kham-core/tests/romanization.rs` (integration).

```rust
#[test]
fn test_builtin_common_words() {
    let map = RomanizationMap::builtin();
    assert_eq!(map.romanize("กิน"), Some("kin"));
    assert_eq!(map.romanize("ข้าว"), Some("khao"));
    assert_eq!(map.romanize("น้ำ"), Some("nam"));
}

#[test]
fn test_ue_vowel_no_diacritics() {
    let map = RomanizationMap::builtin();
    assert_eq!(map.romanize("มือ"), Some("mue"));    // not "müe"
    assert_eq!(map.romanize("เรื่อง"), Some("rueang")); // not "rüang"
}

#[test]
fn test_unknown_word_returns_none() {
    let map = RomanizationMap::builtin();
    assert_eq!(map.romanize("เปปซี่"), None);
}

#[test]
fn test_romanize_or_raw_fallback() {
    let map = RomanizationMap::builtin();
    assert_eq!(map.romanize_or_raw("เปปซี่"), "เปปซี่");
}

#[test]
fn test_from_tsv_last_duplicate_wins() {
    let map = RomanizationMap::from_tsv("กิน\tkin\nกิน\tgin\n");
    assert_eq!(map.romanize("กิน"), Some("gin"));
}
```

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| Using ü for สระ อึ/อื | Use `ue` — RTGS has zero diacritics |
| Using ā, ī, etc. for long vowels | RTGS ignores vowel length — same output as short |
| Tone marks in TSV key don't match segmenter output | Always run `normalize()` on input before lookup |
| ร at word-final maps to `n` not `r` | Final position consonant rules differ from initial |
| อ as vowel carrier is silent | No RTGS output for silent อ |
| Romanizing spelling instead of pronunciation | ทราบ = `sap`, ศรี = `si` — always use spoken form |
| Building rule-based engine instead of table | Start with table; gate rule engine behind feature flag |

## Implementation Checklist

- [x] Create `kham-core/data/romanization_th.tsv` with ~200 high-frequency words
- [x] Implement `kham-core/src/romanizer.rs` (`RomanizationMap` struct, `from_tsv`, `builtin`, `romanize`, `romanize_or_raw`, `romanize_tokens`)
- [x] Register module in `kham-core/src/lib.rs` (`pub mod romanizer`)
- [x] Unit tests in `romanizer.rs`
- [x] Integration tests in `kham-core/tests/romanization.rs`
- [x] Doc comments with Thai+English examples on all public APIs
- [ ] Update `Architecture` section in README
- [ ] Add `romanization` to `FtsTokenizer::builder()` (optional, can be Phase 2)
