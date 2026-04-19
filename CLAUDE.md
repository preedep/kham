# kham.rs — Thai Word Segmentation Engine

Batteries-included Thai word segmentation library in Rust. Multi-target: Rust crate, WASM, Python (PyO3), C FFI, CLI.

## Architecture

Workspace with multiple crates:

- `kham-core/` — Pure Rust, `no_std` compatible. Contains all segmentation and FTS logic.
    - `normalizer` — Thai text normalization (สระลอย reorder, วรรณยุกต์, NFC)
    - `pre_tokenizer` — Unicode script classification (Thai/Latin/Number/Emoji/URL)
    - `tcc` — Thai Character Cluster boundaries (Theeramunkong et al. 2000)
    - `dict` — Double-Array Trie (DARTS), built-in `words_th.txt` via `include_bytes!`
    - `freq` — TNC frequency table (`tnc_freq.txt`), `FreqMap` used by DP scorer
    - `segmenter` — DAG-based maximal matching (newmm algorithm)
    - `token` — `Token` struct with text, byte span, char span, `TokenKind`
    - `stopwords` — `StopwordSet`: sorted `Vec<String>` with binary-search lookup; built-in 1 029-entry list from PyThaiNLP (Apache-2.0) embedded via `include_str!`
    - `synonym` — `SynonymMap`: `BTreeMap<canonical → Vec<synonym>>` loaded from TSV; used by `FtsTokenizer` for query-time expansion
    - `ngram` — `char_ngrams(text, n)` (zero-alloc `&str` iterator) and `token_ngrams(tokens, n)` (owned `String` iterator); OOV fallback indexing
    - `fts` — `FtsTokenizer` / `FtsToken`: orchestrates normalize → segment → stopword tag → synonym expand → OOV trigrams; entry point for `kham-pg` (Phase 2)
- `kham-python/` — PyO3 bindings; exposes `segment()` → `list[str]` and `segment_tokens()` → `list[Token]`
- `kham-wasm/` — wasm-bindgen bindings; exposes `segment()` → `string[]` and `segment_tokens()` → `Token[]`
- `kham-capi/` — C FFI with cbindgen; exposes `kham_segment()` (legacy `KhamTokens`) and `kham_segment_tokens()` (`KhamTokenList` with `KhamToken` structs)
- `kham-cli/` — CLI binary using clap
- `kham-pg/` — PostgreSQL text search extension (`cdylib`). C shim (`src/shim.c`) bridges PG's fmgr API to Rust parser callbacks. Requires `pg_config` in `PATH` (or `PG_CONFIG` env var) to build. Tests run in Docker.

## Commands

```bash
cargo fmt --all                      # format (run before every commit)
cargo fmt --all -- --check           # verify formatting (CI gate)
cargo clippy --all-targets -- -D warnings  # lint (CI gate)
cargo build                          # build all crates
cargo test                           # run all tests
cargo test -p kham-core              # test core only
cargo bench                          # run benchmarks (criterion)
cargo run -p kham-cli -- "ข้อความ"    # run CLI

# Binding targets (see Prerequisites in README.md for tooling)
wasm-pack build kham-wasm --target web                          # build WASM
maturin develop -m kham-python/Cargo.toml                       # build Python wheel
source .venv/bin/activate && pytest kham-python/tests/ -v       # run Python tests
cbindgen --config kham-capi/cbindgen.toml \
    --crate kham-capi --output kham-capi/include/kham.h         # regenerate C header
cargo build -p kham-capi --release                              # build C shared library

# kham-pg (requires pg_config in PATH or PG_CONFIG env var)
cargo build -p kham-pg --release                               # build PG extension .so
make -C kham-pg install                                        # copy .so + control + SQL to PG install
make -C kham-pg regress                                        # run pg_regress in Docker (PG 17)
```

## Code Style

- Rust 2021 edition, MSRV 1.85+
- `#![no_std]` in kham-core — use `alloc` crate, no `std` dependency
- All public APIs must have doc comments with Thai+English examples
- Error handling: return `Result<T, KhamError>` — no `.unwrap()` in library code
- Zero-copy where possible — return `&str` slices referencing input text
- For general Rust conventions, follow the `rust-engineer` skill
- For wasm build Rust, follow the `rust-wasm-build` skill

### Formatting — run before every commit

