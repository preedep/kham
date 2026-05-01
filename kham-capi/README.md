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

// Rich token API — returns structs with offsets and kind
KhamTokenList *list = kham_segment_tokens("ธนาคาร100แห่ง");
for (size_t i = 0; i < list->len; i++) {
    KhamToken tok = list->tokens[i];
    printf("%s  char %zu..%zu  %s\n",
           tok.text, tok.char_start, tok.char_end, tok.kind);
}
kham_token_list_free(list);
```

## API

| Function | Description |
|---|---|
| `kham_segment(text)` | Segment text; returns `KhamTokens*` (word strings only) |
| `kham_tokens_free(t)` | Free a `KhamTokens*` |
| `kham_segment_tokens(text)` | Segment text; returns `KhamTokenList*` (rich structs) |
| `kham_token_list_free(list)` | Free a `KhamTokenList*` |
| `kham_spell_suggestions(word, max_n)` | Spell suggestions ranked by edit distance + phonetic + frequency; returns `KhamSpellList*` |
| `kham_spell_list_free(list)` | Free a `KhamSpellList*` |
| `kham_keywords(text, max_n)` | Top-N keywords by TF × IDF-proxy; returns `KhamKeywordList*` |
| `kham_keyword_list_free(list)` | Free a `KhamKeywordList*` |

### Spell checking

```c
KhamSpellList *list = kham_spell_suggestions("กีนข้าว", 5);
for (size_t i = 0; i < list->len; i++) {
    KhamSpellSuggestion s = list->suggestions[i];
    printf("%s  edit=%d  soundex=%d  freq=%zu\n",
           s.word, s.edit_distance, s.soundex_match, s.freq_score);
}
kham_spell_list_free(list);
```

### Keyword extraction

```c
KhamKeywordList *kws = kham_keywords("นายกรัฐมนตรีประกาศนโยบายเศรษฐกิจ", 5);
for (size_t i = 0; i < kws->len; i++) {
    KhamKeyword kw = kws->keywords[i];
    printf("%s  score=%.3f  count=%zu\n", kw.word, kw.score, kw.count);
}
kham_keyword_list_free(kws);
```

## Link flags

```
-L target/release -lkham_capi
```

On macOS you may need `-rpath @executable_path/../lib` or similar depending on your install layout.

## Notes

- `kham_segment` / `KhamTokens` is the legacy API kept for backward compatibility. Prefer `kham_segment_tokens` for new code.
- Regenerate the header after any `#[repr(C)]` struct change in `src/lib.rs`.
