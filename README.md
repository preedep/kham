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
- **TNC frequency scoring** — Thai National Corpus (CC0) raw counts guide the DP scorer to prefer statistically common segmentations
- **Pre-compiled DARTS** — Double-Array Trie built once at compile time and loaded from a binary blob at runtime (~64 µs vs ~960 ms construction)
- **Text normalization** — วรรณยุกต์ dedup and Sara Am composition before segmentation
- **Thai FTS pipeline** — `FtsTokenizer` adds stopword filtering, synonym expansion, POS tagging, named entity recognition, RTGS romanization, and OOV n-gram fallback; ready for PostgreSQL `tsvector` and SQLite FTS5 integration
- **SQLite FTS5 extension** — loadable `libkham_sqlite` registers a `kham` tokenizer with full NLP pipeline: normalization, NE tagging, synonym expansion, and RTGS romanization via `FTS5_TOKEN_COLOCATED`; `highlight()` and `snippet()` work via byte-accurate offsets into normalized text
- **Named entity recognition** — gazetteer-based NER with greedy multi-token matching (up to 5 consecutive tokens); ~10,400 entries covering Thai provinces, 246 countries, and 10,000+ person names
- **Part-of-speech tagging** — 13-category lookup table for Thai tokens

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
kham-core = "0.1"
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

# FTS pipeline — kind, POS, NE, stopword (one token per line)
kham --fts "ทักษิณเดินทางไปกรุงเทพ"
# ทักษิณ  kind=Named  pos=-     ne=Person  stop=false
# เดิน    kind=Thai   pos=Verb  ne=-       stop=false
# ทาง     kind=Thai   pos=-     ne=-       stop=true
# ไป      kind=Thai   pos=Verb  ne=-       stop=true
# กรุงเทพ kind=Named  pos=-     ne=Place   stop=false

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
use kham_core::synonym::SynonymMap;
use kham_core::stopwords::StopwordSet;
use kham_core::romanizer::RomanizationMap;

let fts = FtsTokenizer::builder()
    .synonyms(SynonymMap::from_tsv(include_str!("synonyms.tsv")))
    .stopwords(StopwordSet::from_text("ซื้อ\nขาย\n"))
    .romanization(RomanizationMap::builtin()) // adds RTGS to synonyms: กิน → "kin"
    .ngram_size(3)                            // trigrams for Unknown tokens (0 = disable)
    .build();
```

`FtsToken` fields: `text`, `position`, `kind`, `is_stop`, `synonyms`, `trigrams`, `pos`, `ne`.

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

## Building

```bash
cargo build                          # all crates (also runs build.rs → dict.bin)
cargo test --release                 # all tests
cargo test -p kham-core --release    # core only
cargo bench -p kham-core             # core criterion benchmarks
cargo bench -p kham-sqlite           # SQLite FTS5 criterion benchmarks

# Bindings
wasm-pack build kham-wasm --target web
cd kham-python && maturin develop
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
| [doc/architecture.md](doc/architecture.md) | Crate graph, pipeline flowcharts, module responsibilities (Mermaid) |
| [doc/benchmarks.md](doc/benchmarks.md) | Throughput numbers, dict construction, PostgreSQL and SQLite FTS5 benchmarks |
| [doc/dict-format.md](doc/dict-format.md) | `dict.bin` binary format, DARTS lifecycle, data sources |
| [doc/adr-001-ne-person-name-import-strategy.md](doc/adr-001-ne-person-name-import-strategy.md) | Why person names are filtered against `words_th.txt` |

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
