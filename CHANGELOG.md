# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2026-05-02

### Added

**kham-core**
- `StopwordSet::builtin_with_extra(extra: &str) -> StopwordSet` — combines the built-in 1 029-word list with caller-supplied domain stopwords; sorted and deduplicated
- `FtsTokenStream` — streaming iterator over `FtsToken`; adds `next_index_token()` to advance past stopwords automatically
- `FtsTokenizer::segment_stream(text) -> FtsTokenStream` — streaming view of `segment_for_fts` output

**kham-pg**
- Stopword suppression — Thai grammatical particles (กับ, ใน, ของ, …) are suppressed by `kham_fts_dict` and excluded from the tsvector, reducing index noise
- Thai number normalization — Thai digit strings (๑๒๓) now route through `kham_fts_dict`, which stores both the Thai form and the ASCII equivalent (123) as colocated lexemes; cross-script numeric queries work automatically
- POS lexeme expansion — tokens with a known part of speech emit `pos_<tag>` (e.g. `pos_noun`, `pos_verb`) as a colocated lexeme; query with `'pos_verb'::tsquery`
- `kham_fts_dict_udom83` and `kham_fts_dict_metasound` — two new dictionary variants backed by the udom83 and MetaSound soundex algorithms respectively; users can swap dictionaries in custom FTS configurations for finer phonetic discrimination
- `kham_tsvector(text) → tsvector` — SQL STABLE helper; shorthand for `to_tsvector('kham', text)`
- `kham_tsquery(text) → tsquery` — SQL STABLE helper; shorthand for `plainto_tsquery('kham', text)`
- `kham_features` regress suite — 14 SQL tests covering all new features
- Docker Hub images — `preedep/kham-pg:<version>-pg<N>` multi-arch images (amd64 + arm64) for PostgreSQL 14–18; `preedep/kham-pg:latest` points to PG 17; no Rust toolchain required

### Changed

**kham-pg**
- `number` token type now maps to `kham_fts_dict` instead of `kham_dict` in the built-in `kham` configuration; this enables Thai digit normalization but changes the lexeme output for purely numeric tokens

---

## [0.6.0] - 2026-05-01

### Added

**kham-core**
- `SpellChecker` — `SpellChecker::builtin().suggestions(word, max_n)`: Levenshtein edit distance ≤ 2 over the built-in dictionary, re-ranked by lk82 phonetic similarity and TNC frequency
- `KeyExtractor` — `KeyExtractor::builtin().extract(text, max_n)`: TF × IDF-proxy keyword extraction; stopwords and single-char tokens excluded
- `FtsTokenizerBuilder::dict_merge()` — overlays extra words on the built-in FTS dictionary without a full trie rebuild

**Bindings — Python / WASM / C FFI**
- `spell_suggestions(word, max_n)` exposed in all three bindings; returns `SpellSuggestion` / `KhamSpellList` rich result types
- `extract_keywords(text, max_n)` exposed in all three bindings; returns `Keyword` / `KhamKeywordList` rich result types

**kham-sqlite**
- Custom synonym map — `synonyms '<path>'` tokenize argument loads a TSV file at table-creation time; synonyms emitted as `FTS5_TOKEN_COLOCATED`
- Custom dictionary overlay — `dict '<path>'` tokenize argument overlays domain words without a full trie rebuild
- 31-test integration suite covering basic MATCH, RTGS, lk82 soundex, `snippet()`/`highlight()`, stopword filtering, mixed script, NE, all config options, custom synonyms and dict
- Windows build support via vcpkg (`SQLITE_INCLUDE_DIR` override; `build.rs` Windows detection)
- Android NDK build support — 4 ABIs (`arm64-v8a`, `armeabi-v7a`, `x86_64`, `x86`) via CI release workflow

**CI**
- `python` job now runs `pytest kham-python/tests/ -v` after `maturin develop` (was build-only)
- `wasm` job now runs `cargo test -p kham-wasm` before `wasm-pack build` (native `#[test]` suite)

### Fixed

**kham-sqlite**
- Trigrams not emitted — `FtsToken::trigrams` for Unknown tokens was populated but never forwarded to SQLite as colocated tokens; OOV n-gram search now works

---

## [0.5.1] - 2026-04-26

### Added

**Python bindings (`kham-python`) — full kham-core parity**
- `normalize(text)`, `romanize(text)`, `soundex(text, algorithm)` top-level functions
- `segment_sentences(text) -> list[str]` — sentence splitting
- `parse_date(text) -> str | None` — Thai date normalization → ISO 8601
- `expand_abbrevs(text) -> str` — abbreviation expansion
- `normalize_number(text) -> str | None` — Thai number word → Arabic
- `fts_tokens(text) -> list[FtsToken]` — full NLP pipeline (NE, POS, synonyms, stopwords)
- `FtsToken` with `text`, `byte_start/end`, `char_start/end`, `kind`, `pos`, `ne`, `is_stop`, `synonyms`

