---
name: thai-segmentation
description: Thai word segmentation domain knowledge — TCC rules, newmm algorithm, and dictionary design. Use when implementing or modifying tokenizer logic, debugging segmentation output, or working on kham-core modules (tcc, segmenter, dict, normalizer).
---

# Thai Segmentation Domain Knowledge

## Processing Pipeline Order

Always follow: Normalize → Pre-Tokenize → Segment → Post-Process.
Never skip stages. Each stage's output is the next stage's input.

## Thai Character Cluster (TCC)

TCC is the smallest unsplittable unit of Thai text. Read `references/tcc-rules.md` for the full regex patterns.

Key principles:
- A consonant + its vowels + tone mark = 1 cluster (e.g. กิ้ = 1 TCC)
- Leading vowels (เ แ โ ไ ใ) attach to the NEXT consonant
- Sara Am (ำ) is special — it's actually 2 characters (นิคหิต + สระอา)
- TCC is the FLOOR — segmenter cannot split smaller than TCC

Implementation: return `Vec<Range<usize>>` of byte positions, NOT strings.

## newmm Algorithm

Dictionary-based maximal matching constrained by TCC boundaries.

Steps:
1. Build possible-word DAG from input text using Trie lookup
2. At each position, find all dictionary words starting here
3. Also add single-TCC edges as fallback
4. Find shortest path (minimum word count) through DAG
5. Output path edges as tokens

Critical details:
- "Maximal matching" means prefer FEWER, LONGER words
- TCC constraint: word boundaries must align with TCC boundaries
- Safe mode: limit DAG graph size to avoid exponential blowup on ambiguous text
- Unknown words: consecutive non-matching TCCs should merge into one Unknown token

## Double-Array Trie (DARTS)

Use DARTS for dictionary, not HashMap or BTreeMap.
- O(key_length) lookup, cache-friendly linear memory layout
- Build once at compile time or startup, immutable after
- `include_bytes!` for built-in dict, runtime load for custom dict
- Consider `cedarwood` or `daachorse` crate, or implement from scratch

## Thai Text Normalization

Before segmentation, normalize:
- Reorder สระลอย (vowels placed before consonant in Unicode but displayed after)
- Fix duplicate วรรณยุกต์ (remove extra tone marks)
- NFC normalization
- Strip zero-width characters (ZWNJ, ZWJ) unless meaningful
- Normalize whitespace (NBSP → space, multiple spaces → single)

## Common Gotchas

- Thai UTF-8: one Thai character = 3 bytes. "กิน" = 9 bytes, not 3
- Byte spans must land on UTF-8 char boundaries — use `str::is_char_boundary()`
- สระ อำ (Sara Am, U+0E33) decomposes to นิคหิต (U+0E4D) + สระอา (U+0E32) in some normalizations
- Mixing Thai numerals (๐-๙) and Arabic numerals (0-9) — both are valid
- Tonal marks can stack: กี่ has 2 marks above กี + ่
