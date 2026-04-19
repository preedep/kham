# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2026-04-19

### Added

**pg_regress test suite (`kham-pg`) — 67 tests across 4 suites**
- `kham_fts.sql` (19 tests): extension load, `ts_token_type`, `ts_parse` (pure Thai, stopwords, mixed script, empty), `to_tsvector` with exact lexeme positions, `plainto_tsquery`/`to_tsquery` match and no-match, Latin lowercasing, GIN index table search, `ts_rank`
- `kham_thai.sql` (20 tests): single-char Unknown, Thai numerals, mixed Thai+numeral, OOV tokens, punctuation, compound word segmentation (`โรงพยาบาล`, `สวัสดีครับ`, `นักพัฒนา`), stopword presence in `ts_parse` and `tsvector`, mixed scripts, whitespace filter, compound FTS, determinism
- `kham_operators.sql` (15 tests): AND / OR / NOT operators, phrase queries (`phraseto_tsquery`), `websearch_to_tsquery` (space-AND and minus exclusion), `ts_debug` alias and lexemes
- `kham_ranking.sql` (13 tests): `ts_rank` and `ts_rank_cd` non-zero for match, `ts_rank` zero for no-match, frequency-based ranking (ปลา×2 > ปลา×1), `ts_stat` lexeme statistics, ORDER BY rank DESC, product catalogue GIN search (10 Thai food items, 4 ingredient queries), NULL document and NULL tsvector handling

### Notes
- `ts_headline` is not supported by the kham parser (no HEADLINE callback); documented as a known limitation

## [0.1.2] - 2026-04-19

### Added

**PostgreSQL extension (`kham-pg`)**
- New `kham-pg` cdylib crate: Thai text search parser for PostgreSQL 17
- C shim (`src/shim.c`) bridges PostgreSQL fmgr API to Rust parser callbacks via `#[no_mangle]` trampolines
- Parser callbacks: `kham_start`, `kham_gettoken`, `kham_end`, `kham_lextypes`
- 6 token types registered: `thai` (1), `latin` (2), `number` (3), `punct` (4), `emoji` (5), `unknown` (6)
- SQL install script `kham_pg--0.1.2.sql`: creates parser, dictionary, configuration, and token mappings
- `make -C kham-pg install` — installs `.so`, control file, and SQL into the host PostgreSQL
- `make -C kham-pg regress` — runs `pg_regress` integration tests inside Docker (PostgreSQL 17)
- Two-stage Docker build (`Dockerfile.test`): builder with Rust + pg headers, runner with postgresql-17 only (~200 MB)
- macOS `build.rs`: auto-detects Homebrew gettext prefix for `libintl.h`
- `PG_MODULE_MAGIC_DATA` portability guard (`#ifdef PG_MODULE_ABI_DATA`) for PGDG PG17 vs Homebrew/PG18+

**FTS modules (`kham-core`)**
- `stopwords` — `StopwordSet`: 1 029-entry built-in list (PyThaiNLP Apache-2.0), O(log n) lookup
- `synonym` — `SynonymMap`: TSV-based canonical → synonym expansion, `BTreeMap` backed
- `ngram` — `char_ngrams` (zero-alloc `&str` iterator) and `token_ngrams` (owned `String` iterator)
- `fts` — `FtsTokenizer` / `FtsToken`: normalize → segment → stopword tag → synonym expand → OOV trigrams
- `FtsTokenizer::lexemes()` — flat lexeme list consumed by `kham-pg` to populate `tsvector`

**C FFI (`kham-capi`)**
- `kham_fts_lexemes()` / `kham_fts_lexemes_free()` — exposes `FtsTokenizer::lexemes()` across the C boundary

### Fixed
- `PG_MODULE_MAGIC_DATA` macro portability between PGDG PG17 and Homebrew builds
- macOS CI: Homebrew gettext include path added to `build.rs` for `libintl.h`
- `kham_gettoken` SQL return type changed from `int4` to `internal` (required by PG17)

## [0.1.1] - 2026-04-18

### Added
- Dual `MIT OR Apache-2.0` licensing (`LICENSE-MIT`, `LICENSE-APACHE`)
- `scripts/deploy.sh` — publish script for crates.io, npm, and PyPI
- `pyproject.toml` for `kham-python` maturin builds
- `readme` field added to workspace and all crate manifests for crates.io display
- FreqMap unit tests: entry count, frequency ordering, and segmentation tie-breaking

## [0.1.0] - 2026-04-18

### Added

**Core (`kham-core`)**
- DAG-based newmm Thai word segmentation algorithm
- Double-Array Trie (DARTS) dictionary with pre-compiled binary blob for fast startup
- Built-in word list (`words_th.txt`, CC0) embedded at compile time
- Thai Character Cluster (TCC) boundary detection (Theeramunkong et al. 2000)
- Unicode script pre-tokenizer: Thai / Latin / Number / Punctuation / Emoji / Whitespace / Unknown
- Thai text normalizer: สระลอย reordering, วรรณยุกต์ deduplication, NFC
- `Token` struct with zero-copy `text: &str`, `span` (byte offsets), `char_span` (Unicode scalar offsets), and `TokenKind`
- `FreqMap` — Thai National Corpus (TNC) frequency table for DP tie-breaking (CC0)
- 4-field lexicographic DP score (`DpScore`): minimise unknowns → maximise dict matches → maximise TNC frequency → minimise token count
- `no_std` compatible (uses `alloc`); optional `std` feature for stdlib integration
- Runtime custom dictionary via `TokenizerBuilder::dict_file()`

**CLI (`kham-cli`)**
- `kham <text>` — segment Thai text, one token per line
- `--dict <FILE>` — load custom word list
- `--sep <STR>` — custom token separator
- `--whitespace` — include whitespace tokens
- `--normalize` — print normalized form before segmenting
- `--kind` — append token kind (`กิน:Thai`)
- `--spans` — append Unicode char span (`กิน:0-3`)

**Python bindings (`kham-python`, PyO3)**
- `kham.segment(text) -> list[str]`
- `kham.segment_tokens(text) -> list[Token]`
- `Token` with `text`, `byte_start`, `byte_end`, `char_start`, `char_end`, `kind`

**WASM bindings (`kham-wasm`, wasm-bindgen)**
- `segment(text: string): string[]`
- `segment_tokens(text: string): Token[]`
- `Token` with matching fields

**C FFI (`kham-capi`, cbindgen)**
- `kham_segment()` / `kham_tokens_free()` — legacy string-only API
- `kham_segment_tokens()` / `kham_token_list_free()` — span-aware `KhamToken` API

**Testing & Benchmarks**
- Integration tests auto-discovering `kham-core/testdata/*.txt` (format: `input|tok1|tok2|…`)
- 30-case pytest suite for Python bindings covering `char_span` round-trip, UTF-8 byte spans, kind labels, and contiguity
- Criterion benchmark suite: dict construction, trie lookup, prefix matching, FreqMap, end-to-end segmentation (short/medium/long), mixed-script scenarios

[0.1.3]: https://github.com/preedep/kham/releases/tag/v0.1.3
[0.1.2]: https://github.com/preedep/kham/releases/tag/v0.1.2
[0.1.1]: https://github.com/preedep/kham/releases/tag/v0.1.1
[0.1.0]: https://github.com/preedep/kham/releases/tag/v0.1.0
