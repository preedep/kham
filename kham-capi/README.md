# kham-capi

C FFI bindings for the kham Thai word segmentation engine, with a cbindgen-generated header.

## Build

```bash
# Generate the header
cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham-capi/include/kham.h

# Build the shared library
cargo build -p kham-capi --release
# → target/release/libkham_capi.dylib  (macOS)
# → target/release/libkham_capi.so     (Linux)
```

## Usage

```c
#include "kham.h"

// Simple API — returns a word list
KhamTokens *t = kham_segment("กินข้าวกับปลา");
for (size_t i = 0; i < t->len; i++) {
    printf("%s\n", t->words[i]);
}
kham_tokens_free(t);

// Rich token API — returns structs with offsets, kind, and confidence
KhamTokenList *list = kham_segment_tokens("ธนาคาร100แห่ง");
for (size_t i = 0; i < list->len; i++) {
    KhamToken tok = list->tokens[i];
    printf("%s  char %zu..%zu  %s  conf=%.2f\n",
           tok.text, tok.char_start, tok.char_end, tok.kind, tok.confidence);
}
kham_token_list_free(list);
```

`KhamToken` fields: `text` (null-terminated), `byte_start`, `byte_end`, `char_start`, `char_end`, `kind` (null-terminated string), `confidence` (`float`, `0.0`–`1.0`).

## API

| Function | Description |
|---|---|
| `kham_segment(text)` | Segment text; returns `KhamTokens*` (word strings only) |
| `kham_tokens_free(t)` | Free a `KhamTokens*` |
| `kham_segment_tokens(text)` | Segment text; returns `KhamTokenList*` (rich structs with confidence) |
| `kham_token_list_free(list)` | Free a `KhamTokenList*` |
| `kham_spell_suggestions(word, max_n)` | Spell suggestions ranked by edit distance + phonetic + frequency; returns `KhamSpellList*` |
| `kham_spell_list_free(list)` | Free a `KhamSpellList*` |
| `kham_spell_did_you_mean(word)` | Single best correction; returns `char*` (empty string if already correct), caller must `free()` |
| `kham_spell_correct_text(text)` | Correct Unknown tokens in a passage; returns `char*`, caller must `free()` |
| `kham_romanize_sentence(text)` | Segment and RTGS-romanize a passage; returns `char*`, caller must `free()` |
| `kham_keywords(text, max_n)` | Top-N unigram keywords by TF × IDF-proxy; returns `KhamKeywordList*` |
| `kham_keyword_list_free(list)` | Free a `KhamKeywordList*` |
| `kham_extract_phrases(text, max_n)` | Top-N bigram/trigram keyphrases by TF × average-IDF; returns `KhamKeywordList*` |

### Spell checking

```c
// Ranked suggestions
KhamSpellList *list = kham_spell_suggestions("กีนข้าว", 5);
for (size_t i = 0; i < list->len; i++) {
    KhamSpellSuggestion s = list->suggestions[i];
    printf("%s  edit=%d  soundex=%d  freq=%zu\n",
           s.word, s.edit_distance, s.soundex_match, s.freq_score);
}
kham_spell_list_free(list);

// Single best correction (returns "" if word is already correct)
char *fix = kham_spell_did_you_mean("กีนข้าว");
if (fix && fix[0]) printf("did you mean: %s\n", fix);
free(fix);

// Correct an entire passage
char *corrected = kham_spell_correct_text("ผมกีนข้าวกับปลา");
printf("%s\n", corrected);  // ผมกินข้าวกับปลา
free(corrected);
```

### Romanization

```c
char *roman = kham_romanize_sentence("กินข้าวกับปลา");
printf("%s\n", roman);  // kin khao kap pla
free(roman);
```

### Keyword extraction

```c
// Unigram keywords
KhamKeywordList *kws = kham_keywords("นายกรัฐมนตรีประกาศนโยบายเศรษฐกิจ", 5);
for (size_t i = 0; i < kws->len; i++) {
    KhamKeyword kw = kws->keywords[i];
    printf("%s  score=%.3f  count=%zu\n", kw.word, kw.score, kw.count);
}
kham_keyword_list_free(kws);

// Bigram / trigram keyphrases (same KhamKeywordList type)
KhamKeywordList *phrases = kham_extract_phrases("การพัฒนาซอฟต์แวร์เป็นสิ่งสำคัญ", 5);
for (size_t i = 0; i < phrases->len; i++) {
    KhamKeyword p = phrases->keywords[i];
    printf("%s  score=%.3f\n", p.word, p.score);
}
kham_keyword_list_free(phrases);
```

## Link flags

```
-L target/release -lkham_capi
```

On macOS you may need `-rpath @executable_path/../lib` or similar depending on your install layout.

## Notes

- `kham_segment` / `KhamTokens` is the legacy API kept for backward compatibility. Prefer `kham_segment_tokens` for new code.
- Regenerate the header after any `#[repr(C)]` struct change in `src/lib.rs`.
