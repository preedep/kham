---
name: thai-test-cases
description: Write and validate test cases for Thai word segmentation. Use when creating tests, adding edge cases, debugging incorrect segmentation, or writing integration tests for kham-core.
---

# Thai Test Case Patterns

## Test File Format

Test data lives in `testdata/` as `.txt` files, one case per line:

```
input|expected_word_1|expected_word_2|...
```

Example:
```
ฉันกินข้าว|ฉัน|กิน|ข้าว
ธนาคารแห่งประเทศไทย|ธนาคาร|แห่ง|ประเทศไทย
```

## Rust Test Pattern

```rust
#[test]
fn test_basic_segmentation() {
    let tok = Tokenizer::default();
    let tokens = tok.segment("ฉันกินข้าว");
    let words: Vec<&str> = tokens.iter().map(|t| t.text).collect();
    assert_eq!(words, vec!["ฉัน", "กิน", "ข้าว"]);
}
```

Always also verify spans:

```rust
#[test]
fn test_spans_are_valid() {
    let input = "ฉันกินข้าว";
    let tok = Tokenizer::default();
    for token in tok.segment(input) {
        // Span must be valid UTF-8 boundary
        assert!(input.is_char_boundary(token.span.start));
        assert!(input.is_char_boundary(token.span.end));
        // Span must reconstruct the text
        assert_eq!(&input[token.span.clone()], token.text);
    }
}
```

## Mandatory Edge Cases

Always include tests for these categories. Read `references/edge-cases.md` for full catalog.

1. **Empty / whitespace**: `""`, `" "`, `"  \t\n  "`
2. **Single character**: `"ก"`, `"a"`, `"1"`
3. **Mixed script**: `"ธนาคาร100แห่ง"`, `"hello สวัสดี world"`
4. **สระลอย**: `"เขากิน"` — เ belongs to ข not standalone
5. **Ambiguous**: `"ตากลม"` — ตา+กลม vs ตาก+ลม
6. **OOV (out-of-vocab)**: words not in dictionary
7. **URL/email**: `"ดูที่https://example.com/path"`
8. **Emoji**: `"สนุก😄มาก"`
9. **Repeated chars**: `"555555"`, `"กกกกก"`
10. **Thai digits**: `"ราคา ๑๒๓ บาท"`

## Byte Length Reminder

Thai chars are 3 bytes each in UTF-8:
- `"กิน"` = 9 bytes (ก=3, อิ=3, น=3)
- `"ก"` = 3 bytes, span = 0..3
- `"a"` = 1 byte, span = 0..1

Never hardcode byte offsets in tests — compute from `&str` methods.

## Gotchas

- `assert_eq!` on `Vec<&str>` works well for readability
- Use `#[ignore]` for known-failing ambiguity cases, with TODO
- Benchmark tests go in `benches/`, never in `#[test]`
- Long Thai text (1000+ words) should be a separate integration test, not unit test
