---
name: number-normalizer
description: Build and test Thai number normalization in kham-core. Use when implementing or extending number.rs, adding Thai number word coverage, debugging parse_thai_word edge cases, or integrating number normalization into FtsTokenizer synonyms.
metadata:
  domain: nlp
  triggers: number normalization, Thai digits, ๐–๙, parse_thai_word, thai_digits_to_ascii, number.rs, spelled-out numbers
  role: specialist
---

# Thai Number Normalization — kham-core

Specialist for `kham-core/src/number.rs` — Thai digit conversion and spelled-out Thai cardinal number parsing.

## Two Normalization Paths

### Path 1 — Thai digit characters → ASCII

Thai digits U+0E50–U+0E59 (๐๑๒๓๔๕๖๗๘๙) are classified as `TokenKind::Number` by `pre_tokenizer`.
They are visually distinct from ASCII `0–9` but semantically identical.

| Thai | Unicode  | ASCII |
|------|----------|-------|
| ๐    | U+0E50   | 0     |
| ๑    | U+0E51   | 1     |
| …    | …        | …     |
| ๙    | U+0E59   | 9     |

**Key functions:**
```rust
thai_digit_to_ascii(c: char) -> Option<char>    // single char
thai_digits_to_ascii(text: &str) -> String       // whole string (pass-through if no Thai digits)
is_thai_digit_str(text: &str) -> bool            // true iff every char is ๐–๙
```

### Path 2 — Spelled-out Thai cardinal words → u64

Thai writes numbers as positional words with explicit multiplier tokens.

**Digit words:** ศูนย์ หนึ่ง สอง สาม สี่ ห้า หก เจ็ด แปด เก้า

**Multipliers:** สิบ (×10) ร้อย (×100) พัน (×1k) หมื่น (×10k) แสน (×100k) ล้าน (×1M)

**Special forms:**
- `ยี่` — form of 2 used only in the tens position (`ยี่สิบ` = 20, `ยี่สิบเอ็ด` = 21)
- `เอ็ด` — form of 1 used only in the units position after `สิบ` (สิบเอ็ด = 11)
- Implied 1: bare `สิบ` = 10, bare `ร้อย` = 100, etc.

**Key functions:**
```rust
parse_thai_word(text: &str) -> Option<u64>          // None for non-number or empty
thai_word_to_decimal(text: &str) -> Option<String>  // wraps parse_thai_word → "123"
```

## Parser Architecture

`parse_thai_word` splits on `ล้าน` to separate the millions coefficient from the remainder, then delegates each half to `parse_below_lan`. This handles `สิบล้าน` (10M), `หนึ่งร้อยล้าน` (100M), etc.

`parse_below_lan` runs a linear scan using `next_num_token` (greedy prefix match against a static vocabulary table). State:
- `pending: Option<u64>` — digit awaiting its multiplier
- `had_sip: bool` — whether `สิบ` appeared (gates `เอ็ด` validity)
- All arithmetic uses `checked_add` / `checked_mul` to avoid overflow

**Validation rules enforced:**
- `เอ็ด` only valid after `สิบ` (returns `None` otherwise)
- Two consecutive digit words without a multiplier → `None`
- `ล้าน` inside `parse_below_lan` → `None` (caller handles it)

## FTS Integration

`FtsTokenizer` (in `fts.rs`) automatically normalizes numbers when `number_normalize: true` (default):

```rust
// In segment_for_fts loop:
TokenKind::Number  → thai_digits_to_ascii → add to FtsToken::synonyms if different
TokenKind::Thai    → thai_word_to_decimal → add to synonyms if Some(_)
```

Opt out: `.number_normalize(false)` on `FtsTokenizerBuilder`.

## Adding New Number Forms

1. Add the word to the `VOCAB` table in `next_num_token` — longer/more specific prefixes first.
2. Add or extend the match arm in `parse_below_lan` (or add a new outer-level split for larger units).
3. Add unit tests in `number.rs` `#[cfg(test)]` block.
4. Run `cargo test -p kham-core` and `cargo clippy`.

## Common Pitfalls

- **ยี่ without สิบ**: `ยี่` is only valid before `สิบ`. The parser treats it as digit-2, so `ยี่พัน` would currently yield `Some(2000)`. If you want to restrict it strictly to `ยี่สิบ`, add a validation pass.
- **เอ็ด without สิบ**: caught — returns `None`.
- **Overflow**: all arithmetic is `checked_*`. Very large inputs (> u64::MAX) return `None`.
- **Whitespace**: `parse_thai_word` trims leading/trailing whitespace; `parse_below_lan` does not (called internally only).
- **Non-number Thai words**: `next_num_token` returns `None` → `parse_below_lan` returns `None` → correct.

## Test Checklist

When modifying `number.rs`, verify:
- [ ] All 10 Thai digits (๐–๙) convert correctly
- [ ] Pass-through for non-Thai-digit strings (no allocation)
- [ ] `ศูนย์` = 0
- [ ] `สิบ` = 10 (implied 1), `สิบเอ็ด` = 11
- [ ] `ยี่สิบ` = 20, `ยี่สิบเอ็ด` = 21
- [ ] Hundreds, thousands, ten-thousands, hundred-thousands
- [ ] `ล้าน` splitting: `สิบล้าน`, `หนึ่งร้อยล้าน`, complex 7-digit numbers
- [ ] `เอ็ด` without `สิบ` → `None`
- [ ] Two consecutive digit words → `None`
- [ ] Empty / whitespace-only → `None`
- [ ] FTS: Thai-digit Number token has ASCII synonym
- [ ] FTS: Thai number-word token has decimal synonym
- [ ] FTS: `number_normalize(false)` suppresses synonyms
