# kham-core

Pure Rust, `no_std` / `alloc`-only segmentation and FTS library. All modules live under `src/`.

## Modules

| Module | Purpose |
|---|---|
| `normalizer` | Thai text normalization (สระลอย reorder, วรรณยุกต์, NFC) |
| `number` | Number normalization: Thai digits (๐–๙) → ASCII; spelled-out Thai number words → integer |
| `pre_tokenizer` | Unicode script classification (Thai / Latin / Number / Emoji / URL) |
| `tcc` | Thai Character Cluster boundaries (Theeramunkong et al. 2000) |
| `dict` | Double-Array Trie (DARTS), built-in `words_th.txt` via `include_bytes!` |
| `freq` | TNC frequency table (`tnc_freq.txt`), `FreqMap` used by DP scorer |
| `segmenter` | DAG-based maximal matching (newmm algorithm) |
| `token` | `Token` struct with text, byte span, char span, `TokenKind` |
| `stopwords` | `StopwordSet`: sorted `Vec<String>`, binary-search lookup |
| `synonym` | `SynonymMap`: `BTreeMap<canonical → Vec<synonym>>` from TSV |
| `ngram` | `char_ngrams` / `token_ngrams` — OOV fallback indexing |
| `pos` | `PosTagger` / `PosTag` — table-driven POS tagging (13 categories) |
| `ne` | `NeTagger` / `NamedEntityKind` — gazetteer-based NER |
| `fts` | `FtsTokenizer` / `FtsToken` — full pipeline for PostgreSQL FTS |
| `romanizer` | `RomanizationMap` — RTGS table-lookup Thai → Latin |
| `soundex` | Thai phonetic encoding — lk82, udom83, MetaSound, Thai–English cross-language |

## Dictionary

- Built-in: `words_th.txt` (Apache-2.0, PyThaiNLP) embedded at compile time via `include_bytes!`
- Custom dict: `Tokenizer::builder().dict_file("path")`
- Trie: Double-Array Trie, O(n) lookup — no external trie-building utilities
- Never ship BEST corpus or non-CC0 data
- Frequency data: `tnc_freq.txt` (Apache-2.0, PyThaiNLP) embedded separately — loaded into `FreqMap`, used by newmm DP scorer as tiebreaker; do not merge into the trie binary
- Stopword data: `stopwords_th.txt` (Apache-2.0, PyThaiNLP) — attribution header must be kept

## Segmenter DP Scoring

The newmm forward DP uses a 4-field lexicographic `DpScore`. Priority order is fixed — do not reorder:

1. **Minimise unknowns** (`neg_unknowns`) — primary criterion
2. **Minimise token count** (`neg_tokens`) — prefer fewer, longer compounds (matches PyThaiNLP newmm)
3. **Maximise dict matches** (`dict_words`)
4. **Maximise TNC frequency** (`freq_score`) — final tiebreaker; unknown edges contribute 0

The `Ord` derive compares fields in declaration order — insert new dimensions at the correct priority position.

**Rationale for priority 2:** Splitting a compound into two known words scores *more* dict matches than keeping it as one compound. Placing token-count minimisation above dict-match maximisation prevents spurious splits and aligns kham with PyThaiNLP's newmm behaviour (measured: 94.9% sentence-level agreement, F1 0.975).

## FTS Modules

All FTS modules are `no_std` / `alloc`-only. Pipeline order: normalize → segment → NE tag → stopword tag → POS tag → synonym expand → OOV trigrams.

### `stopwords` — `StopwordSet`

```rust
StopwordSet::builtin()               // 1 029-word built-in list (PyThaiNLP Apache-2.0)
StopwordSet::from_text(data: &str)   // newline-separated; # lines ignored; BOM stripped
set.contains(word: &str) -> bool     // O(log n) binary search
set.len() -> usize
```

Data: `kham-core/data/stopwords_th.txt` — sorted, deduplicated, UTF-8, BOM-stripped. Attribution header must be preserved.

### `synonym` — `SynonymMap`

TSV format: `canonical<TAB>syn1<TAB>syn2<TAB>…` (`#` lines ignored).

```rust
SynonymMap::empty()
SynonymMap::from_tsv(data: &str)              // duplicate canonicals merge (extend, not replace)
map.expand(word: &str) -> Option<&[String]>
map.has_synonyms(word: &str) -> bool
```

### `ngram`

```rust
char_ngrams(text: &str, n: usize) -> impl Iterator<Item = &str>   // zero-alloc &str slices
token_ngrams(tokens: &[&str], n: usize) -> impl Iterator<Item = String>
```

**Unknown-token constraint:** newmm emits Unknown tokens one TCC at a time. Bare consonants (1 char) produce no grams when `n ≥ 2` — single Thai chars are morphemically atomic.