```bash
cargo fmt --all                  # format
cargo fmt --all -- --check       # verify (what CI runs)
```

The CI `fmt` job fails if any diff exists. Common triggers:
- Long function signatures or call-sites not broken across lines (`rustfmt` wraps at 100 chars)
- Struct literals with 3+ fields left on one line
- `assert!` / `assert_eq!` with a message argument not split onto its own line

**Always run `cargo fmt --all` before pushing.** If you edit any `.rs` file, format before committing — do not rely on CI to catch it.

### Clippy — run before every commit

```bash
cargo clippy --workspace --exclude kham-python --exclude kham-wasm --all-targets -- -D warnings
```

Common clippy failures in this codebase:
- `map_or(false, |x| …)` → use `is_some_and(|x| …)` instead
- `map_or(true, |x| …)` → use `is_none_or(|x| …)` instead
- Needless `return`, redundant closures, or unused `mut` bindings
- Literal tab characters inside `//!` or `///` doc-comment code blocks → replace with spaces or `<TAB>` placeholder text (`tabs_in_doc_comments` lint)

## Token Output Contract

Every segmentation returns `Vec<Token>` where:

```rust
pub struct Token<'a> {
    pub text: &'a str,            // zero-copy reference to input
    pub span: Range<usize>,       // byte offsets in original text
    pub char_span: Range<usize>,  // Unicode scalar-value (char) offsets in original text
    pub kind: TokenKind,          // Thai | Latin | Number | Punctuation | Emoji | Whitespace | Unknown
}
```

Byte spans must be valid UTF-8 boundaries. `char_span` is suitable for Python/JavaScript string indexing. Always test both `span` and `char_span` with mixed Thai+English+Number+Emoji input.

## Testing

- Unit tests co-located in each module
- Integration tests in `kham-core/tests/` with real Thai text
- Benchmark suite in `benches/` using criterion — compare against nlpO3
- Test data: `kham-core/testdata/` — one `.txt` file per scenario, loaded by `kham-core/tests/integration.rs`
  - `basic.txt` — pure Thai sentences, all tokens must be `TokenKind::Thai`
  - `mixed_script.txt` — Thai + Latin + Number combinations
  - `normalization.txt` — canonical inputs; asserts `normalize()` is idempotent then segments correctly
  - Format: `input|tok1|tok2|…` (one case per line; lines starting with `#` are comments; whitespace tokens excluded)
- Edge cases to always test: สระลอย, วรรณยุกต์ซ้อน, zero-width chars, mixed script "ธนาคาร100แห่ง", empty string, single char
- Python binding tests: `kham-python/tests/test_kham.py` — 30 pytest cases covering `segment_tokens()` char_span round-trip, byte_span UTF-8 decoding, kind labels, contiguity, and edge cases; run after every `maturin develop`
- kham-pg tests run inside Docker (PostgreSQL 17). Use `make -C kham-pg regress` — this builds the image and runs `pg_regress` in a single container. See `kham-pg/docker/` for Dockerfile and entrypoint. Expected output files live in `kham-pg/regress/expected/`; `results/` is gitignored.
  
## Dictionary

- Built-in: `words_th.txt` (Apache-2.0, sourced from PyThaiNLP) embedded at compile time
- Custom dict loaded at runtime via `Tokenizer::builder().dict_file("path")`
- Trie implementation: Double-Array Trie for O(n) lookup
- Never ship BEST corpus or any non-CC0 data in the repo
- Any trie extension must be generated by our own code and build process; avoid external trie-building utilities
- Keep memory usage predictable and efficient
- Avoid unnecessary allocations during dictionary loading and token lookup
- Prefer compact trie/node representations for large-scale dictionaries
- Frequency data: `tnc_freq.txt` (Apache-2.0, sourced from PyThaiNLP) embedded separately from `dict.bin` — loaded into `FreqMap` at runtime, used by the newmm DP scorer to break ties between equally-matched segmentation paths; do not merge into the trie binary
- Stopword data: `stopwords_th.txt` (Apache-2.0, sourced from PyThaiNLP) embedded via `include_str!` — 1 029 Thai function words parsed and sorted at runtime into `StopwordSet`; attribution header must be kept in the data file

## FTS Modules

Four modules in `kham-core` implement Phase 1 of Thai full-text search support. All are `no_std` / `alloc`-only.

### `stopwords` — `StopwordSet`