**WASM bindings (`kham-wasm`) — full kham-core parity**
- `normalize(text)`, `romanize(text)`, `soundex(text, algorithm)` exports
- `segment_sentences(text): string[]` — sentence splitting
- `parse_date(text): string | null` — Thai date normalization → ISO 8601
- `expand_abbrevs(text): string` — abbreviation expansion
- `normalize_number(text): string | null` — Thai number word → Arabic
- `fts_tokens(text): FtsToken[]` — full NLP pipeline
- `FtsToken` JS class with `text`, `byte_start/end`, `char_start/end`, `kind`, `pos`, `ne`, `is_stop`, `synonyms`

**C FFI (`kham-capi`) — full kham-core parity**
- `kham_normalize()` / `kham_normalized_free()` — text normalization
- `kham_romanize()` / `kham_romanized_free()` — RTGS romanization
- `kham_soundex()` / `kham_soundex_free()` — phonetic encoding (lk82 / udom83 / metasound / crosslang)
- `kham_segment_sentences()` / `kham_sentence_list_free()` — sentence splitting
- `kham_parse_date()` / `kham_date_free()` — Thai date → ISO 8601
- `kham_expand_abbrevs()` / `kham_abbrevs_free()` — abbreviation expansion
- `kham_normalize_number()` / `kham_number_free()` — number normalization
- `kham_fts_tokens()` / `kham_fts_token_list_free()` — full NLP pipeline
- `KhamFtsToken` struct with NE kind, POS tag, stopword flag, and synonym array

**Website (`kham-web`)**
- Number Conversion widget — 4-tab demo (Arabic↔Thai digits, Thai number words, Baht text)
- Normalizer widget — character-level diff display with U+ code toggle
- Soundex widget — lk82/udom83/MetaSound/cross-language phonetic demo
- Rust / Python / WASM code tabs on every API section
- FTS mode, romanization, and sentence splitting added to live demo

### Fixed
- Python test expectations corrected to match dictionary-aware (compound-first) segmentation output
- Rustfmt import ordering in all three binding crates (`kham-python`, `kham-wasm`, `kham-capi`)

---

## [0.5.0] - 2026-04-26

### Added

**kham-pg — `kham_fts_dict` custom dictionary**
- New custom text-search dictionary template (`kham_fts_dict`) that expands each Thai/Named token to up to 6 lexemes at the same tsvector position: the normalised word, its lk82 Thai Soundex code, and its RTGS romanization (if present in the built-in map)
- Latin, Number, and Unknown tokens continue through the simple `kham_dict` (lowercase pass-through)
- Enables phonetic-fuzzy search (`to_tsvector @@ plainto_tsquery('หม้อ')` matches documents containing any word with the same soundex code) and Latin-script romanization search without schema changes

**kham-pg — `ts_headline` support**
- `kham_headline` callback registered as HEADLINE function in `CREATE TEXT SEARCH PARSER kham`
- Fills `prs->startsel / stopsel / fragdelim` from caller options (defaults: `<b>`, `</b>`, ` ... `)
- Marks all non-skip tokens `in=1` (full-document mode); marks tokens matching TSQuery operands `selected=1` with prefix-query support
- 5 regress tests covering StartSel/StopSel override, exact/prefix match, no-match, and complex Thai documents

**kham-pg — Named entity token type**
- `TokenKind::Named(_) → 7` (`named`) registered in `kham_lextypes`; SQL configuration maps `named` through `kham_fts_dict`

### Fixed

**kham-pg — PG 16+ lexize calling convention**
- `kham_dict_lexize_shim` previously read arg3 as `bool isNull` via `PG_GETARG_BOOL(3)`. Since PG 16, arg3 is a `List*` pointer of subsequent tokens for multi-word recognition (always non-NULL during real token calls); reading it as a bool produced `true` and every token was silently discarded as a stopword, producing empty tsvectors for all Thai text.
- Fix: arg3 is ignored entirely. End-of-input detection now uses `token == NULL || len <= 0`.

---

## [0.4.0] - 2026-04-25

### Added