### `fts` — `FtsTokenizer` / `FtsToken`

```rust
pub struct FtsToken {
    pub text: String,
    pub position: usize,               // ordinal index in non-whitespace sequence (0-based)
    pub kind: TokenKind,
    pub is_stop: bool,
    pub synonyms: Vec<String>,
    pub trigrams: Vec<String>,         // populated for Unknown tokens only
    pub pos: Option<PosTag>,           // Thai tokens only; None for OOV / non-Thai
    pub ne: Option<NamedEntityKind>,   // Some(k) iff kind == TokenKind::Named(k)
}

FtsTokenizer::new()
FtsTokenizer::builder()
    .stopwords(StopwordSet)
    .synonyms(SynonymMap)
    .ngram_size(usize)                 // 0 = disable
    .pos_tagger(PosTagger)
    .ne_tagger(NeTagger)
    .romanization(RomanizationMap)     // adds RTGS forms to synonyms
    .build()

fts.segment_for_fts(text) -> Vec<FtsToken>   // all non-whitespace tokens with metadata
fts.index_tokens(text)    -> Vec<FtsToken>   // stopword positions preserved
fts.lexemes(text)         -> Vec<String>     // text + synonyms + trigrams — used by kham-pg
```

**Implementation rules:**
- `segment_for_fts` calls `normalize()` internally — callers do not normalise first
- Stopword positions are preserved in `index_tokens` so phrase-distance scoring is correct
- `trigrams` only for `TokenKind::Unknown`; Thai tokens never receive trigrams
- `pos` only for `TokenKind::Thai` — Named, Latin, Number always get `None`
- `ne` only for `TokenKind::Named(k)` — corresponds 1:1 with `kind`
- NE tagging runs before POS tagging; Named tokens skip POS lookup
- No `std`-only code — gate FST support behind `#[cfg(feature = "std")]` if ever needed

### `number` — number normalization

```rust
// Thai digit → ASCII
thai_digit_to_ascii(c: char) -> Option<char>       // single char; None for non-Thai digits
thai_digits_to_ascii(text: &str) -> String          // whole string; pass-through if no Thai digits
is_thai_digit_str(text: &str) -> bool               // true iff all chars are ๐–๙

// Spelled-out Thai number word → integer
parse_thai_word(text: &str) -> Option<u64>          // None for non-number input or empty
thai_word_to_decimal(text: &str) -> Option<String>  // convenience: Some("123") or None
```

**FTS integration** (automatic, opt-out with `.number_normalize(false)`):
- `TokenKind::Number` tokens containing Thai digits get their ASCII form in `FtsToken::synonyms`.
- `TokenKind::Thai` tokens recognised as number words get their decimal string in `synonyms`.

Supported range: 0 (`ศูนย์`) through multi-billion values (`u64`). Special forms: `ยี่` (20-prefix), `เอ็ด` (units-1 after สิบ), implied-1 for bare multipliers (`ร้อย` = 100).

### `pos` — `PosTagger` / `PosTag`

**13 variants:** `Noun Verb Adj Adv Particle ProperNoun Pronoun Numeral Classifier Conjunction Auxiliary Determiner Preposition`

**TSV tags:** `NOUN VERB ADJ ADV PART PROPN PRON NUM CLAS CONJ AUX DET PREP`

```rust
PosTagger::builtin()                        // ~230 entries, hand-curated
PosTagger::from_tsv(data: &str)            // last duplicate wins; unknown tags skipped
tagger.tag(word: &str) -> Option<PosTag>   // None if OOV; Copy

PosTag::from_tag("VERB") -> Option<PosTag>
PosTag::Verb.as_tag() -> &'static str      // "VERB"
PosTag::Verb.as_str() -> &'static str      // "Verb"
```

Data: `kham-core/data/pos_th.tsv` — sections grouped by tag with `# ── NOUN ──` comments.

### `ne` — `NeTagger` / `NamedEntityKind`

**Three categories:** `Person Place Org` — **TSV tags:** `PERSON PLACE ORG`

`NamedEntityKind` is defined in `token.rs` (not `ne.rs`) to avoid circular imports.

```rust
NeTagger::builtin()                         // ~400 entries: provinces, countries (full list), cities, orgs
NeTagger::from_tsv(data: &str)             // last duplicate wins
tagger.tag(word: &str) -> Option<NamedEntityKind>
tagger.tag_tokens(tokens: Vec<Token>, source: &str) -> Vec<Token>
//   greedy longest-match up to 5 consecutive Thai tokens; merges split compound names

NamedEntityKind::from_tag("PLACE") -> Option<NamedEntityKind>
NamedEntityKind::Place.as_tag() -> &'static str   // "PLACE"
NamedEntityKind::Place.as_str() -> &'static str   // "Place"
```

