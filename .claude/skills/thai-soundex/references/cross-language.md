# Thai–English Cross-Language Soundex (Suwanvisat & Prasitjutrakul, 1998)

**Paper:** Suwanvisat, P., & Prasitjutrakul, S. (1998). *Thai–English Cross-Language
Transliterated Word Retrieval.* NECTEC Annual Conference.

## Purpose

Unlike lk82/udom83/MetaSound which encode Thai→Thai phonetics, this algorithm
encodes **both** Thai transliterations and English source words into the same
phonetic code — so a query for `"แมคโดนัลด์"` (McDonald's) and `"McDonald"` produce
the same or similar codes.

Useful for: Thai–English name matching, transliterated loan word retrieval,
cross-language information retrieval where Thai text contains transliterated
foreign proper nouns.

## How It Works

The algorithm operates on the **romanized form** of the Thai word (Thai → RTGS/phonetic
transcription → apply English Soundex-like rules):

```
Thai word → romanize → normalize to English phoneme mapping → encode
English word → normalize to English phoneme mapping → encode
```

Both paths produce a shared code space, enabling cross-language matching.

## Dependency: Thai Romanizer

This algorithm requires a phonetic transcription step — not RTGS (which is spelling-based),
but pronunciation-based transcription. In kham, the `romanizer` module provides RTGS, which
is close but not identical to pronunciation.

For cross-language soundex, a pronunciation-based romanizer would be needed
(`#[cfg(feature = "phonetic")]` — future extension).

## English Soundex Groupings (the target encoding)

The cross-language algorithm maps Thai sounds to English Soundex digit groups:

| Soundex digit | English letters | Thai approximate equivalents |
|---|---|---|
| `1` | B, F, P, V | บ ป พ ผ ภ ฝ ฟ |
| `2` | C, G, J, K, Q, S, X, Y, Z | ก ข ค จ ช ซ ศ ษ ส ย |
| `3` | D, T | ด ต ถ ท ธ ฎ ฏ ฐ ฑ ฒ |
| `4` | L | ล ฬ |
| `5` | M, N | ม น ณ |
| `6` | R | ร |
| (0) | A, E, I, O, U, H, W, Y | vowels, ห, ว (silent) |

## Algorithm Steps

1. **Romanize** the Thai word (RTGS or pronunciation-based)
2. Convert to uppercase
3. Keep the first letter; remove all vowels (A E I O U) and H, W, Y from the rest
4. Replace remaining consonants with Soundex digits (table above)
5. Remove consecutive duplicate digits
6. Pad or truncate to 4 characters

For English input: steps 2–6 only (skip step 1).

## Example

| Input | Step 1 (romanize) | Step 2–6 (encode) | Code |
|---|---|---|---|
| `แมคโดนัลด์` (McDonald's) | `maekdonald` | M-236 | `M236` |
| `McDonald` | *(skip)* | M-236 | `M236` |
| `ไมโครซอฟต์` (Microsoft) | `maikhroso` | M-262 | `M262` |
| `Microsoft` | *(skip)* | M-262 | `M262` |

## Implementation Prerequisites in kham

Before implementing this algorithm:
1. `kham-core/src/romanizer.rs` — already exists (RTGS table-lookup)
2. A pronunciation-based phonetic transcriber (not yet implemented — needed for accuracy)
3. English Soundex encoder (straightforward, ~20 lines)

**Minimal viable version:** use RTGS romanization as step 1 (not perfect, but functional
for many loan words where RTGS approximates pronunciation).

## Code Sketch

```rust
pub fn thai_english_soundex(word: &str, romanizer: &RomanizationMap) -> alloc::string::String {
    // 1. Try to romanize; fall back to raw word if not in table
    let roman = romanizer.romanize_or_raw(word);
    // 2. Apply English Soundex to the romanized form
    english_soundex_from_roman(&roman.to_uppercase())
}

pub fn english_soundex(word: &str) -> alloc::string::String {
    // Standard English Soundex on Latin input
    english_soundex_from_roman(&word.to_uppercase())
}

fn english_soundex_from_roman(upper: &str) -> alloc::string::String {
    let mut chars = upper.chars().filter(|c| c.is_ascii_alphabetic());
    let first = match chars.next() {
        Some(c) => c,
        None => return alloc::string::String::new(),
    };
    let mut code = alloc::string::String::with_capacity(4);
    code.push(first);
    let mut last = soundex_digit(first);
    for c in chars {
        let d = soundex_digit(c);
        if d != '0' && d != last {
            code.push(d);
            if code.len() == 4 { break; }
        }
        last = d;
    }
    while code.len() < 4 { code.push('0'); }
    code
}

fn soundex_digit(c: char) -> char {
    match c {
        'B'|'F'|'P'|'V'         => '1',
        'C'|'G'|'J'|'K'|'Q'|'S'|'X'|'Z' => '2',
        'D'|'T'                  => '3',
        'L'                      => '4',
        'M'|'N'                  => '5',
        'R'                      => '6',
        _                        => '0',  // vowels, H, W, Y
    }
}
```

## Limitations

- RTGS is spelling-based, not pronunciation-based — some loan words romanize
  differently than their English source (e.g., `เบียร์` → RTGS `bia`, English `beer`)
- Only useful for transliterated foreign words; pure Thai vocabulary has no meaningful
  English Soundex code
- The algorithm assumes the loan word was borrowed from English; fails for French, German,
  Japanese, etc. transliterations

## Reference

- Paper: Suwanvisat, P., & Prasitjutrakul, S. (1998). Thai–English Cross-Language Transliterated Word Retrieval. NECTEC.
- PyThaiNLP may not have a direct implementation — this may need to be implemented from scratch based on the paper.