```rust
StopwordSet::builtin()               // 1 029-word built-in list (PyThaiNLP Apache-2.0)
StopwordSet::from_text(data: &str)   // parse newline-separated list; lines with # ignored; BOM stripped
set.contains(word: &str) -> bool     // O(log n) binary search on sorted Vec<String>
set.len() -> usize
```

Data file: `kham-core/data/stopwords_th.txt` — sorted, deduplicated, UTF-8, BOM-stripped. Attribution header must be preserved.

### `synonym` — `SynonymMap`

TSV format: `canonical<TAB>syn1<TAB>syn2<TAB>…` (one rule per line; `#` lines ignored).

```rust
SynonymMap::empty()                  // no expansions
SynonymMap::from_tsv(data: &str)     // parse TSV; duplicate canonicals merge their synonym lists
map.expand(word: &str) -> Option<&[String]>   // None if no entry
map.has_synonyms(word: &str) -> bool
```

When adding a new synonym entry, duplicate canonicals accumulate — later lines extend the existing `Vec`, not replace it.

### `ngram` — character and token n-grams

```rust
// Zero-alloc iterator: yields &str slices of exactly n Unicode scalar values
char_ngrams(text: &str, n: usize) -> impl Iterator<Item = &str>

// Owned iterator: concatenates n consecutive token strings
token_ngrams(tokens: &[&str], n: usize) -> impl Iterator<Item = String>
```

**Unknown-token constraint:** The newmm DP emits Unknown tokens one TCC at a time. Bare consonants (1 char) produce no grams when `n ≥ 2`. Only multi-char TCCs (e.g. consonant + vowel = 2 chars) generate grams. This is expected — single Thai chars are morphemically atomic and are not useful n-gram anchors.

### `fts` — `FtsTokenizer` / `FtsToken`

```rust
pub struct FtsToken {
    pub text: String,         // owned; may be normalised
    pub position: usize,      // ordinal index in non-whitespace sequence (0-based)
    pub kind: TokenKind,      // from underlying segmenter
    pub is_stop: bool,        // matched StopwordSet
    pub synonyms: Vec<String>,// from SynonymMap (empty if no match)
    pub trigrams: Vec<String>,// char n-grams for TokenKind::Unknown tokens only
}
```

```rust
FtsTokenizer::new()                          // built-in stopwords, no synonyms, ngram_size=3
FtsTokenizer::builder()
    .stopwords(StopwordSet)
    .synonyms(SynonymMap)
    .ngram_size(usize)                       // 0 = disable n-gram generation
    .build()

fts.segment_for_fts(text) -> Vec<FtsToken>  // all non-whitespace tokens, with metadata
fts.index_tokens(text)    -> Vec<FtsToken>  // stopwords removed; positions preserved
fts.lexemes(text)         -> Vec<String>    // flat list: text + synonyms + trigrams per token
```

`lexemes()` is the single method `kham-pg` (Phase 2) calls to populate a PostgreSQL `tsvector`.

### FTS implementation rules

- `segment_for_fts` always calls `normalize()` internally before `segment()` — callers do not need to normalise first.
- Stopword positions are preserved in `index_tokens` output so phrase-distance scoring remains correct.
- `trigrams` is only populated for `TokenKind::Unknown` tokens; `TokenKind::Thai` tokens never receive trigrams.
- Do not add `std`-only code to any of these modules. If FST support is ever needed (synonym sets > 5k), gate it behind `#[cfg(feature = "std")]`.

## kham-pg Extension

`kham-pg` is a PostgreSQL text search parser (`cdylib`) for Thai, wrapping `kham-core`'s `FtsTokenizer`.

### Architecture

```
PostgreSQL fmgr  ──▶  src/shim.c (C)  ──▶  kham_*_impl() (Rust, src/lib.rs)
                       PG_MODULE_MAGIC
                       PG_FUNCTION_INFO_V1
                       PG_GETARG_POINTER / PG_GETARG_INT32 / PG_RETURN_*
                       palloc / pfree / pstrdup / ereport
```

### Parser callback signatures (what PostgreSQL actually passes)

| Callback    | SQL signature                         | PG call site                        |
|-------------|---------------------------------------|-------------------------------------|
| `kham_start`    | `(internal, int4) → internal`     | `OidFunctionCall2(start, PointerGetDatum(buf), Int32GetDatum(len))` |
| `kham_gettoken` | `(internal, internal, internal) → int4` | state + char** + int* output pointers |
| `kham_end`      | `(internal) → void`               | state pointer                       |
| `kham_lextypes` | `(internal) → internal`           | returns palloc'd `LexDescr[]`       |

