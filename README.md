# kham

Thai word segmentation engine written in Rust. Fast, `no_std`-compatible core library with bindings for Python, WebAssembly, C, a command-line interface, and database extensions for PostgreSQL and SQLite.

[![CI](https://github.com/preedep/kham/actions/workflows/ci.yml/badge.svg)](https://github.com/preedep/kham/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/kham-core.svg)](https://crates.io/crates/kham-core)
[![PyPI](https://img.shields.io/pypi/v/kham.svg)](https://pypi.org/project/kham/)
[![npm](https://img.shields.io/npm/v/kham-wasm.svg)](https://www.npmjs.com/package/kham-wasm)

## Features

- **newmm algorithm** — DAG-based maximal matching constrained to Thai Character Cluster (TCC) boundaries
- **Multi-target** — single core library ships as a Rust crate, Python wheel, WASM module, C shared library, and CLI binary
- **Zero-copy API** — `segment()` returns `&str` slices into the original input; no heap allocation per token
- **`no_std` core** — `kham-core` compiles for bare-metal targets (`alloc` only, no `std` dependency)
- **Built-in dictionary** — 62,102-word CC0-licensed Thai word list embedded at compile time; custom dictionaries loaded at runtime
- **Compound-first DP scoring** — DP scorer prioritises fewer, longer tokens (compound preservation) over splitting into more dict matches, then uses TNC frequency as a tiebreaker; achieves 94.9% sentence-level agreement with PyThaiNLP newmm (F1 0.975)
- **Pre-compiled DARTS** — Double-Array Trie built once at compile time and loaded from a binary blob at runtime (~64 µs vs ~960 ms construction)
- **Text normalization** — วรรณยุกต์ dedup and Sara Am composition before segmentation
- **Thai FTS pipeline** — `FtsTokenizer` adds stopword filtering, synonym expansion, POS tagging, named entity recognition, RTGS romanization, and OOV n-gram fallback; ready for PostgreSQL `tsvector` and SQLite FTS5 integration
- **SQLite FTS5 extension** — loadable `libkham_sqlite` registers a `kham` tokenizer with full NLP pipeline: normalization, NE tagging, synonym expansion, RTGS romanization, and Thai phonetic soundex (lk82 by default) via `FTS5_TOKEN_COLOCATED`; `highlight()` and `snippet()` work via byte-accurate offsets; soundex algorithm selectable per-table via `tokenize='kham soundex udom83'`
- **Named entity recognition** — gazetteer-based NER with greedy multi-token matching (up to 5 consecutive tokens); ~36,600 entries covering Thai provinces, 246 countries, 17,000+ Wikipedia places/orgs, and 9,000+ person and family names
- **Part-of-speech tagging** — 13-category lookup table for Thai tokens
- **Number normalization** — Thai digit characters (๐–๙) converted to ASCII synonyms in FTS; spelled-out Thai cardinal words parsed to integers (`หนึ่งร้อย` → `100`); Thai Baht currency text parsed and generated (`parse_thai_baht` / `to_thai_baht_text`)
- **Abbreviation expansion** — `AbbrevMap` with 118-entry built-in TSV (months, era markers, ranks, agencies); greedy longest-first pre-tokenisation expansion so dot-containing forms (`ก.ค.` → `กรกฎาคม`) are replaced before segmentation; opt-in via `FtsTokenizerBuilder::abbrevs()`
- **Date parsing** — `parse_thai_date` handles 7 input formats (full month, abbreviated month, era marker, `วันที่` prefix, slash/dash-separated, Thai digits) in both Buddhist Era and Gregorian; formats back to ISO 8601 or Thai text
- **Sentence segmentation** — `split_sentences` splits Thai and mixed-script text on Thai terminators (`๚` `๛`), Paiyannoi (`ฯ`, excluding `ฯลฯ`), punctuation, and newlines with decimal- and abbreviation-aware dot rules
- **Phonetic encoding (Soundex)** — four Thai soundex algorithms: lk82 (4-char, 12 groups), udom83 (4-char, 14 groups with finer sibilant/liquid distinctions), MetaSound (per-syllable `[initial][vowel][final]`), and Thai–English cross-language Soundex (Suwanvisat & Prasitjutrakul 1998); FTS integration via `.soundex(SoundexAlgorithm)` builder option emits phonetic codes as synonyms for fuzzy name matching

## Packages

| Crate | Registry | Description |
|---|---|---|
| `kham-core` | [crates.io](https://crates.io/crates/kham-core) | Pure Rust engine, `no_std` compatible |
| `kham-cli` | [crates.io](https://crates.io/crates/kham-cli) | `kham` binary (clap) |
| `kham-python` | [PyPI](https://pypi.org/project/kham/) | Python bindings via PyO3 / maturin |
| `kham-wasm` | [npm](https://www.npmjs.com/package/kham-wasm) | WebAssembly bindings via wasm-bindgen |
| `kham-capi` | [crates.io](https://crates.io/crates/kham-capi) | C FFI with cbindgen-generated header |
| `kham-pg` | [PGXN](https://pgxn.org/dist/kham_pg/) (coming soon) | PostgreSQL extension: custom text search parser for Thai |
| `kham-sqlite` | — | SQLite loadable extension: FTS5 tokenizer for Thai |

## Quick start

### Rust

```toml
[dependencies]
kham-core = "0.4"
```

```rust
use kham_core::Tokenizer;

let tok = Tokenizer::new();
let tokens = tok.segment("กินข้าวกับปลา");
for t in &tokens {
    println!("{} ({:?})", t.text, t.kind);
}
// กิน (Thai)
// ข้าว (Thai)
// กับ (Thai)
// ปลา (Thai)
```

Mixed script works out of the box:

```rust
let tokens = tok.segment("ธนาคาร100แห่ง");
assert_eq!(tokens[0].text, "ธนาคาร"); // Thai
assert_eq!(tokens[1].text, "100");     // Number
assert_eq!(tokens[2].text, "แห่ง");   // Thai
```

### Python

```bash
pip install kham
```

```python
import kham

tokens = kham.segment("กินข้าวกับปลา")
print(tokens)  # ['กิน', 'ข้าว', 'กับ', 'ปลา']

tokens = kham.segment_tokens("ธนาคาร100แห่ง")
for t in tokens:
    print(t.text, t.char_start, t.char_end, t.kind)
# ธนาคาร  0  6  Thai
# 100     6  9  Number
# แห่ง    9  13 Thai
```

### JavaScript / TypeScript (WASM)

```bash
npm install kham-wasm
```

```js
import init, { segment, segment_tokens } from "kham-wasm";
await init();

const words = segment("กินข้าวกับปลา");
// ["กิน", "ข้าว", "กับ", "ปลา"]

const tokens = segment_tokens("ธนาคาร100แห่ง");
for (const t of tokens) {
    console.log(t.text, t.char_start, t.char_end, t.kind);
}
```

### PostgreSQL

`kham-pg` registers a custom text search parser so you can index and query Thai text with `tsvector` / `tsquery`.

```bash
make -C kham-pg regress   # build + run pg_regress in Docker (PostgreSQL 17)
make -C kham-pg install   # install locally (requires pg_config in PATH)
psql -c "CREATE EXTENSION kham_pg;"
```

```sql
-- Token types
SELECT * FROM ts_token_type('kham');
-- 1  thai    Thai word
-- 2  latin   Latin script token
-- 3  number  Numeric token
-- 4  punct   Punctuation
-- 5  emoji   Emoji token
-- 6  unknown Unknown / OOV token
-- 7  named   Named entity token (person, place, organisation)

-- Tokenise
SELECT * FROM ts_parse('kham', 'ทักษิณเดินทางไปกรุงเทพ');
-- 1  เดิน
-- 1  ทาง
-- 1  ไป
-- 7  ทักษิณ     ← Named: Person
-- 7  กรุงเทพ    ← Named: Place (merged from กรุง+เทพ by multi-token NE)

-- Build tsvector
SELECT to_tsvector('kham', 'กินข้าวกับปลา');
-- 'กิน':1 'กับ':3 'ข้าว':2 'ปลา':4

-- Search
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ข้าว ปลา');

-- GIN index
CREATE INDEX articles_fts_idx ON articles
    USING GIN (to_tsvector('kham', body));
```

> **Note:** `ts_headline` is not supported — the kham parser has no HEADLINE callback.

### SQLite

`kham-sqlite` registers a `kham` tokenizer as a loadable SQLite extension, enabling Thai full-text search with FTS5.

```bash
cargo build -p kham-sqlite --release
```

```sql
-- Load the extension
SELECT load_extension('./target/release/libkham_sqlite', 'sqlite3_kham_init');

-- Create an FTS5 virtual table
CREATE VIRTUAL TABLE articles USING fts5(title, body, tokenize='kham');

-- Insert Thai documents
INSERT INTO articles VALUES ('อาหารไทย', 'กินข้าวกับปลาและน้ำพริก');
INSERT INTO articles VALUES ('สภาพอากาศ', 'วันนี้อากาศดีมากท้องฟ้าแจ่มใส');

-- Full-text search
SELECT title FROM articles WHERE articles MATCH 'ปลา';
-- อาหารไทย

SELECT title FROM articles WHERE articles MATCH 'อากาศ';
-- สภาพอากาศ

-- RTGS romanization (built-in — no config required)
SELECT title FROM articles WHERE articles MATCH 'kin';
-- อาหารไทย  (กิน is indexed as both "กิน" and its RTGS form "kin")

-- Thai phonetic soundex — lk82 enabled by default; near-homophones share a code
SELECT title FROM articles WHERE articles MATCH '1619';  -- lk82 code for กินข้าว
-- อาหารไทย

-- Override or disable soundex per table
CREATE VIRTUAL TABLE articles2 USING fts5(title, body, tokenize='kham soundex udom83');
CREATE VIRTUAL TABLE articles3 USING fts5(title, body, tokenize='kham soundex none');

-- Snippet highlighting (byte-accurate offsets into normalized text)
SELECT snippet(articles, 1, '>>>', '<<<', '...', 6)
FROM articles WHERE articles MATCH 'ข้าว';
-- กิน>>>ข้าว<<<กับปลาและน้ำพริก
```

SQLite itself must be compiled with FTS5 support (the default in most distributions).  
On macOS, use `brew install sqlite` — the system sqlite3 binary has `load_extension` disabled.

### CLI

```bash
cargo install kham-cli
```

```bash
kham "กินข้าวกับปลา"               # กิน|ข้าว|กับ|ปลา
kham --sep " / " "สวัสดีชาวโลก"    # สวัสดี / ชาว / โลก
kham --kind "ธนาคาร100แห่ง"        # ธนาคาร:Thai|100:Number|แห่ง:Thai
kham --spans "กินข้าวกับปลา"       # กิน:0-3|ข้าว:3-7|กับ:7-10|ปลา:10-13

# FTS pipeline — kind, POS, NE, stopword, synonyms (one token per line)
kham --fts "ทักษิณเดินทางไปกรุงเทพ"
# ทักษิณ  kind=Person  pos=-     ne=Person  stop=false  syn=-
# เดิน    kind=Thai    pos=Verb  ne=-       stop=false  syn=-
# ทาง     kind=Thai    pos=Noun  ne=-       stop=true   syn=-
# ไป      kind=Thai    pos=Verb  ne=-       stop=true   syn=-
# กรุงเทพ kind=Place   pos=-     ne=Place   stop=false  syn=-

# FTS + phonetic encoding — syn= shows the lk82 code for Thai/Named tokens
kham --fts --soundex lk82 "กินข้าวกับปลา" | column -t
# กิน   kind=Thai  pos=Verb  ne=-  stop=false  syn=1600
# ข้าว  kind=Thai  pos=Noun  ne=-  stop=false  syn=1900
# กับ   kind=Thai  pos=Conj  ne=-  stop=true   syn=1400
# ปลา   kind=Thai  pos=Noun  ne=-  stop=false  syn=4800

echo "กินข้าว" | kham           # stdin
RUST_LOG=debug kham "กินข้าว"  # per-token trace + timing
```

### C

```c
#include "kham.h"

KhamTokens *t = kham_segment("กินข้าวกับปลา");
for (size_t i = 0; i < t->len; i++) printf("%s\n", t->words[i]);
kham_tokens_free(t);

// Rich token structs
KhamTokenList *list = kham_segment_tokens("ธนาคาร100แห่ง");
for (size_t i = 0; i < list->len; i++) {
    KhamToken tok = list->tokens[i];
    printf("%s  char %zu..%zu  %s\n", tok.text, tok.char_start, tok.char_end, tok.kind);
}
kham_token_list_free(list);
```

Generate the header:

```bash
cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham-capi/include/kham.h
cargo build -p kham-capi --release
```

## Token contract

```rust
pub struct Token<'a> {
    pub text: &'a str,            // zero-copy slice of the input string
    pub span: Range<usize>,       // byte offsets in the original string
    pub char_span: Range<usize>,  // Unicode scalar-value (char) offsets
    pub kind: TokenKind,          // Thai | Latin | Number | Punctuation | Emoji | Whitespace | Unknown | Named(NamedEntityKind)
}
```

- `span` — byte offsets; slice with `&input[token.span.clone()]`
- `char_span` — Unicode scalar-value offsets for Python/JavaScript indexing
- Joining all `token.text` values (whitespace kept) reconstructs the original input exactly

## Full-Text Search

`FtsTokenizer` wraps the segmenter with the full NLP pipeline:

```rust
use kham_core::fts::FtsTokenizer;

let fts = FtsTokenizer::new();

// All tokens with metadata
let tokens = fts.segment_for_fts("ทักษิณเดินทางไปกรุงเทพ");
for t in &tokens {
    println!("{} ne={:?} pos={:?} stop={}", t.text, t.ne, t.pos, t.is_stop);
}
// ทักษิณ  ne=Some(Person)  pos=None    stop=false
// เดิน    ne=None          pos=Verb    stop=false
// ทาง     ne=None          pos=None    stop=true
// ไป      ne=None          pos=Verb    stop=true
// กรุงเทพ ne=Some(Place)   pos=None    stop=false  ← merged from กรุง+เทพ

// Flat lexeme list for tsvector (stopwords removed)
let lexemes = fts.lexemes("กินข้าวกับปลา");
// → ["กิน", "ข้าว", "ปลา"]
```

Builder options:

```rust
use kham_core::fts::FtsTokenizer;
use kham_core::abbrev::AbbrevMap;
use kham_core::synonym::SynonymMap;
use kham_core::stopwords::StopwordSet;
use kham_core::romanizer::RomanizationMap;

let fts = FtsTokenizer::builder()
    .abbrevs(AbbrevMap::builtin())            // ก.ค. → กรกฎาคม before segmentation
    .synonyms(SynonymMap::from_tsv(include_str!("synonyms.tsv")))
    .stopwords(StopwordSet::from_text("ซื้อ\nขาย\n"))
    .romanization(RomanizationMap::builtin()) // adds RTGS to synonyms: กิน → "kin"
    .soundex(SoundexAlgorithm::Lk82)         // adds lk82 code to synonyms for Thai/Named tokens
    .ngram_size(3)                            // trigrams for Unknown tokens (0 = disable)
    .number_normalize(true)                   // Thai digits → ASCII synonym (default: true)
    .build();
```

`FtsToken` fields: `text`, `position`, `kind`, `is_stop`, `synonyms`, `trigrams`, `pos`, `ne`.

## Number normalization

`kham-core` provides three number utilities in `kham_core::number`:

```rust
use kham_core::number::{
    thai_digits_to_ascii, parse_thai_word, u64_to_thai_word,
    parse_thai_baht, to_thai_baht_text, BahtAmount,
};

// Thai digit characters → ASCII
thai_digits_to_ascii("๑๒๓")              // "123"
thai_digits_to_ascii("ธนาคาร๑๐๐แห่ง")   // "ธนาคาร100แห่ง"

// Spelled-out Thai number words ↔ integer (fully round-trips)
parse_thai_word("หนึ่งร้อยยี่สิบสาม")   // Some(123)
parse_thai_word("สิบล้าน")              // Some(10_000_000)
u64_to_thai_word(123)                  // "หนึ่งร้อยยี่สิบสาม"
u64_to_thai_word(10_000_000)           // "สิบล้าน"

// Thai Baht currency text ↔ BahtAmount (fully round-trips)
parse_thai_baht("หนึ่งร้อยบาทห้าสิบสตางค์")
// Some(BahtAmount { baht: 100, satang: 50 })

to_thai_baht_text(100, 50)   // "หนึ่งร้อยบาทห้าสิบสตางค์"
to_thai_baht_text(100, 0)    // "หนึ่งร้อยบาทถ้วน"
```

In `FtsTokenizer`, number normalization runs automatically: `TokenKind::Number` tokens with Thai digits get their ASCII form added to `synonyms` (so `123` matches `๑๒๓` in search), and Thai number-word tokens get their decimal string added to `synonyms`. Opt out with `.number_normalize(false)`.

## Abbreviation expansion

`kham_core::abbrev::AbbrevMap` expands Thai abbreviations before segmentation so dot-containing patterns are consumed as single units rather than fragmenting at each dot.

```rust
use kham_core::abbrev::AbbrevMap;

let map = AbbrevMap::builtin();

// Pre-tokenisation: replace abbreviated forms in running text
assert_eq!(map.expand_text("วันที่5ก.ค.2567"), "วันที่5กรกฎาคม2567");
assert_eq!(map.expand_text("พ.ศ.2567"),        "พุทธศักราช2567");

// Post-tokenisation: look up a single already-segmented token
let exps = map.lookup("ดร.").unwrap();
assert_eq!(exps, &["ดอกเตอร์"]);
```

The built-in TSV (118 entries) covers all 12 month abbreviations, era markers (`พ.ศ.`, `ค.ศ.`, `ก่อน ค.ศ.`), military ranks, police ranks, government agencies, and Bangkok districts. Ambiguous abbreviations (e.g. `อ.` → อาจารย์ / อำเภอ) return all expansions from `lookup`; `expand_text` uses the primary (first) expansion.

Use with `FtsTokenizer` via `FtsTokenizerBuilder::abbrevs(AbbrevMap::builtin())` — disabled by default.

## Date parsing

`kham_core::date::parse_thai_date` parses Thai date strings in Buddhist Era or Gregorian and formats them back to ISO 8601 or Thai text.

```rust
use kham_core::date::{parse_thai_date, Era};

// Full month name (Buddhist Era inferred from year ≥ 2300)
let d = parse_thai_date("5 กรกฎาคม 2567").unwrap();
assert_eq!(d.day, 5);
assert_eq!(d.month, 7);
assert_eq!(d.to_iso8601(), "2024-07-05"); // BE 2567 → CE 2024

// Abbreviated month with era marker
let d = parse_thai_date("5 ก.ค. พ.ศ. 2567").unwrap();
assert_eq!(d.to_thai_text(), "5 กรกฎาคม พ.ศ. 2567");

// Thai digits
let d = parse_thai_date("๕ ก.ค. ๒๕๖๗").unwrap();
assert_eq!(d.to_iso8601(), "2024-07-05");

// Slash / dash separated
let d = parse_thai_date("5/7/2567").unwrap();
assert_eq!(d.era, Era::Buddhist);
```

Supported formats: full month name, abbreviated month (e.g. `ก.ค.`), explicit era marker (`พ.ศ.` / `ค.ศ.`), `วันที่` prefix, slash-separated, dash-separated, Thai digits. Era is inferred when omitted: year ≥ 2300 → Buddhist Era.

## Sentence segmentation

`kham_core::sentence::split_sentences` splits Thai and mixed-script text into sentences.

```rust
use kham_core::sentence::split_sentences;

let text = "สวัสดีครับ! วันนี้อากาศดีมาก\nเราไปกินข้าวกันเถอะ";
let sents = split_sentences(text);
assert_eq!(sents.len(), 3);
assert_eq!(sents[0].text, "สวัสดีครับ!");
assert_eq!(sents[1].text, "วันนี้อากาศดีมาก");
assert_eq!(sents[2].text, "เราไปกินข้าวกันเถอะ");
```

Split delimiters and their rules:

| Character | Rule |
|---|---|
| `๚` `๛` | Always splits |
| `ฯ` | Splits unless part of `ฯลฯ` |
| `\n` | Always splits |
| `!` `?` | Always splits |
| `.` | Splits only when followed by whitespace or end-of-string (not in `3.14`, `ก.ค.`, `A.B.C.`) |

Each `Sentence` carries `text: &str`, `span: Range<usize>` (byte offsets), and `char_span: Range<usize>`.

## Named entity recognition

The built-in gazetteer (~10,400 entries) covers:

| Category | Coverage |
|---|---|
| Place | Thai provinces (77), full country list (246), world cities, regions |
| Person | 10,000+ Thai given names filtered against the dictionary to reduce false positives |
| Org | Thai government ministries, state enterprises, banks, universities, international orgs |

Multi-token matching merges compound names split by the segmenter:

```
กรุงเทพ  → segmenter splits → กรุง + เทพ
         → NE tagger merges → กรุงเทพ  Named(Place)

กนกวรรณ  → segmenter splits → กนก + วร + รณ
         → NE tagger merges → กนกวรรณ  Named(Person)
```

See [ADR-001](doc/adr-001-ne-person-name-import-strategy.md) for the person-name import decision.

## Phonetic encoding (Soundex)

`kham_core::soundex` provides four Thai phonetic encoding algorithms for fuzzy name matching and spell-correction:

```rust
use kham_core::soundex::{soundex, sounds_like, SoundexAlgorithm};
use kham_core::soundex::{lk82, udom83, metasound};
use kham_core::soundex::{thai_english_soundex, sounds_like_cross_lang};

// lk82 — 4-char code, 12 consonant groups (most widely used)
assert_eq!(lk82("กาน"), lk82("ขาน")); // ก and ข in the same group → "1600"
assert_eq!(lk82("กาน"), "1600");

// udom83 — finer sibilant/liquid distinctions
assert_ne!(udom83("ลาน"), udom83("ราน")); // ล and ร are split in udom83
assert_ne!(udom83("สาน"), udom83("ชาน")); // sibilant ≠ affricate

// MetaSound — 3 chars per syllable: [initial][vowel][final]
assert_eq!(metasound("กาน"), "112"); // initial=ก(1) vowel=า(1) final=น(2)
assert_ne!(metasound("กาน"), metasound("กาม")); // different final

// Unified API
assert!(sounds_like("กาน", "คาน", SoundexAlgorithm::Lk82));

// Thai–English cross-language (Suwanvisat & Prasitjutrakul 1998)
// — encodes Thai and English to a shared code space without a romanizer
assert_eq!(thai_english_soundex("Robert"), thai_english_soundex("Rupert"));
// Thai transliteration and English source share a common prefix
let en = thai_english_soundex("McDonald");
let th = thai_english_soundex("แมคโดนัลด์");
assert_eq!(&en[..3], &th[..3]); // "523"
```

FTS integration — emit the soundex code as a synonym alongside RTGS romanization:

```rust
use kham_core::soundex::SoundexAlgorithm;

let fts = FtsTokenizer::builder()
    .soundex(SoundexAlgorithm::Lk82) // adds lk82 code to FtsToken::synonyms
    .build();
// Searching the lk82 code matches all words in the same phonetic group
```

## Building

```bash
cargo build                          # all crates (also runs build.rs → dict.bin)
cargo test --release                 # all tests
cargo test -p kham-core --release    # core only
cargo bench -p kham-core             # core criterion benchmarks
cargo bench -p kham-sqlite           # SQLite FTS5 criterion benchmarks
cargo run -p kham-bench-accuracy     # word-boundary P/R/F1 accuracy benchmark
cargo run -p kham-bench-accuracy -- --threshold 0.95  # exit 1 if F1 < threshold

# Bindings
wasm-pack build kham-wasm --target web
maturin develop -m kham-python/Cargo.toml
make -C kham-pg regress              # PostgreSQL: Docker pg_regress
cargo build -p kham-sqlite --release # SQLite: build libkham_sqlite.dylib/.so
```

Prerequisites per target:

| Target | Tool | Install |
|---|---|---|
| All | Rust ≥ 1.85 | `curl -sSf https://sh.rustup.rs \| sh` |
| WASM | `wasm-pack` | `cargo install wasm-pack` |
| Python | `maturin` | `pip install maturin` |
| C | `cbindgen` | `cargo install cbindgen` |
| PostgreSQL | Docker with BuildKit | [docs.docker.com](https://docs.docker.com/engine/install/) |
| PostgreSQL (local) | `pg_config`, C compiler, `gettext` (macOS) | `brew install postgresql@17 gettext` |
| SQLite (macOS) | Xcode CLT or Homebrew sqlite | `xcode-select --install` or `brew install sqlite` |
| SQLite (Linux) | SQLite development headers | `apt install libsqlite3-dev` |

## CI

| Job | What it checks |
|---|---|
| `fmt` | `cargo fmt --check` |
| `clippy` | `cargo clippy -D warnings` |
| `test` | Unit + integration + doc tests, stable and MSRV 1.85, Linux and macOS |
| `no_std` | `kham-core` compiles for `thumbv7em-none-eabihf` |
| `wasm` | `wasm-pack build --target web` succeeds |
| `python` | `maturin develop` on Python 3.8 and 3.12 |
| `pg_regress` | 67 SQL tests across 4 suites in Docker PostgreSQL 17 |

## Further reading

| Document | Contents |
|---|---|
| [doc/roadmap.md](doc/roadmap.md) | Release history, pending action checklist, PyThaiNLP corpus import plan |
| [doc/architecture.md](doc/architecture.md) | Crate graph, pipeline flowcharts, module responsibilities (Mermaid) |
| [doc/benchmarks.md](doc/benchmarks.md) | Throughput numbers, dict construction, PostgreSQL and SQLite FTS5 benchmarks |
| [doc/dict-format.md](doc/dict-format.md) | `dict.bin` binary format, DARTS lifecycle, data sources |
| [doc/adr-001-ne-person-name-import-strategy.md](doc/adr-001-ne-person-name-import-strategy.md) | Why person names are filtered against `words_th.txt` |
| [doc/adr-002-syllables-corpus-import-decision.md](doc/adr-002-syllables-corpus-import-decision.md) | Why syllables_th.txt syllables and abbreviations are not imported |
| [doc/adr-003-orchid-pos-tag-mapping.md](doc/adr-003-orchid-pos-tag-mapping.md) | ORCHID 44-tag → kham-core 13-category POS mapping |

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