**Thai phonetic encoding — `soundex` module (`kham-core`)**
- `lk82(word)` — Lorchirachoonkul 1982; 4-char code, 12 consonant groups; most widely deployed Thai soundex
- `udom83(word)` — Udompanich 1983; 4-char code, 14 groups with finer sibilant/liquid distinctions (ส vs ช, ล vs ร)
- `metasound(word)` — Snae & Brückner 2009; per-syllable `[initial][vowel][final]` encoding
- `thai_english_soundex(word)` / `english_soundex(word)` — Suwanvisat & Prasitjutrakul 1998 cross-language table; encodes Thai and English into a shared phonetic space without a romanizer
- `soundex(word, SoundexAlgorithm)` — unified enum dispatch; `sounds_like(a, b, algo)` convenience helper
- `sounds_like_lk82`, `sounds_like_udom83`, `sounds_like_cross_lang` — boolean proximity helpers
- FTS integration: `FtsTokenizerBuilder::soundex(SoundexAlgorithm)` appends phonetic code to `FtsToken::synonyms` for Thai and Named tokens (opt-in; lk82/udom83 recommended)
- `SoundexAlgorithm` enum: `Lk82 | Udom83 | MetaSound`

**CLI — `kham-cli`**
- `--soundex <ALGO>` flag (requires `--fts`): `lk82`, `udom83`, `metasound`, `cross_lang`; phonetic code appears in `syn=` FTS output field

**Accuracy benchmark — `kham-bench-accuracy`**
- New binary crate; reads `input|tok1|tok2|…` testdata files, computes word-boundary precision / recall / F1
- `--threshold <F>` — exits non-zero if F1 falls below threshold (CI gate)
- `--verbose` — prints each failing testdata case

**Developer tooling**
- `scripts/compare_pythainlp.py` — compares kham vs PyThaiNLP `word_tokenize(engine='newmm')` on 39 built-in sentences; supports `--show-all`, `--export-testdata`, `--agreed`; annotates known PyThaiNLP errors with `[KHAM-OK]` tag and TNC frequency evidence

**Data expansions (`kham-core`)**
- NE gazetteer (`ne_th.tsv`): +17,240 Wikipedia Thai-title entries (places, organisations, CC-BY-SA-4.0) and +8,980 Thai family names → gazetteer grows from ~10,400 to ~36,600 entries
- POS table (`pos_th.tsv`): +8,691 ORCHID-derived entries (CC-BY-4.0) → from 338 to ~9,000 entries across 13 categories
- Frequency table (`tnc_freq.txt`): +2,410 TTC-only words merged in (CC0, Thai Textbook Corpus)

**Build / binary size**
- `tnc_freq.txt`, `ne_th.tsv`, `pos_th.tsv` zlib-compressed at compile time via `build.rs`; decompressed at first use — reduces embedded data size significantly

### Changed

**Segmentation algorithm — compound-first DP scoring (breaking)**

`DpScore` field order changed: `neg_unknowns → neg_tokens → dict_words → freq_score`
(was `neg_unknowns → dict_words → freq_score → neg_tokens`).

Minimising token count is now priority 2, above maximising dict-word matches. Previously, splitting a compound word into two known words scored *higher* than keeping it whole (more dict matches = better score), causing systematic over-segmentation. The new order preserves compound words and matches PyThaiNLP newmm behaviour.

**Measured impact** (39-sentence benchmark vs PyThaiNLP newmm as reference):

| Metric | Before | After |
|---|---|---|
| Sentence agreement | 1/39 (2.6%) | 37/39 (94.9%) |
| Micro F1 | 0.418 | 0.975 |
| Genuine diffs | — | **0** (2 remaining are confirmed PyThaiNLP errors) |

Callers who relied on the previous splitting behaviour will see different token output for compound words (e.g. `กินข้าว` → one token, not two; `วันนี้` → one token, not two).

### Fixed

- Soundex doctest: `sounds_like_cross_lang("McDonald", "MacDonald")` was incorrect (not equal); replaced with `sounds_like_cross_lang("กาน", "คาน")`
- `pos.rs` integration test: `fts_token_has_pos_for_known_thai_word` used `"กินข้าว"` and expected separate `กิน` / `ข้าว` tokens — now tests each word standalone since the compound is one token

---

## [0.3.0] - 2026-04-25

### Added

**New NLP modules (`kham-core`)**
- `abbrev` — `AbbrevMap` with 118-entry built-in TSV (months, era markers, military/police ranks, government agencies, Bangkok districts); greedy longest-first pre-tokenisation text expansion; post-tokenisation single-token lookup; ambiguous abbreviations return all expansions
- `date` — Thai date normalization: parses 7 input formats (full month, abbreviated month, era marker, `วันที่` prefix, slash/dash-separated, Thai digits) in both Buddhist Era and Gregorian; formats to ISO 8601 or Thai text; heuristic era inference (year ≥ 2300 → Buddhist Era)
- `sentence` — Thai sentence segmentation: splits on Thai terminators (`๚` `๛`), Paiyannoi (`ฯ`, excluding `ฯลฯ`), universal punctuation (`!` `?`), newlines, and `.` when followed by whitespace or end-of-string; decimal- and abbreviation-aware dot rules prevent false splits