**Critical:** `kham_start` receives a raw `char*` + `int4` length — NOT a varlena `text*`. Use `PG_GETARG_POINTER(0)` + `PG_GETARG_INT32(1)`, never `PG_GETARG_TEXT_PP`.

### Token type integers

| PG type | Name      | `TokenKind`          |
|---------|-----------|----------------------|
| 1       | `thai`    | `TokenKind::Thai`    |
| 2       | `latin`   | `TokenKind::Latin`   |
| 3       | `number`  | `TokenKind::Number`  |
| 4       | `punct`   | `TokenKind::Punctuation` |
| 5       | `emoji`   | `TokenKind::Emoji`   |
| 6       | `unknown` | `TokenKind::Unknown` |

### SQL install objects

Created by `kham_pg--0.1.0.sql` in this order:
1. `CREATE FUNCTION kham_start/gettoken/end/lextypes` — registers the C symbols from `MODULE_PATHNAME`
2. `CREATE TEXT SEARCH PARSER kham` — wires up the four functions
3. `CREATE TEXT SEARCH DICTIONARY kham_dict` — `simple` template (lowercase pass-through)
4. `CREATE TEXT SEARCH CONFIGURATION kham` — uses `kham` parser
5. `ALTER TEXT SEARCH CONFIGURATION kham ADD MAPPING FOR thai, latin, number, unknown WITH kham_dict`

Punctuation and emoji have no mapping — PG discards those token types at index time.

### Build requirements

- `pg_config` in `PATH` **or** `PG_CONFIG=/path/to/pg_config`
- C compiler (clang or gcc) — `cc` crate compiles `src/shim.c`
- For regress tests: Docker with BuildKit

### Required C headers (include order matters)

```c
#include "postgres.h"        // must be first
#include "fmgr.h"            // PG_FUNCTION_INFO_V1, PG_GETARG_*, PG_RETURN_*
#include "tsearch/ts_public.h"  // LexDescr
#include "utils/palloc.h"    // palloc, pfree, pstrdup
```

`varatt.h` (VARDATA_ANY / VARSIZE_ANY_EXHDR) is only needed if reading varlena arguments. `kham_start` uses raw pointer args, so it is not included.

### Docker test environment

Multi-stage build (`kham-pg/docker/Dockerfile.test`):
- **Stage 1 (builder)**: `debian:bookworm-slim` + `postgresql-server-dev-17` + Rust → `libkham_pg.so`
- **Stage 2 (runner)**: `debian:bookworm-slim` + `postgresql-17` only — no Rust toolchain (~200 MB vs ~2 GB single-stage)

Do **not** use Alpine/musl: Rust musl targets are static-only and do not support `cdylib`.

Key constraints:
- `dynamic_shared_memory_type = mmap` must be set before `pg_ctl start` (PG 17 removed `none`)
- pg_ctl/initdb run as `postgres` via `gosu`; entrypoint uses `pg_config` for all paths
- `pg_regress` binary: `$(pg_config --pgxs | dirname | dirname)/test/regress/pg_regress`
- Use `--outputdir=.` in pg_regress so output lands at `regress/output/` and `regress/results/`
- Linux cdylib symbol export: `build.rs` provides `src/pg_exports.map` version script so `Pg_magic_func`, `kham_start`, and all `pg_finfo_*` symbols appear in the dynamic table

## Segmenter DP scoring

The newmm forward DP uses a 4-field lexicographic score (`DpScore`) to select the best segmentation path. Priority order is fixed — do not reorder the fields:

1. **Minimise unknowns** (`neg_unknowns`) — fewest out-of-vocabulary tokens, primary criterion
2. **Maximise dict matches** (`dict_words`) — more recognised words is better
3. **Maximise TNC frequency** (`freq_score`) — cumulative raw corpus counts break ties between equally-matched paths; unknown edges contribute 0
4. **Minimise token count** (`neg_tokens`) — final tiebreaker; fewer, longer tokens preferred

When adding a new scoring dimension, insert it at the correct priority position and update the `DpScore` struct field order (the `Ord` derive compares fields in declaration order).

## Binding crates

