# kham — Python bindings

Python bindings for the **kham** Thai NLP engine, built with [PyO3](https://pyo3.rs) and [maturin](https://maturin.rs).

[![PyPI](https://img.shields.io/pypi/v/kham)](https://pypi.org/project/kham/)
[![Python](https://img.shields.io/pypi/pyversions/kham)](https://pypi.org/project/kham/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](https://github.com/preedep/kham)

## Install

```bash
pip install kham
```

## Quick start

```python
import kham

# Segmentation
kham.segment("กินข้าวกับปลา")
# ['กิน', 'ข้าว', 'กับ', 'ปลา']

# FTS pipeline — POS, NE, stopwords, romanization
for t in kham.segment_fts("นายกรัฐมนตรีกินข้าว"):
    print(t.text, t.pos, t.ne, t.is_stop, t.roman)

# Number conversion
kham.number_to_thai_word(1234)   # 'หนึ่งพันสองร้อยสามสิบสี่'
kham.thai_word_to_number("สองล้าน")  # 2000000
```

---

## API reference

### Segmentation

#### `segment(text: str) → list[str]`

Segment Thai text and return a list of token strings. Whitespace excluded.

```python
kham.segment("กินข้าวกับปลา")
# ['กิน', 'ข้าว', 'กับ', 'ปลา']
```

#### `segment_tokens(text: str) → list[Token]`

Segment and return [`Token`](#token) objects with span information.

```python
for t in kham.segment_tokens("ธนาคาร100แห่ง"):
    print(t.text, t.char_start, t.char_end, t.kind, t.confidence)
# ธนาคาร  0   6  Thai    0.95
# 100     6   9  Number  1.0
# แห่ง    9  13  Thai    1.0
```

#### `segment_above_confidence(text: str, min_confidence: float) → list[Token]`

Convenience wrapper around `segment_stream`: returns only tokens whose confidence meets the threshold. Equivalent to filtering `segment_tokens` by `t.confidence >= min_confidence` but uses the streaming iterator internally.

```python
toks = kham.segment_above_confidence("ธนาคาร100แห่ง", 0.9)
for t in toks:
    print(t.text, t.confidence)
```

#### `segment_fts(text: str) → list[FtsToken]`

Full NLP pipeline: normalize → segment → NE → stopwords → POS → synonyms → romanization.
Returns [`FtsToken`](#ftstoken) objects.

```python
for t in kham.segment_fts("นายกรัฐมนตรีกินข้าว"):
    print(f"{t.text:10} pos={t.pos!r:15} ne={t.ne!r} stop={t.is_stop}")
```

---

### Romanization

#### `romanize(text: str) → list[RomanToken]`

Segment and return each token paired with its RTGS romanization.

```python
for t in kham.romanize("กินข้าว"):
    print(t.text, "→", t.roman)
# กิน  → kin
# ข้าว → khao
```

#### `romanize_sentence(text: str) → str`

Segment and romanize the entire text in one call. Thai and Named tokens are converted to RTGS; numbers, Latin, punctuation, and whitespace pass through unchanged.

```python
kham.romanize_sentence("กินข้าวกับปลา")      # 'kin khao kap pla'
kham.romanize_sentence("กินข้าว 100 บาท")    # 'kin khao 100 baht'
kham.romanize_sentence("hello โลก")          # 'hello lok'
```

---

### Normalization

#### `normalize(text: str) → str`

Apply two-rule Thai normalization:
1. **Duplicate tone marks** — consecutive tone marks collapsed to the last one.
2. **Sara Am composition** — nikhahit (อํ U+0E4D) + sara aa (อา) → sara am (อำ U+0E33).

```python
kham.normalize("ข้้าว")               # 'ข้าว'  (deduplicate tone)
kham.normalize("กํา")  # 'กำ'   (compose sara am)
kham.normalize("กินข้าว")             # 'กินข้าว' (no change)
```

---

### Sentence splitting

#### `split_sentences(text: str) → list[Sentence]`

Split text into sentence spans. Boundaries: Thai markers (ฯ ๚ ๛), newlines,
`!`, `?`, `.` followed by a space.

```python
for s in kham.split_sentences("กินข้าวแล้ว! ดื่มน้ำด้วย"):
    print(repr(s.text), s.char_start, s.char_end)
```

---

### Spell checking

#### `spell_suggestions(word: str, max_n: int = 5) → list[SpellSuggestion]`

Return up to `max_n` spelling suggestions for `word`, ranked by edit distance then phonetic similarity then corpus frequency.

```python
for s in kham.spell_suggestions("กีนข้าว"):
    print(s.word, s.edit_distance, s.soundex_match, s.freq_score)
# กินข้าว  1  True  1342
```

#### `spell_did_you_mean(word: str) → str | None`

Return the single best correction for `word`, or `None` if the word is already in the dictionary.

```python
kham.spell_did_you_mean("กีนข้าว")  # 'กินข้าว'
kham.spell_did_you_mean("กินข้าว")  # None  (already correct)
kham.spell_did_you_mean("")          # None
```

#### `spell_correct_text(text: str) → str`

Segment `text` and replace every Unknown token (≥ 2 characters) with its best spelling correction. Known tokens pass through unchanged.

```python
kham.spell_correct_text("ผมกีนข้าวกับปลา")  # 'ผมกินข้าวกับปลา'
kham.spell_correct_text("กินข้าว")           # 'กินข้าว'  (no change)
```

---

### Keyword extraction

#### `extract_keywords(text: str, max_n: int = 10) → list[Keyword]`

Extract the top `max_n` keywords from `text` by TF × IDF-proxy score. Stopwords and single-character tokens are excluded.

```python
for kw in kham.extract_keywords("นายกรัฐมนตรีประกาศนโยบายเศรษฐกิจ", 5):
    print(kw.word, kw.score, kw.count)
```

#### `extract_phrases(text: str, max_n: int = 10) → list[Keyword]`

Extract the top `max_n` bigram and trigram keyphrases from adjacent content tokens, scored by TF × average-IDF. Returns the same `Keyword` type as `extract_keywords`.

```python
text = "การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญในยุคดิจิทัล"
for p in kham.extract_phrases(text, 5):
    print(p.word, p.score, p.count)
# การพัฒนาซอฟต์แวร์  0.842  1
```

---

### Soundex (phonetic encoding)

#### `soundex_word(word: str, algo: str = "lk82") → str`

Encode a Thai word using a phonetic algorithm.

| `algo` | Groups | Length | Notes |
|---|---|---|---|
| `"lk82"` | 12 | 4 chars | Royal Institute 1982, most common |
| `"udom83"` | 14 | 4 chars | Finer sibilant distinctions |
| `"metasound"` | — | 3 chars/syllable | Per-syllable encoding |

```python
kham.soundex_word("กาน")              # '1600'
kham.soundex_word("กาน", "udom83")    # '1900'
kham.soundex_word("กาน", "metasound") # '112'
```

#### `sounds_like(a: str, b: str, algo: str = "lk82") → bool`

```python
kham.sounds_like("กาน", "ขาน")            # True  (same lk82 group)
kham.sounds_like("ลาน", "ราน", "udom83")  # False (ล/ร split in udom83)
```

#### `thai_english_soundex(word: str) → str`

Thai–English cross-language soundex (Suwanvisat & Prasitjutrakul 1998).
Accepts both Thai script and ASCII input.

```python
kham.thai_english_soundex("Robert")  # '671763'
kham.thai_english_soundex("โรเบิร์ต")  # same prefix as "Robert"
```

#### `sounds_like_cross_lang(a: str, b: str) → bool`

```python
kham.sounds_like_cross_lang("สมชาย", "Somchai")  # True
kham.sounds_like_cross_lang("Robert", "Rupert")   # True
```

---

### Number conversion

#### `thai_digits_to_ascii(text: str) → str`

Convert Thai digit characters (๐–๙) to ASCII. Other characters unchanged.

```python
kham.thai_digits_to_ascii("ราคา ๑๒๓ บาท")     # 'ราคา 123 บาท'
kham.thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง")   # 'ธนาคาร100แห่ง'
```

#### `number_to_thai_word(n: int) → str`

Convert a non-negative integer to its Thai cardinal word representation.

```python
kham.number_to_thai_word(0)          # 'ศูนย์'
kham.number_to_thai_word(21)         # 'ยี่สิบเอ็ด'
kham.number_to_thai_word(1_000_000)  # 'หนึ่งล้าน'
```

#### `thai_word_to_number(text: str) → int | None`

Parse a Thai cardinal number word. Returns `None` for non-number input.

```python
kham.thai_word_to_number("หนึ่งร้อยยี่สิบสาม")  # 123
kham.thai_word_to_number("สองล้าน")              # 2000000
kham.thai_word_to_number("กินข้าว")              # None
```

#### `number_to_baht_text(baht: int, satang: int) → str`

Render a Baht amount as Thai currency text (`satang` must be 0–99).

```python
kham.number_to_baht_text(100, 0)   # 'หนึ่งร้อยบาทถ้วน'
kham.number_to_baht_text(21, 50)   # 'ยี่สิบเอ็ดบาทห้าสิบสตางค์'
```

#### `parse_baht_text(text: str) → BahtAmount | None`

Parse a Thai Baht currency string. Returns `None` if the string is not valid.

```python
amt = kham.parse_baht_text("หนึ่งร้อยบาทถ้วน")
if amt:
    print(amt.baht, amt.satang)  # 100  0
```

---

## Classes

### `Token`

| Attribute | Type | Description |
|---|---|---|
| `text` | `str` | Token text |
| `byte_start` / `byte_end` | `int` | UTF-8 byte offsets |
| `char_start` / `char_end` | `int` | Unicode scalar-value offsets (use for string slicing) |
| `kind` | `str` | `"Thai"` \| `"Latin"` \| `"Number"` \| `"Punctuation"` \| `"Emoji"` \| `"Whitespace"` \| `"Unknown"` \| `"Person"` \| `"Place"` \| `"Org"` |
| `confidence` | `float` | `0.0` (Unknown token) … `1.0` (unambiguous dict match) |

### `FtsToken`

| Attribute | Type | Description |
|---|---|---|
| `text` | `str` | Token text (normalized) |
| `position` | `int` | Ordinal position in non-whitespace sequence (0-based) |
| `kind` | `str` | Same values as `Token.kind` |
| `is_stop` | `bool` | `True` if in the built-in stopword list |
| `roman` | `str` | RTGS romanization (equals `text` for non-Thai / OOV) |
| `pos` | `str \| None` | POS tag: `"Noun"` \| `"Verb"` \| `"Adj"` \| `"Adv"` \| `"Particle"` \| `"ProperNoun"` \| `"Pronoun"` \| `"Numeral"` \| `"Classifier"` \| `"Conjunction"` \| `"Auxiliary"` \| `"Determiner"` \| `"Preposition"` |
| `ne` | `str \| None` | NE category: `"Person"` \| `"Place"` \| `"Org"` |
| `synonyms` | `list[str]` | Synonym / number-normalization expansions |
| `trigrams` | `list[str]` | Character trigrams (Unknown tokens only) |

### `RomanToken`

| Attribute | Type | Description |
|---|---|---|
| `text` | `str` | Original token text |
| `roman` | `str` | RTGS romanization |

### `Sentence`

| Attribute | Type | Description |
|---|---|---|
| `text` | `str` | Sentence text |
| `char_start` / `char_end` | `int` | Unicode scalar-value offsets |

### `BahtAmount`

| Attribute | Type | Description |
|---|---|---|
| `baht` | `int` | Whole baht amount |
| `satang` | `int` | Satang (0–99) |

### `SpellSuggestion`

| Attribute | Type | Description |
|---|---|---|
| `word` | `str` | Suggested word |
| `edit_distance` | `int` | Levenshtein distance from the input (1 or 2) |
| `soundex_match` | `bool` | `True` if the lk82 soundex code matches the input |
| `freq_score` | `int` | TNC corpus frequency (higher = more common) |

### `Keyword`

| Attribute | Type | Description |
|---|---|---|
| `word` | `str` | Keyword text |
| `score` | `float` | TF × IDF-proxy score |
| `count` | `int` | Occurrence count in the input text |

---

## Build from source

```bash
git clone https://github.com/preedep/kham
cd kham
pip install maturin
maturin develop -m kham-python/Cargo.toml

# Run tests
pytest kham-python/tests/ -v
```

## Links

- [kham.io](https://kham.io) — live demo & full documentation
- [GitHub](https://github.com/preedep/kham)
- [kham-core on crates.io](https://crates.io/crates/kham-core)
- [API reference](https://kham.io/api)
