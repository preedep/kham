# kham

Thai word segmentation engine written in Rust. Fast, `no_std`-compatible core library with bindings for Python, WebAssembly, C, a command-line interface, and database extensions for PostgreSQL and SQLite.

[![CI](https://github.com/preedep/kham/actions/workflows/ci.yml/badge.svg)](https://github.com/preedep/kham/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/kham-core.svg)](https://crates.io/crates/kham-core)
[![PyPI](https://img.shields.io/pypi/v/kham.svg)](https://pypi.org/project/kham/)
[![npm](https://img.shields.io/npm/v/kham-wasm.svg)](https://www.npmjs.com/package/kham-wasm)

## Features

- **newmm algorithm** — DAG-based maximal matching constrained to Thai Character Cluster (TCC) boundaries
- **Compound-first DP scoring** — minimises token count before maximising dictionary matches, then uses TNC frequency as tiebreaker; 94.9% sentence-level agreement with PyThaiNLP newmm (F1 0.975)
- **Zero-copy API** — `segment()` returns `&str` slices into the original input; no heap allocation per token
- **`no_std` core** — `kham-core` compiles for bare-metal targets (`alloc` only)
- **Built-in dictionary** — 62,102-word CC0-licensed Thai word list embedded at compile time
- **Thai FTS pipeline** — `FtsTokenizer` adds stopword filtering, POS tagging, NER, RTGS romanization, phonetic soundex, abbreviation expansion, and OOV n-gram fallback
- **Named entity recognition** — gazetteer-based NER (~36,600 entries): provinces, countries, Wikipedia places/orgs, person and family names
- **Part-of-speech tagging** — 13-category lookup table (~9,000 entries)
- **Phonetic encoding** — lk82, udom83, MetaSound, and Thai–English cross-language Soundex
- **Number normalization** — Thai digits ↔ ASCII, spelled-out number words ↔ integer, Thai Baht currency text
- **Abbreviation expansion** — 118-entry built-in TSV (months, era markers, ranks, agencies)
- **Date parsing** — 7 input formats, Buddhist Era and Gregorian, round-trips to ISO 8601 and Thai text
- **Sentence segmentation** — Thai terminators, Paiyannoi, punctuation, with decimal/abbreviation-aware dot rules
- **Multi-target** — Rust crate, Python wheel, WASM module, C shared library, CLI binary, PostgreSQL FTS parser, SQLite FTS5 tokenizer

## Packages

| Crate | Registry | Docs | Description |
|---|---|---|---|
| `kham-core` | [crates.io](https://crates.io/crates/kham-core) | (this file) | Pure Rust engine, `no_std` compatible |
| `kham-cli` | [crates.io](https://crates.io/crates/kham-cli) | (this file) | `kham` binary |
| `kham-python` | [PyPI](https://pypi.org/project/kham/) | [kham-python/README.md](kham-python/README.md) | Python bindings via PyO3 / maturin |
| `kham-wasm` | [npm](https://www.npmjs.com/package/kham-wasm) | [kham-wasm/README.md](kham-wasm/README.md) | WebAssembly bindings via wasm-bindgen |
| `kham-capi` | [crates.io](https://crates.io/crates/kham-capi) | [kham-capi/README.md](kham-capi/README.md) | C FFI with cbindgen-generated header |
| `kham-pg` | [PGXN](https://pgxn.org/dist/kham_pg/) (coming soon) | [kham-pg/README.md](kham-pg/README.md) | PostgreSQL text search parser for Thai |
| `kham-sqlite` | — | [kham-sqlite/README.md](kham-sqlite/README.md) | SQLite FTS5 tokenizer for Thai |

---

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
// กินข้าว (Thai)
// กับ     (Thai)
// ปลา     (Thai)
```

Mixed script works out of the box:

```rust
let tokens = tok.segment("ธนาคาร100แห่ง");
assert_eq!(tokens[0].text, "ธนาคาร"); // Thai
assert_eq!(tokens[1].text, "100");     // Number
assert_eq!(tokens[2].text, "แห่ง");   // Thai
```

### CLI

```bash
cargo install kham-cli
```

```bash
kham "กินข้าวกับปลา"               # กินข้าว|กับ|ปลา
kham --sep " / " "สวัสดีชาวโลก"    # สวัสดี / ชาว / โลก
kham --kind "ธนาคาร100แห่ง"        # ธนาคาร:Thai|100:Number|แห่ง:Thai
kham --spans "กินข้าวกับปลา"       # กินข้าว:0-7|กับ:7-10|ปลา:10-13

# FTS pipeline — kind, POS, NE, stopword, synonyms (one token per line)
kham --fts "ทักษิณเดินทางไปกรุงเทพ"
# ทักษิณ  kind=Person  pos=-     ne=Person  stop=false  syn=-
# เดิน    kind=Thai    pos=Verb  ne=-       stop=false  syn=-
# ทาง     kind=Thai    pos=Noun  ne=-       stop=true   syn=-
# ไป      kind=Thai    pos=Verb  ne=-       stop=true   syn=-
# กรุงเทพ kind=Place   pos=-     ne=Place   stop=false  syn=-

# FTS + phonetic encoding — syn= shows the lk82 code
kham --fts --soundex lk82 "กินข้าวกับปลา" | column -t
# กินข้าว  kind=Thai  pos=-     ne=-  stop=false  syn=1619
# กับ      kind=Thai  pos=Conj  ne=-  stop=true   syn=1400
# ปลา      kind=Thai  pos=Noun  ne=-  stop=false  syn=4800

echo "กินข้าว" | kham           # stdin
RUST_LOG=debug kham "กินข้าว"  # per-token trace + timing
```

### Other targets

| Target | Quick link |
|---|---|
| Python | [kham-python/README.md](kham-python/README.md) |
| JavaScript / TypeScript (WASM) | [kham-wasm/README.md](kham-wasm/README.md) |
| C | [kham-capi/README.md](kham-capi/README.md) |
| PostgreSQL FTS | [kham-pg/README.md](kham-pg/README.md) |
| SQLite FTS5 | [kham-sqlite/README.md](kham-sqlite/README.md) |

---

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

---

## Full-Text Search

`FtsTokenizer` wraps the segmenter with the full NLP pipeline:

```rust
use kham_core::fts::FtsTokenizer;

let fts = FtsTokenizer::new();

let tokens = fts.segment_for_fts("ทักษิณเดินทางไปกรุงเทพ");
for t in &tokens {
    println!("{} ne={:?} pos={:?} stop={}", t.text, t.ne, t.pos, t.is_stop);
}
// ทักษิณ  ne=Some(Person)  pos=None  stop=false
// เดิน    ne=None          pos=Verb  stop=false
// ทาง     ne=None          pos=None  stop=true
// ไป      ne=None          pos=Verb  stop=true
// กรุงเทพ ne=Some(Place)   pos=None  stop=false  ← merged from กรุง+เทพ

// Flat lexeme list for tsvector (stopwords removed)
let lexemes = fts.lexemes("กินข้าวกับปลา");
// → ["กินข้าว", "ปลา"]
```

Builder options:

```rust
use kham_core::fts::FtsTokenizer;
use kham_core::abbrev::AbbrevMap;
use kham_core::synonym::SynonymMap;
use kham_core::stopwords::StopwordSet;
use kham_core::romanizer::RomanizationMap;
use kham_core::soundex::SoundexAlgorithm;

let fts = FtsTokenizer::builder()
    .abbrevs(AbbrevMap::builtin())             // ก.ค. → กรกฎาคม before segmentation
    .synonyms(SynonymMap::from_tsv(include_str!("synonyms.tsv")))
    .stopwords(StopwordSet::from_text("ซื้อ\nขาย\n"))
    .romanization(RomanizationMap::builtin())  // adds RTGS to synonyms: กิน → "kin"
    .soundex(SoundexAlgorithm::Lk82)          // adds lk82 code to synonyms for Thai/Named tokens
    .ngram_size(3)                             // trigrams for Unknown tokens (0 = disable)
    .number_normalize(true)                    // Thai digits → ASCII synonym (default: true)
    .build();
```

`FtsToken` fields: `text`, `position`, `kind`, `is_stop`, `synonyms`, `trigrams`, `pos`, `ne`.

---

## Number normalization

```rust
use kham_core::number::{
    thai_digits_to_ascii, parse_thai_word, u64_to_thai_word,
    parse_thai_baht, to_thai_baht_text,
};

thai_digits_to_ascii("๑๒๓")             // "123"
parse_thai_word("หนึ่งร้อยยี่สิบสาม")  // Some(123)
u64_to_thai_word(123)                   // "หนึ่งร้อยยี่สิบสาม"
parse_thai_baht("หนึ่งร้อยบาทห้าสิบสตางค์")
// Some(BahtAmount { baht: 100, satang: 50 })
to_thai_baht_text(100, 0)              // "หนึ่งร้อยบาทถ้วน"
```

In `FtsTokenizer`, number normalization runs automatically: `TokenKind::Number` tokens get their ASCII form added to `synonyms`. Opt out with `.number_normalize(false)`.

---

## Abbreviation expansion

```rust
use kham_core::abbrev::AbbrevMap;

let map = AbbrevMap::builtin();
assert_eq!(map.expand_text("วันที่5ก.ค.2567"), "วันที่5กรกฎาคม2567");
assert_eq!(map.expand_text("พ.ศ.2567"),        "พุทธศักราช2567");

let exps = map.lookup("ดร.").unwrap();
assert_eq!(exps, &["ดอกเตอร์"]);
```

Built-in TSV covers 12 month abbreviations, era markers, military/police ranks, government agencies, and Bangkok districts. Use with `FtsTokenizerBuilder::abbrevs(AbbrevMap::builtin())`.

---

## Date parsing

```rust
use kham_core::date::{parse_thai_date, Era};

let d = parse_thai_date("5 กรกฎาคม 2567").unwrap();
assert_eq!(d.to_iso8601(), "2024-07-05"); // BE 2567 → CE 2024

let d = parse_thai_date("๕ ก.ค. ๒๕๖๗").unwrap();
assert_eq!(d.to_iso8601(), "2024-07-05");

let d = parse_thai_date("5/7/2567").unwrap();
assert_eq!(d.era, Era::Buddhist);
```

Supported formats: full month name, abbreviated month, era marker (`พ.ศ.` / `ค.ศ.`), `วันที่` prefix, slash/dash-separated, Thai digits. Era inferred when omitted: year ≥ 2300 → Buddhist Era.

---

## Sentence segmentation

```rust
use kham_core::sentence::split_sentences;

let text = "สวัสดีครับ! วันนี้อากาศดีมาก\nเราไปกินข้าวกันเถอะ";
let sents = split_sentences(text);
assert_eq!(sents[0].text, "สวัสดีครับ!");
assert_eq!(sents[1].text, "วันนี้อากาศดีมาก");
assert_eq!(sents[2].text, "เราไปกินข้าวกันเถอะ");
```

| Character | Rule |
|---|---|
| `๚` `๛` | Always splits |
| `ฯ` | Splits unless part of `ฯลฯ` |
| `\n` | Always splits |
| `!` `?` | Always splits |
| `.` | Splits only when followed by whitespace or end-of-string |

---

## Named entity recognition

The built-in gazetteer (~36,600 entries) covers Thai provinces, 246 countries, 17,000+ Wikipedia places/orgs, and 9,000+ person and family names. Multi-token matching merges compound names split by the segmenter:

```
กรุงเทพ  → segmenter splits → กรุง + เทพ
         → NE tagger merges → กรุงเทพ  Named(Place)
```

See [ADR-001](doc/adr-001-ne-person-name-import-strategy.md) for the person-name import decision.

---

## Phonetic encoding (Soundex)

```rust
use kham_core::soundex::{lk82, udom83, metasound, sounds_like, SoundexAlgorithm};
use kham_core::soundex::{thai_english_soundex, sounds_like_cross_lang};

assert_eq!(lk82("กาน"), lk82("ขาน")); // same consonant group → "1600"
assert!(sounds_like("กาน", "คาน", SoundexAlgorithm::Lk82));

// Thai–English cross-language (Suwanvisat & Prasitjutrakul 1998)
let en = thai_english_soundex("McDonald");
let th = thai_english_soundex("แมคโดนัลด์");
assert_eq!(&en[..3], &th[..3]); // shared phonetic prefix
```

FTS integration — emit the soundex code as a synonym:

```rust
let fts = FtsTokenizer::builder()
    .soundex(SoundexAlgorithm::Lk82)
    .build();
```

---

## Building

```bash
cargo build                          # all crates
cargo test --release                 # all tests
cargo test -p kham-core --release    # core only
cargo bench -p kham-core             # throughput benchmarks
cargo run -p kham-bench-accuracy     # word-boundary P/R/F1
cargo run -p kham-bench-accuracy -- --threshold 0.95  # CI gate
```

Prerequisites:

| Target | Tool | Install |
|---|---|---|
| All | Rust ≥ 1.85 | `curl -sSf https://sh.rustup.rs \| sh` |
| WASM | `wasm-pack` | `cargo install wasm-pack` |
| Python | `maturin` | `pip install maturin` |
| C | `cbindgen` | `cargo install cbindgen` |
| PostgreSQL | Docker with BuildKit | [docs.docker.com](https://docs.docker.com/engine/install/) |
| SQLite (macOS) | Homebrew sqlite | `brew install sqlite` |
| SQLite (Linux) | SQLite dev headers | `apt install libsqlite3-dev` |

---

## CI

| Job | What it checks |
|---|---|
| `fmt` | `cargo fmt --check` |
| `clippy` | `cargo clippy -D warnings` |
| `test` | Unit + integration + doc tests, stable and MSRV 1.85, Linux and macOS |
| `no_std` | `kham-core` compiles for `thumbv7em-none-eabihf` |
| `wasm` | `wasm-pack build --target web` succeeds |
| `python` | `maturin develop` on Python 3.8 and 3.12 |
| `pg_regress` | 31 SQL tests across 4 suites in Docker PostgreSQL 17 |

---

## Further reading

| Document | Contents |
|---|---|
| [doc/roadmap.md](doc/roadmap.md) | Release history, pending action checklist, corpus import plan |
| [doc/architecture.md](doc/architecture.md) | Crate graph, pipeline flowcharts, module responsibilities |
| [doc/benchmarks.md](doc/benchmarks.md) | Throughput numbers, PostgreSQL and SQLite FTS5 benchmarks |
| [doc/dict-format.md](doc/dict-format.md) | `dict.bin` binary format, DARTS lifecycle, data sources |
| [doc/adr-001-ne-person-name-import-strategy.md](doc/adr-001-ne-person-name-import-strategy.md) | Person name import strategy |
| [doc/adr-002-syllables-corpus-import-decision.md](doc/adr-002-syllables-corpus-import-decision.md) | Why syllables_th.txt is excluded |
| [doc/adr-003-orchid-pos-tag-mapping.md](doc/adr-003-orchid-pos-tag-mapping.md) | ORCHID 44-tag → 13-category POS mapping |

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
