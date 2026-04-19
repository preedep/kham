---
name: pos
description: Build and test Thai POS tagging in kham-core. Use when implementing the pos module, authoring pos_th.tsv, expanding POS coverage, writing POS tests, or wiring PosTag into FtsTokenizer.
metadata:
  domain: nlp
  triggers: POS, part-of-speech, PosTagger, PosTag, pos_th.tsv
  role: specialist
---

# Thai POS Tagging — kham-core

Specialist for `kham-core/src/pos` — table-driven part-of-speech tagging of segmented Thai words.

## Approach

**Table-driven only.** A context-sensitive ML tagger is a future `#[cfg(feature = "ml")]` extension.
The lookup assigns the **primary / most common POS** when a word has multiple uses in context
(e.g. ดี is `Adj` even though it sometimes modifies verbs as an adverb).

## PosTag Variants

| Variant | TSV tag | Examples |
|---------|---------|---------|
| `Noun` | `NOUN` | คน บ้าน ปลา น้ำ |
| `Verb` | `VERB` | กิน ทำ ไป ดู |
| `Adj` | `ADJ` | ดี ใหญ่ สวย ร้อน |
| `Adv` | `ADV` | มาก เร็ว เสมอ บ่อย |
| `Particle` | `PART` | ครับ ค่ะ นะ หน่อย |
| `ProperNoun` | `PROPN` | กรุงเทพ ไทย ญี่ปุ่น |
| `Pronoun` | `PRON` | ฉัน เขา เรา คุณ |
| `Numeral` | `NUM` | หนึ่ง สิบ ร้อย ล้าน |
| `Classifier` | `CLAS` | ตัว ใบ อัน ชิ้น |
| `Conjunction` | `CONJ` | และ หรือ แต่ เพราะ |
| `Auxiliary` | `AUX` | ได้ ต้อง กำลัง จะ |
| `Determiner` | `DET` | นี้ นั้น ทุก บาง |
| `Preposition` | `PREP` | ใน บน ตาม จาก |

## Module Layout

```
kham-core/
├── src/
│   └── pos.rs               # PosTag enum + PosTagger struct + impl
└── data/
    └── pos_th.tsv           # built-in POS table (hand-curated, ~230 entries)
```

## Data File Format (`pos_th.tsv`)

```tsv
# Thai word → POS tag
# Format: <thai_word><TAB><POS_TAG>
# Lines starting with # are comments; blank lines ignored
# Duplicate keys: last entry wins
กิน	VERB
ข้าว	NOUN
ดี	ADJ
และ	CONJ
ได้	AUX
นี้	DET
ใน	PREP
ครับ	PART
```

Rules:
- Tab-separated, exactly 2 columns
- Thai word is post-normalize (same form as segmenter output)
- Tag must be one of the 13 recognised strings above — unknown tags are silently skipped
- Last duplicate wins (allows domain-specific override files)
- Group entries by tag with `# ── NOUN ──` section comments for readability

## API

```rust
use kham_core::pos::{PosTag, PosTagger};

// Built-in table
let tagger = PosTagger::builtin();

// Custom / override table
let tagger = PosTagger::from_tsv("กิน\tVERB\nข้าว\tNOUN\n");

// Lookup (returns copied enum value — no lifetime)
tagger.tag("กิน")           // → Some(PosTag::Verb)
tagger.tag("xyz")           // → None

// Tag ↔ string roundtrip
PosTag::from_tag("VERB")    // → Some(PosTag::Verb)
PosTag::Verb.as_tag()       // → "VERB"

tagger.len()                // number of entries
tagger.is_empty()
```

## Integration with FtsTokenizer

`FtsToken.pos: Option<PosTag>` is populated automatically by `FtsTokenizer::segment_for_fts`.
POS is only assigned for `TokenKind::Thai` tokens — Latin, Number, etc. always get `None`.

```rust
// Default: uses PosTagger::builtin()
let fts = FtsTokenizer::new();

// Override:
let fts = FtsTokenizer::builder()
    .pos_tagger(PosTagger::from_tsv(custom_data))
    .build();

let tokens = fts.segment_for_fts("กินข้าว");
// tokens[0].text == "กิน", tokens[0].pos == Some(PosTag::Verb)
// tokens[1].text == "ข้าว", tokens[1].pos == Some(PosTag::Noun)
```

## Implementation Rules

- `no_std` / `alloc`-only — no `std` imports
- Backed by `BTreeMap<String, PosTag>` — `PosTag` is `Copy`, so `tag()` returns `Option<PosTag>` (not a reference)
- `from_tsv()` silently skips unrecognised tag strings — never panic on bad data
- Do NOT assign POS to non-Thai tokens in the FTS pipeline — check `token.kind == TokenKind::Thai` first
- Do NOT implement context-sensitive (ML) tagging — table lookup only; gate ML behind `#[cfg(feature = "ml")]`

## Expanding the Table

When adding entries to `pos_th.tsv`:
1. Find the correct section comment (`# ── NOUN ──`, etc.) and insert there
2. For ambiguous words, assign the **most frequent** POS (check TNC if unsure)
3. Run `cargo test -p kham-core` — no expected-output changes needed unless tests reference the word
4. Commit TSV and any test additions together

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| Adding POS to Latin/Number tokens | Guard with `token.kind == TokenKind::Thai` |
| Ambiguous words (ดี as adv vs adj) | Use most common POS; table can be overridden per domain |
| Unknown tag string silently dropped | Check spelling against the 13 valid tags |
| Context-sensitive tagging | Out of scope for table approach; defer to ML feature |
| Tone mark mismatch in TSV key | Always use post-normalize form (run `normalize()` first) |

## Writing Tests

```rust
// Unit tests in kham-core/src/pos.rs
#[test]
fn common_verbs() {
    let t = PosTagger::builtin();
    assert_eq!(t.tag("กิน"), Some(PosTag::Verb));
}

// Integration tests in kham-core/tests/pos.rs
#[test]
fn fts_token_has_pos_for_known_thai_word() {
    let fts = FtsTokenizer::new();
    let tokens = fts.segment_for_fts("กินข้าว");
    assert_eq!(tokens.iter().find(|t| t.text == "กิน").unwrap().pos,
               Some(PosTag::Verb));
}
```