All three bindings expose two functions:

| Function | Returns | Use when |
|---|---|---|
| `segment(text)` | token strings only | simple tokenisation, backward-compatible |
| `segment_tokens(text)` | rich token objects | span information needed |

**Token field mapping** — `Token.char_span: Range<usize>` is flattened into two integer fields in every binding: `char_start` and `char_end`. The same pattern applies to `span` → `byte_start` / `byte_end`. Follow this convention for any future `Token` field additions.

**Rule: adding a field to `Token` requires updating all three bindings.** The binding layer is the boundary where Rust types are serialised for the target language — a field that exists in `kham-core` but not in the binding is silently invisible to callers.

**C FFI legacy API** — `KhamTokens` / `kham_segment()` / `kham_tokens_free()` exist for backward compatibility and return only token strings. Do not remove them. New span-aware callers should use `kham_segment_tokens()` / `kham_token_list_free()`.

**C FFI safety** — `kham-capi` and `kham-pg` are the only crates that use `unsafe` (FFI boundaries). Do not add `unsafe` to any other crate. In `kham-pg`, unsafe is confined to `src/lib.rs`; `src/shim.c` is plain C with no unsafe Rust.

**C header location** — `kham-capi/include/kham.h` is generated by cbindgen and is gitignored. Regenerate it with `cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham-capi/include/kham.h` whenever `KhamToken`, `KhamTokenList`, or any exported symbol changes. The `[export] include` list in `cbindgen.toml` must be kept in sync with the public `#[repr(C)]` structs in `src/lib.rs`.

## CLI design

`kham-cli` exposes options that map to **user-facing runtime inputs** — things that vary per invocation and that a non-developer user can reasonably author:

- `--dict <FILE>` — custom word list (plain text, one word per line); users have domain vocabulary
- `--sep`, `--whitespace`, `--normalize`, `--kind`, `--spans` — output formatting
  - `--kind` appends token kind: `กิน:Thai`
  - `--spans` appends Unicode char span: `กิน:0-3`
  - Combined `--kind --spans`: `กิน:Thai:0-3`

**Do not add** a `--freq-file` or `--no-freq` flag. Frequency data is an internal scorer detail, not a user input:
- It is a tiebreaker that only activates when unknown count and dict match count are identical
- Users cannot meaningfully author replacement frequency data (requires corpus counts)
- If domain-tuned frequencies are ever needed, add `freq_tsv(data: &str)` to `TokenizerBuilder` first, then reconsider the CLI

## Important

- This is a library-first project — `kham-core` must never depend on `std`
- Performance matters — benchmark every PR that touches segmenter or dict
- Algorithm reference: study nlpO3 (Apache-2.0) and PyThaiNLP newmm, but write clean-room implementation
- All Thai text in tests must be valid UTF-8, never raw bytes

## Documentation

- Write clear, concise documentation for each module and function
- Use doc comments (`///`) for public APIs
- Include examples and usage notes where appropriate
- Follow Rust style guide for documentation formatting
- Update documentation regularly as features evolve in README.md and Related Documentation
- Diagram uses mermaid for visual representation

## Performance

kham.rs is designed as a high-performance Thai word segmentation engine.

Benchmarks are implemented using `criterion` and cover:

- Dictionary lookup (Double-Array Trie)
- Prefix matching
- Full text segmentation (newmm DAG)
- Mixed-script input (Thai + Latin + Number)

Run benchmarks:

```bash
cargo bench
```

### Benchmark Scope

- Dictionary build time (`dict/construction/from_binary_blob`, `from_builtin_word_list`)
- Dictionary lookup throughput (`dict/contains/hit`, `dict/contains/miss`)
- Prefix lookup throughput (`dict/prefixes`)
- FreqMap startup cost (`freq/construction/builtin`) — compare against `dict/construction/from_binary_blob` to see relative weight
- FreqMap lookup throughput (`freq/get`) — hit, rare-hit, and miss cases
- End-to-end segmentation throughput (`segment/by_length/short`, `medium`, `long`) — pure Thai, reports MB/s
- Mixed-script segmentation performance (`segment/mixed/sparse`, `medium`, `dense`) — exercises pre-tokenizer boundary overhead

### Benchmark Rules

- Run on release mode only
- Report CPU, OS, Rust version, and dictionary size
- Compare against previous baseline before merging performance-sensitive changes