**FTS pipeline (`kham-core`)**
- `FtsTokenizerBuilder::abbrevs(AbbrevMap)` — opt-in pre-segmentation abbreviation expansion; disabled by default

## [0.2.0] - 2026-04-24

### Added

**Named Entity Recognition (`kham-core`)**
- `NeTagger`: gazetteer-based tagger with greedy longest-match multi-token support (up to 5 consecutive tokens)
- Built-in NE gazetteer (`ne_th.tsv`): 10,488 entries — countries (PyThaiNLP Apache-2.0) and Thai person names (dictionary-filter strategy, ADR-001)
- `TokenKind::Named(NamedEntityKind)` variant in `Token`; `NamedEntityKind`: Person / Place / Org
- FTS pipeline: NE surface form injected as synonym; `FtsToken::ne` field set for Named tokens
- kham-pg: token type 7 (`named`) registered in `kham_lextypes`; SQL config maps `named` through `kham_dict`

**Part-of-Speech Tagging (`kham-core`)**
- `PosTagger`: lookup-based Thai POS tagger; `pos_th.tsv` with 338 entries (13 categories: NOUN VERB ADJ ADV PART PROPN PRON NUM CLAS CONJ AUX DET PREP)
- `FtsToken::pos` field wired into the FTS pipeline; NE-tagged tokens skip POS lookup

**RTGS Romanization (`kham-core`)**
- `RomanizationMap`: table-driven Thai → Roman mapping; `romanization_th.tsv` with 415 entries
- `romanize()`, `romanize_or_raw()`, `romanize_tokens()` — zero-alloc syllable-level mapping
- `FtsTokenizerBuilder::romanization(RomanizationMap)` — opt-in; RTGS form injected as synonym for Thai and Named tokens

**Number normalization (`kham-core`)**
- `thai_digits_to_ascii`, `parse_thai_word`, `u64_to_thai_word`, `parse_thai_baht`, `to_thai_baht_text`, `BahtAmount`
- FTS auto-adds ASCII synonyms for Thai digit tokens and number-word tokens; opt-out with `FtsTokenizerBuilder::number_normalize(false)`

**SQLite FTS5 extension (`kham-sqlite`)**
- New `kham-sqlite` cdylib: loadable SQLite extension registering a `kham` FTS5 tokenizer
- Full NLP pipeline: normalization → segmentation → NE tagging → synonym expansion → RTGS romanization via `FTS5_TOKEN_COLOCATED`
- Byte-accurate offsets into normalized text enable `highlight()` and `snippet()`
- Exports `sqlite3_kham_init` (explicit load) and `sqlite3_khamsqlite_init` (implicit)
- Criterion benchmark suite for SQLite FTS5 throughput

**FTS pipeline (`kham-core` + `kham-cli`)**
- `FtsTokenizer` wires POS, NE, romanization, stopwords, and synonym expansion in a single pipeline pass
- NE tagging runs before POS tagging; `Named` tokens have `pos: None`
- `FtsTokenizerBuilder::number_normalize(bool)` — opt-out of Thai digit/word normalization (default: `true`)
- CLI `--fts` flag: outputs kind / POS / NE / stopword metadata per token

**Documentation**
- `doc/architecture.md`, `doc/benchmarks.md`, `doc/dict-format.md` split out from README
- `doc/adr-001-ne-person-name-import-strategy.md` — ADR for NE gazetteer person-name import approach
- Per-crate `CLAUDE.md` files: `kham-core/`, `kham-cli/`, `kham-pg/`

**C FFI (`kham-capi`)**
- `kham-capi/include/kham.h` regenerated via cbindgen and now tracked in the repository

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

[0.6.0]: https://github.com/preedep/kham/releases/tag/v0.6.0
[0.5.1]: https://github.com/preedep/kham/releases/tag/v0.5.1
[0.5.0]: https://github.com/preedep/kham/releases/tag/v0.5.0
[0.4.0]: https://github.com/preedep/kham/releases/tag/v0.4.0
[0.3.0]: https://github.com/preedep/kham/releases/tag/v0.3.0
[0.2.0]: https://github.com/preedep/kham/releases/tag/v0.2.0
[0.1.3]: https://github.com/preedep/kham/releases/tag/v0.1.3
[0.1.2]: https://github.com/preedep/kham/releases/tag/v0.1.2
[0.1.1]: https://github.com/preedep/kham/releases/tag/v0.1.1
[0.1.0]: https://github.com/preedep/kham/releases/tag/v0.1.0