**Multi-token matching:** `tag_tokens` tries spans of 5→1 consecutive Thai tokens; first gazetteer hit wins. Compound names split by the segmenter (e.g. กรุง+เทพ → กรุงเทพ) are merged into one `Named` token with combined spans. TSV entries must match the segmenter's concatenated output exactly — verify with `Tokenizer::new().segment("candidate")`.

Data: `kham-core/data/ne_th.tsv` — Thai provinces (77), full country list from PyThaiNLP (~246, Apache-2.0), world cities, regions, organisations, universities, public figures (~400 total).

### `soundex` — Thai phonetic encoding

Four algorithms, all pure-Rust `no_std`; no data file (tables are inline `match` expressions).

```rust
use kham_core::soundex::{soundex, sounds_like, SoundexAlgorithm};
use kham_core::soundex::{lk82, udom83, metasound};
use kham_core::soundex::{thai_english_soundex, english_soundex, sounds_like_cross_lang};

// Unified enum API (lk82 / udom83 / MetaSound)
soundex("กาน", SoundexAlgorithm::Lk82)      // "1600" — always 4 chars
soundex("กาน", SoundexAlgorithm::Udom83)    // "1900" — always 4 chars
soundex("กาน", SoundexAlgorithm::MetaSound) // "112"  — 3 chars per syllable

sounds_like("กาน", "ขาน", SoundexAlgorithm::Lk82)    // true — same group
sounds_like("ลาน", "ราน", SoundexAlgorithm::Udom83)   // false — ล/ร split in udom83

// Direct functions
lk82("กรุงเทพ")   // "1873"
udom83("สาน")    // differs from udom83("ชาน") — sibilant ≠ affricate
metasound("กาน") // "112": initial=ก(1) vowel=า(1) final=น(2)

// Thai–English cross-language (Suwanvisat & Prasitjutrakul 1998)
// — direct Thai+English table, no romanizer; vowels→'7', ง→"52", variable length
thai_english_soundex("Robert")     // "671763"
thai_english_soundex("กิน")        // "25"  (ก→2, น→5; vowel ิ skipped)
sounds_like_cross_lang("Robert", "Rupert")  // true
```

**FTS integration** — `.soundex(SoundexAlgorithm)` builder appends the code to
`FtsToken::synonyms` for Thai and Named tokens (not set by default; only lk82/udom83
recommended — MetaSound is variable-length and collision-prone at word level):

```rust
let fts = FtsTokenizer::builder()
    .soundex(SoundexAlgorithm::Lk82)
    .build();
// กิน → synonyms includes its lk82 code; fuzzy FTS matches near-homophones
```

**Algorithm summary:**

| Function | Output length | Notes |
|---|---|---|
| `lk82` | 4 chars | 12 groups; most widely used in Thai NLP |
| `udom83` | 4 chars | 14 groups; finer sibilant/liquid distinctions |
| `metasound` | 3 chars/syllable | Per-syllable `[initial][vowel][final]` |
| `thai_english_soundex` | variable | Paper (1998) combined Thai+English table |
| `english_soundex` | 4 chars | Standard Odell–Russell Soundex |

### `romanizer` — `RomanizationMap`

Table-lookup RTGS romanization. No rule-based phonetic engine — table only. Rule-based can be a future `#[cfg(feature = "phonetic")]` extension.

```rust
RomanizationMap::builtin()                   // embedded via include_str!
RomanizationMap::from_tsv(data: &str)       // last duplicate wins
map.romanize(word: &str) -> Option<&str>    // zero-copy borrow from map
map.romanize_or_raw(word: &str) -> &str
map.romanize_tokens(tokens: &[&str]) -> Vec<String>
```

Data: `kham-core/data/romanization_th.tsv` — hand-curated, NOT auto-generated. To add entries: edit TSV → `cargo test -p kham-core` → commit TSV alongside API changes.

## Testing

- Unit tests co-located in each module
- Integration tests in `kham-core/tests/` with real Thai text
- Test data: `kham-core/testdata/` — format `input|tok1|tok2|…` (one case per line; `#` = comment; whitespace tokens excluded)
  - `basic.txt` — pure Thai; all tokens must be `TokenKind::Thai`
  - `mixed_script.txt` — Thai + Latin + Number
  - `normalization.txt` — asserts `normalize()` is idempotent then segments correctly
- Edge cases to always test: สระลอย, วรรณยุกต์ซ้อน, zero-width chars, `ธนาคาร100แห่ง`, empty string, single char

## Performance

Criterion benchmarks cover dict construction/lookup, segmentation throughput (MB/s by length), mixed-script overhead, and FTS pipeline cost. Run with `cargo bench`. Benchmark every PR touching segmenter or dict — compare against previous baseline before merging.
