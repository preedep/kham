# Udom83 — Udompanich Phonetic Encoding (1983)

An alternative Thai phonetic soundex algorithm developed one year after lk82.
Uses different consonant groupings — particularly in how it handles the
sibilant/affricate class and the liquid class. Often compared to lk82 to see
which gives better precision for a given application.

## Key Differences vs. lk82

| Aspect | lk82 | udom83 |
|---|---|---|
| Sibilants (ซ ศ ษ ส) | grouped with affricates (จ ช ฌ) | separate group from affricates |
| Liquids (ล ร ฬ) | one group | ร and ล may split |
| Code alphabet | numeric + alpha (`0`–`B`) | numeric only or different scheme |
| Code length | 4 characters | 4 characters |

> **Source:** Udompanich, U. (1983).
> Authoritative open-source reference: PyThaiNLP `pythainlp/soundex/udom83.py`
> Cross-check every group assignment there before implementation.

## Algorithm Steps

Same skeleton as lk82:

1. Pre-strip silent consonants (consonant + ์)
2. Iterate over Thai consonants only (skip vowel signs, tone marks)
3. Map each consonant to its udom83 group code
4. Remove consecutive duplicate codes
5. Pad/truncate to 4 characters
6. Return as string

## Udom83 Consonant Group Table (approximate)

> **Warning:** These groupings are a best-effort reconstruction.
> Verify character-by-character against PyThaiNLP's `udom83.py`.

| Code | Thai consonants | Notes |
|------|----------------|-------|
| `0` | อ | glottal / null onset |
| `1` | ก ข ค ฆ | velar stops |
| `2` | จ ช ฉ ฌ | palatal affricates only |
| `3` | ซ ศ ษ ส | sibilants (separate from affricates, unlike lk82) |
| `4` | ต ถ ท ธ ฏ ฐ ฑ ฒ ด ฎ | dental/alveolar stops |
| `5` | บ ป พ ผ ภ ฝ ฟ | bilabial / labiodental |
| `6` | ม | bilabial nasal |
| `7` | น ณ ญ | alveolar/palatal nasals |
| `8` | ง | velar nasal |
| `9` | ล ฬ | lateral |
| `A` | ร | rhotic |
| `B` | ว | labiovelar |
| `C` | ย | palatal |
| `D` | ห ฮ | glottal/laryngeal |

## Worked Examples (vs. lk82)

| Word | lk82 | udom83 | Observation |
|---|---|---|---|
| สาน | `2600` | `3700` | ส in different group |
| ซาน | `2600` | `3700` | ซ and ส same group in both |
| ชาน | `2600` | `2700` | ช stays in group 2 in udom83 |
| ลาน vs ราน | `8600` / `8600` | `9700` / `A700` | ล and ร split in udom83 |

## When to Prefer udom83 over lk82

- When distinguishing ร (rhotic) from ล (lateral) matters for your domain
  (e.g., Thai names where ร/ล confusion is a meaningful error, not an acceptable variant)
- When sibilant precision matters (ซ/ศ vs. ช)
- Applications where lk82 over-merges and produces too many false matches

## Shared Infrastructure with lk82

Both algorithms share:
- `is_thai_consonant()` helper
- `strip_silent_consonants()` pre-processor
- Same padding/truncation logic (4 chars, pad with `'0'`)
- Same skip logic for vowel signs and tone marks

Structure the module so both functions call shared helpers:

```rust
// In soundex.rs — shared by lk82 and udom83
fn strip_silent(s: &str) -> alloc::string::String { … }
fn is_thai_consonant(c: char) -> bool { … }
fn encode_with_table(word: &str, table: fn(char) -> u8) -> alloc::string::String { … }

pub fn lk82(word: &str) -> alloc::string::String {
    encode_with_table(&strip_silent(word), consonant_to_lk82)
}

pub fn udom83(word: &str) -> alloc::string::String {
    encode_with_table(&strip_silent(word), consonant_to_udom83)
}
```

## Reference

- PyThaiNLP source: <https://github.com/PyThaiNLP/pythainlp/blob/master/pythainlp/soundex/udom83.py>
- Original paper: Udompanich, U. (1983). *Thai Phonetic Encoding.*
