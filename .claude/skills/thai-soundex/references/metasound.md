# MetaSound — Snae & Brückner (2009)

A hybrid phonetic encoding algorithm for Thai that combines ideas from English
Soundex and English Metaphone. Published as part of research on Thai name
matching for astrological/naming datasets.

**Paper:** Snae, C., & Brückner, M. (2009). *Novel Phonetic Name Matching
Algorithm with a Statistical Ontology for Analysing Names Given in Accordance
with Thai Astrology.* WSEAS Transactions on Computers, 8(5).

## Core Idea

Where lk82/udom83 treat all consonants uniformly (iterate, group, deduplicate),
MetaSound adds **vowel-awareness** and **position-awareness**:

- The **initial consonant** is preserved more precisely (less grouping, more detail)
- **Vowel length** is encoded as a separate position in the code
- **Final consonant class** (sonorant vs. stop) is encoded
- Result is longer than 4 chars and more discriminating than lk82/udom83

## MetaSound Code Structure

```
[Initial consonant group][Vowel code][Final consonant group]
```

Each syllable of the Thai word contributes one triple. For multi-syllable words,
concatenate triples and truncate/pad to a fixed output length.

## Initial Consonant Groups (MetaSound)

MetaSound uses finer groupings than lk82 — it keeps the high/mid/low consonant
class distinction in some cases:

| Group | Consonants | Sound |
|---|---|---|
| `1` | ก ข ค ฆ | /k/, /kʰ/ |
| `2` | ง | /ŋ/ |
| `3` | จ ช ฉ ฌ | /tɕ/, /tɕʰ/ |
| `4` | ซ ศ ษ ส | /s/ |
| `5` | ญ ย | /j/ |
| `6` | ฎ ด | /d/ |
| `7` | ฏ ต | /t/ |
| `8` | ฐ ฑ ฒ ถ ท ธ | /tʰ/ |
| `9` | น ณ | /n/ |
| `A` | บ | /b/ |
| `B` | ป | /p/ |
| `C` | ผ พ ภ | /pʰ/ |
| `D` | ฝ ฟ | /f/ |
| `E` | ม | /m/ |
| `F` | ร | /r/ |
| `G` | ล ฬ | /l/ |
| `H` | ว | /w/ |
| `I` | ห ฮ | /h/ |
| `J` | อ | glottal |

> Cross-check against PyThaiNLP `pythainlp/soundex/metasound.py` for exact groupings.

## Vowel Code

MetaSound encodes the vowel nucleus as a single digit:

| Code | Vowel class | Thai examples |
|---|---|---|
| `0` | short /a/ | ะ ั |
| `1` | long /aː/ | า |
| `2` | short /i/ | ิ |
| `3` | long /iː/ | ี |
| `4` | short /ɯ/ | ึ |
| `5` | long /ɯː/ | ื |
| `6` | short /u/ | ุ |
| `7` | long /uː/ | ู |
| `8` | /e/, /ɛ/ class | เ–ะ เ– แ–ะ แ– |
| `9` | /o/, /ɔ/ class | โ–ะ โ– เ–าะ –อ |
| `A` | diphthong /ia/ | เ–ีย |
| `B` | diphthong /ɯa/ | เ–ือ |
| `C` | diphthong /ua/ | –ัว –วะ |
| `D` | /am/ (sara am) | –ำ |
| `E` | /ai/ | ไ– ใ– |
| `F` | /ao/ | เ–า |

## Final Consonant Groups

Only 8 final sounds in Thai — MetaSound groups them into 4 classes:

| Class | Finals | Sound quality |
|---|---|---|
| `1` | ก | velar stop |
| `2` | น ณ ญ ร ล ฬ | alveolar sonorant |
| `3` | ม | bilabial nasal |
| `4` | ง | velar nasal |
| `5` | ย ว | glide (part of diphthong) |
| `6` | (open syllable / glottal stop) | no final or ะ |

## Implementation Complexity

MetaSound is significantly harder to implement than lk82/udom83 because:

1. **You need to parse Thai syllable structure**, not just scan consonants linearly
2. Vowel forms in Thai are discontinuous — leading vowels (เ แ โ ไ ใ) appear
   before the consonant in Unicode, while the vowel's consonant carrier is to the right
3. Tone marks and ห นำ affect how the syllable is interpreted

**Recommended approach:** implement lk82 and udom83 first. MetaSound requires
a Thai syllable parser (TCC-level analysis) as a prerequisite.

### Minimal Syllable Parser Needed

To determine vowel and final consonant, you need to identify:
- The initial consonant (first Thai consonant in the syllable)
- Whether a leading vowel (เ แ โ ไ ใ) precedes it
- The vowel signs attached (sara above/below)
- The final consonant (last Thai consonant before tone/diacritic)

`kham-core/src/tcc.rs` already provides TCC (Thai Character Cluster) boundaries —
MetaSound can build on top of TCC iteration rather than reimplementing syllable parsing.

## Worked Example

Word: **กาน** /kaːn/
- Initial: ก → group `1`
- Vowel: สระ อา (long /aː/) → code `1`
- Final: น → group `2`
- MetaSound code: `112`

Word: **ขาน** /kʰaːn/
- Initial: ข → group `1` (same as ก)
- Vowel: อา → `1`
- Final: น → `2`
- MetaSound code: `112` → same as กาน ✓

Word: **กาม** /kaːm/
- Initial: ก → `1`, Vowel: อา → `1`, Final: ม → `3`
- MetaSound code: `113` → different from กาน ✓ (MetaSound distinguishes น/ม finals)

## Reference

- PyThaiNLP source: <https://github.com/PyThaiNLP/pythainlp/blob/master/pythainlp/soundex/metasound.py>
- Paper: Snae, C., & Brückner, M. (2009). WSEAS Transactions on Computers 8(5).
