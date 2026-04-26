# kham-wasm

WebAssembly bindings for the **kham** Thai NLP engine, built with [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen).

[![npm](https://img.shields.io/npm/v/kham-wasm)](https://www.npmjs.com/package/kham-wasm)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](https://github.com/preedep/kham)

## Install

```bash
npm install kham-wasm
```

## Quick start

```js
import init, { segment, segment_fts } from "kham-wasm";

await init();

// Segmentation
const words = segment("กินข้าวกับปลา");
// ["กิน", "ข้าว", "กับ", "ปลา"]

// FTS pipeline — POS, NE, stopwords, romanization
for (const t of segment_fts("นายกรัฐมนตรีกินข้าว")) {
    console.log(t.text, t.pos, t.ne, t.is_stop, t.roman);
}

// Number conversion
number_to_thai_word(1234n);    // "หนึ่งพันสองร้อยสามสิบสี่"
thai_word_to_number("สองล้าน"); // "2000000" (empty string = not a number)
```

---

## API reference

### Segmentation

#### `segment(text: string) → string[]`

Segment Thai text and return an array of token strings. Whitespace excluded.

```js
segment("กินข้าวกับปลา");
// ["กิน", "ข้าว", "กับ", "ปลา"]
```

#### `segment_tokens(text: string) → Token[]`

Segment and return [`Token`](#token) objects with span information.

```js
for (const t of segment_tokens("ธนาคาร100แห่ง")) {
    console.log(t.text, t.char_start, t.char_end, t.kind);
}
// ธนาคาร  0   6  Thai
// 100     6   9  Number
// แห่ง    9  13  Thai
```

#### `segment_fts(text: string) → FtsToken[]`

Full NLP pipeline: normalize → segment → NE → stopwords → POS → synonyms → romanization.
Returns [`FtsToken`](#ftstoken) objects.

```js
for (const t of segment_fts("นายกรัฐมนตรีกินข้าว")) {
    console.log(`${t.text.padEnd(10)} pos=${t.pos} ne=${t.ne} stop=${t.is_stop}`);
}
```

---

### Romanization

#### `romanize(text: string) → RomanToken[]`

Segment and return each token paired with its RTGS romanization.

```js
for (const t of romanize("กินข้าว")) {
    console.log(t.text, "→", t.roman);
}
// กิน  → kin
// ข้าว → khao
```

---

### Normalization

#### `normalize(text: string) → string`

Apply two-rule Thai normalization:
1. **Duplicate tone marks** — consecutive tone marks collapsed to the last one.
2. **Sara Am composition** — nikhahit (อํ U+0E4D) + sara aa (อา) → sara am (อำ U+0E33).

```js
normalize("ข้้าว");  // "ข้าว"  (deduplicate tone)
normalize("กํา");   // "กำ"    (compose sara am)
```

---

### Sentence splitting

#### `split_sentences(text: string) → Sentence[]`

Split text into sentence spans. Boundaries: Thai markers (ฯ ๚ ๛), newlines,
`!`, `?`, `.` followed by a space.

```js
for (const s of split_sentences("กินข้าวแล้ว! ดื่มน้ำด้วย")) {
    console.log(s.text, s.char_start, s.char_end);
}
```

---

### Soundex (phonetic encoding)

#### `soundex_word(word: string, algo?: string) → string`

Encode a Thai word using a phonetic algorithm.

| `algo` | Groups | Length | Notes |
|---|---|---|---|
| `"lk82"` (default) | 12 | 4 chars | Royal Institute 1982, most common |
| `"udom83"` | 14 | 4 chars | Finer sibilant distinctions |
| `"metasound"` | — | 3 chars/syllable | Per-syllable encoding |

```js
soundex_word("กาน");              // "1600"
soundex_word("กาน", "udom83");    // "1900"
soundex_word("กาน", "metasound"); // "112"
```

#### `sounds_like(a: string, b: string, algo?: string) → boolean`

```js
sounds_like("กาน", "ขาน");             // true  (same lk82 group)
sounds_like("ลาน", "ราน", "udom83");   // false (ล/ร split in udom83)
```

#### `thai_english_soundex(word: string) → string`

Thai–English cross-language soundex (Suwanvisat & Prasitjutrakul 1998).
Accepts both Thai script and ASCII input.

```js
thai_english_soundex("Robert");  // "671763"
thai_english_soundex("โรเบิร์ต");  // same prefix as "Robert"
```

#### `sounds_like_cross_lang(a: string, b: string) → boolean`

```js
sounds_like_cross_lang("สมชาย", "Somchai");  // true
sounds_like_cross_lang("Robert", "Rupert");   // true
```

---

### Number conversion

#### `thai_digits_to_ascii(text: string) → string`

Convert Thai digit characters (๐–๙) to ASCII. Other characters unchanged.

```js
thai_digits_to_ascii("ราคา ๑๒๓ บาท");    // "ราคา 123 บาท"
thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง");  // "ธนาคาร100แห่ง"
```

#### `number_to_thai_word(n: bigint) → string`

Convert a non-negative integer to its Thai cardinal word representation.

```js
number_to_thai_word(0n);          // "ศูนย์"
number_to_thai_word(21n);         // "ยี่สิบเอ็ด"
number_to_thai_word(1_000_000n);  // "หนึ่งล้าน"
```

#### `thai_word_to_number(text: string) → string`

Parse a Thai cardinal number word. Returns `""` (empty string) for non-number input,
or the numeric value as a decimal string for valid input.

```js
thai_word_to_number("หนึ่งร้อยยี่สิบสาม");  // "123"
thai_word_to_number("สองล้าน");              // "2000000"
thai_word_to_number("กินข้าว");              // ""
```

#### `number_to_baht_text(baht: bigint, satang: number) → string`

Render a Baht amount as Thai currency text (`satang` must be 0–99).

```js
number_to_baht_text(100n, 0);   // "หนึ่งร้อยบาทถ้วน"
number_to_baht_text(21n, 50);   // "ยี่สิบเอ็ดบาทห้าสิบสตางค์"
```

#### `parse_baht_text(text: string) → BahtResult`

Parse a Thai Baht currency string. Check `result.valid` before using.

```js
const r = parse_baht_text("หนึ่งร้อยบาทถ้วน");
if (r.valid) {
    console.log(r.baht, r.satang);  // 100n  0
}
```

---

## Classes

### `Token`

| Field | Type | Description |
|---|---|---|
| `text` | `string` | Token text |
| `byte_start` / `byte_end` | `number` | UTF-8 byte offsets |
| `char_start` / `char_end` | `number` | Unicode scalar-value offsets (use for string slicing) |
| `kind` | `string` | `"Thai"` \| `"Latin"` \| `"Number"` \| `"Punctuation"` \| `"Emoji"` \| `"Whitespace"` \| `"Unknown"` \| `"Person"` \| `"Place"` \| `"Org"` |

### `FtsToken`

| Field | Type | Description |
|---|---|---|
| `text` | `string` | Token text (normalized) |
| `position` | `number` | Ordinal position in non-whitespace sequence (0-based) |
| `kind` | `string` | Same values as `Token.kind` |
| `is_stop` | `boolean` | `true` if in the built-in stopword list |
| `roman` | `string` | RTGS romanization (equals `text` for non-Thai / OOV) |
| `pos` | `string \| null` | POS tag: `"Noun"` \| `"Verb"` \| `"Adj"` \| `"Adv"` \| `"Particle"` \| `"ProperNoun"` \| `"Pronoun"` \| `"Numeral"` \| `"Classifier"` \| `"Conjunction"` \| `"Auxiliary"` \| `"Determiner"` \| `"Preposition"` |
| `ne` | `string \| null` | NE category: `"Person"` \| `"Place"` \| `"Org"` |
| `synonyms` | `string[]` | Synonym / number-normalization expansions |
| `trigrams` | `string[]` | Character trigrams (Unknown tokens only) |

### `RomanToken`

| Field | Type | Description |
|---|---|---|
| `text` | `string` | Original token text |
| `roman` | `string` | RTGS romanization |

### `Sentence`

| Field | Type | Description |
|---|---|---|
| `text` | `string` | Sentence text |
| `char_start` / `char_end` | `number` | Unicode scalar-value offsets |

### `BahtResult`

| Field | Type | Description |
|---|---|---|
| `valid` | `boolean` | `true` if the input was a valid Baht string |
| `baht` | `bigint` | Whole baht amount (only meaningful when `valid` is `true`) |
| `satang` | `number` | Satang 0–99 (only meaningful when `valid` is `true`) |

---

## Build from source

```bash
git clone https://github.com/preedep/kham
cd kham
wasm-pack build kham-wasm --target web
# → kham-wasm/pkg/
```

For Node.js:

```bash
wasm-pack build kham-wasm --target nodejs
```

## Links

- [kham.io](https://kham.io) — live demo & full documentation
- [GitHub](https://github.com/preedep/kham)
- [kham-core on crates.io](https://crates.io/crates/kham-core)
- [API reference](https://kham.io/api)
