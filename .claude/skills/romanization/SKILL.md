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

## Standard: RTGS

**Royal Thai General System of Transcription** is the Thai government official romanization standard:
- Consonant-by-consonant mapping (initial vs. final position differ)
- No tone marks in output
- No vowel-length distinction in output (long/short vowels map identically)
- Diphthongs and vowel clusters have explicit multi-character mappings
- Used in road signs, passports, official documents

Reference: [Royal Institute of Thailand RTGS table](http://www.royin.go.th)

## RTGS Consonant Table (initial / final)

| Thai | Initial | Final |
|------|---------|-------|
| ก | k | k |
| ข ค | kh | k |
| ง | ng | ng |
| จ | ch | t |
| ช | ch | t |
| ซ ส | s | t |
| ญ ย | y | n/y |
| ด ฎ | d | t |
| ต ฏ | t | t |
| ถ ท | th | t |
| น | n | n |
| บ | b | p |
| ป | p | p |
| ผ พ | ph | p |
| ฝ ฟ | f | f |
| ม | m | m |
| ร | r | n |
| ล | l | n |
| ว | w | o/w |
| ห | h | — |
| อ | — | — |
| ฮ | h | — |

## RTGS Vowel Table

| Thai vowel | RTGS |
|-----------|------|
| สระ อา / อ (short) | a |
| สระ อิ / อี | i |
| สระ อุ / อู | u |
| สระ เอ | e |
| สระ แอ | ae |
| สระ โอ | o |
| สระ เอา | ao |
| สระ เอีย | ia |
| สระ เอือ | uea |
| สระ อัว | ua |
| สระ ไ ใ | ai |
| สระ เอา | ao |
| สระ อำ | am |

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
- RTGS output is lowercase Latin only — no diacritics, no uppercase
- Do not include whitespace tokens
- Sort entries alphabetically by Thai word for readability

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

#[test]
fn test_romanize_tokens_aligned() {
    let map = RomanizationMap::from_tsv("กิน\tkin\nปลา\tpla\n");
    let tokens = vec!["กิน", "ปลา"];
    assert_eq!(map.romanize_tokens(&tokens), vec!["kin", "pla"]);
}
```

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| Tone marks in TSV key don't match segmenter output | Always run `normalize()` on input before lookup |
| Long vs. short vowel mismatch | RTGS collapses both — use same output for อิ and อี |
| ร at word-final maps to `n` not `r` | Check consonant position (initial vs. final) |
| อ as vowel carrier is silent | No output for silent อ |
| Building rule-based engine instead of table | Start with table; gate rule engine behind feature flag |

## Implementation Checklist

- [ ] Create `kham-core/data/romanization_th.tsv` with ~200 high-frequency words
- [ ] Implement `kham-core/src/romanizer.rs` (`RomanizationMap` struct, `from_tsv`, `builtin`, `romanize`, `romanize_or_raw`, `romanize_tokens`)
- [ ] Register module in `kham-core/src/lib.rs` (`pub mod romanizer`)
- [ ] Unit tests in `romanizer.rs`
- [ ] Integration tests in `kham-core/tests/romanization.rs`
- [ ] Doc comments with Thai+English examples on all public APIs
- [ ] Update `Architecture` section in README
- [ ] Add `romanization` to `FtsTokenizer::builder()` (optional, can be Phase 2)
